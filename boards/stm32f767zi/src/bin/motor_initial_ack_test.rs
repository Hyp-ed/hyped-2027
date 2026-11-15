#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    can::{
        filter::Mask32, Can, CanRx, CanTx, Fifo, Rx0InterruptHandler, Rx1InterruptHandler,
        SceInterruptHandler, TxInterruptHandler,
    },
    peripherals::CAN1,
};
use embassy_time::{with_timeout, Duration, Timer};
use hyped_can::HypedCanFrame;
use hyped_motors::{can_open_message::CanOpenMessage, can_open_processor::Messages};
use panic_probe as _;
use static_cell::StaticCell;

const NODE_ID: u8 = 0x01;

bind_interrupts!(struct Irqs {
    CAN1_RX0 => Rx0InterruptHandler<CAN1>;
    CAN1_RX1 => Rx1InterruptHandler<CAN1>;
    CAN1_SCE => SceInterruptHandler<CAN1>;
    CAN1_TX => TxInterruptHandler<CAN1>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    info!("Setting up CAN1 (main bus)...");
    static CAN: StaticCell<Can<'static>> = StaticCell::new();
    let can = CAN.init(Can::new(p.CAN1, p.PD0, p.PD1, Irqs));

    can.modify_filters()
        .enable_bank(0, Fifo::Fifo0, Mask32::accept_all());
    can.modify_config().set_bitrate(500_000);
    can.enable().await;

    info!("CAN1 enabled");

    let (mut tx, mut rx) = can.split();

    match rx.read().await {
        Ok(envelope) => {
            let frame = envelope.frame;
            match frame.id() {
                embassy_stm32::can::Id::Standard(id) => {
                    const BOOT_UP_ID: u16 = 0x700;
                    defmt::assert_eq!(id.as_raw(), BOOT_UP_ID + NODE_ID as u16);
                }
                embassy_stm32::can::Id::Extended(id) => {
                    defmt::panic!(
                        "Bad bootup frame id: Expected standard, received extended: {}",
                        id.as_raw()
                    )
                }
            }
        }
        Err(e) => {
            error!("{}", e);
        }
    }

    info!("Received bootup frame!");

    info!("Resetting test mode...");
    configure(&mut tx, &mut rx, Messages::TestModeCommand(0)).await;

    info!("Configuring Modes of Operation...");
    configure(&mut tx, &mut rx, Messages::ModesOfOperation).await;

    info!("Configuring Sensor Type...");
    configure(&mut tx, &mut rx, Messages::SensorType).await;

    info!("Configuring Undervoltage Limit...");
    configure(&mut tx, &mut rx, Messages::UndervoltageLimit).await;

    info!("Configuring Test Stepper frequency...");
    configure(&mut tx, &mut rx, Messages::TestStepperFrequency).await;

    info!("Configuring Test Stepper Enable...");
    configure(&mut tx, &mut rx, Messages::TestStepperEnable).await;

    info!("Configuring maximum controller current...");
    configure(&mut tx, &mut rx, Messages::SetMaxCurrent).await;

    info!("Configuring secondary current protection...");
    configure(&mut tx, &mut rx, Messages::SecondaryCurrentProtection).await;

    info!("Configuring motor rated current...");
    configure(&mut tx, &mut rx, Messages::MotorRatedCurrent).await;

    info!("Configuring overvoltage limit...");
    configure(&mut tx, &mut rx, Messages::OvervoltageLimit).await;

    info!("CONFIG COMPLETE");

    unwrap!(spawner.spawn(rx_task(rx)));
    Timer::after(Duration::from_secs(5)).await;

    info!("Sending Preoperational message...");
    send_motor_message(&mut tx, Messages::EnterPreoperationalState).await;
    Timer::after(Duration::from_secs(30)).await;
    wait_after_send().await;

    info!("Sending Operational message...");
    send_motor_message(&mut tx, Messages::EnterOperationalState).await;
    wait_after_send().await;

    info!("Sending Shutdown message...");
    send_motor_message(&mut tx, Messages::Shutdown).await;
    wait_after_send().await;

    info!("Sending Switch On message...");
    send_motor_message(&mut tx, Messages::SwitchOn).await;
    wait_after_send().await;
    Timer::after(Duration::from_secs(30)).await;

    info!("Sending Start Drive message...");
    send_motor_message(&mut tx, Messages::StartDrive).await;
    Timer::after(Duration::from_secs(15)).await;

    info!("Starting motor");
    info!("Incrementing Test Mode Command to 1000...");

    info!("Setting Test Mode Command to 200");
    send_motor_message(&mut tx, Messages::TestModeCommand(200)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Setting Test Mode Command to 400");
    send_motor_message(&mut tx, Messages::TestModeCommand(400)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Setting Test Mode Command to 600");
    send_motor_message(&mut tx, Messages::TestModeCommand(600)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Setting Test Mode Command to 800");
    send_motor_message(&mut tx, Messages::TestModeCommand(800)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Setting Test Mode Command to 1000");
    send_motor_message(&mut tx, Messages::TestModeCommand(1000)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Ramping down Test Mode Command to 0...");
    send_motor_message(&mut tx, Messages::TestModeCommand(500)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Stopping motor...");
    send_motor_message(&mut tx, Messages::EnterStopState).await;

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task]
async fn rx_task(mut rx: CanRx<'static>) {
    loop {
        match rx.read().await {
            Ok(envelope) => {
                let frame = envelope.frame;
                let raw_id: u32 = match frame.id() {
                    embassy_stm32::can::Id::Standard(id) => id.as_raw() as u32,
                    embassy_stm32::can::Id::Extended(id) => id.as_raw(),
                };
                info!(
                    "Received id={=u32:x} data={=[u8]:#02x}",
                    raw_id,
                    frame.data()
                );
            }
            Err(e) => {
                warn!("CAN receive error: {:?}", e);
                continue;
            }
        }
    }
}

const SDO_RX_ID: u16 = 0x580 + NODE_ID as u16;
const SDO_TX_ID: u32 = 0x600 + NODE_ID as u32;
const SDO_UPLOAD_REQUEST: u8 = 0x40;
const SDO_DOWNLOAD_RESPONSE: u8 = 0x60;
const SDO_ABORT: u8 = 0x80;
const SDO_RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);

/// Write an SDO, assert the controller acknowledged it, then read the object back and verify it
/// holds the value that was written.
async fn configure(tx: &mut CanTx<'static>, rx: &mut CanRx<'static>, msg: Messages) {
    let write: CanOpenMessage = msg.into();

    send_sdo(tx, write.clone()).await;
    let ack = read_sdo_response(rx).await;
    check_sdo_object(&write, &ack);
    defmt::assert_eq!(
        ack.command,
        SDO_DOWNLOAD_RESPONSE,
        "Unexpected download response command for {=u16:#06x}:{=u8}",
        write.index,
        write.sub_index
    );

    send_sdo(
        tx,
        CanOpenMessage {
            id: SDO_TX_ID,
            command: SDO_UPLOAD_REQUEST,
            index: write.index,
            sub_index: write.sub_index,
            data: 0,
        },
    )
    .await;
    let read = read_sdo_response(rx).await;
    check_sdo_object(&write, &read);

    // An expedited upload response mirrors the download command with bit 6 set instead of bit 5
    // (0x2F -> 0x4F, 0x2B -> 0x4B, 0x23 -> 0x43), so the sizes match iff this holds
    let expected_command = write.command + 0x20;
    defmt::assert_eq!(
        read.command,
        expected_command,
        "Unexpected upload response command for {=u16:#06x}:{=u8}",
        write.index,
        write.sub_index
    );

    let unused_bytes = (write.command >> 2) & 0x03;
    let mask = u32::MAX >> (8 * unused_bytes as u32);
    defmt::assert_eq!(
        read.data & mask,
        write.data & mask,
        "Readback mismatch for {=u16:#06x}:{=u8}",
        write.index,
        write.sub_index
    );

    info!(
        "Verified {=u16:#06x}:{=u8} = {=u32:#x}",
        write.index,
        write.sub_index,
        read.data & mask
    );
}

/// Panic if the response is an abort or refers to a different object than the request
fn check_sdo_object(request: &CanOpenMessage, response: &CanOpenMessage) {
    if response.command == SDO_ABORT {
        defmt::panic!(
            "SDO abort for {=u16:#06x}:{=u8}: code {=u32:#010x}",
            response.index,
            response.sub_index,
            response.data
        );
    }
    defmt::assert_eq!(response.index, request.index, "SDO response index mismatch");
    defmt::assert_eq!(
        response.sub_index,
        request.sub_index,
        "SDO response sub-index mismatch"
    );
}

/// Wait for the next SDO response from the motor controller, skipping any other frames
async fn read_sdo_response(rx: &mut CanRx<'static>) -> CanOpenMessage {
    loop {
        let envelope = match with_timeout(SDO_RESPONSE_TIMEOUT, rx.read()).await {
            Ok(Ok(envelope)) => envelope,
            Ok(Err(e)) => {
                warn!("CAN receive error: {:?}", e);
                continue;
            }
            Err(_) => defmt::panic!("Timed out waiting for SDO response"),
        };

        let frame = envelope.frame;
        match frame.id() {
            embassy_stm32::can::Id::Standard(id) if id.as_raw() == SDO_RX_ID => {
                let data = frame.data();
                defmt::assert_eq!(data.len(), 8, "SDO response must be 8 bytes");
                return CanOpenMessage {
                    id: SDO_RX_ID as u32,
                    command: data[0],
                    index: u16::from_le_bytes([data[1], data[2]]),
                    sub_index: data[3],
                    data: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
                };
            }
            embassy_stm32::can::Id::Standard(id) => {
                info!(
                    "Skipping id={=u16:x} data={=[u8]:#02x}",
                    id.as_raw(),
                    frame.data()
                );
            }
            embassy_stm32::can::Id::Extended(id) => {
                info!(
                    "Skipping id={=u32:x} data={=[u8]:#02x}",
                    id.as_raw(),
                    frame.data()
                );
            }
        }
    }
}

async fn wait_after_send() {
    Timer::after(Duration::from_secs(1)).await;
}

async fn send_nmt(tx: &mut CanTx<'static>, command: u8, node_id: u8) {
    let id = embassy_stm32::can::Id::Standard(unwrap!(embassy_stm32::can::StandardId::new(0x000)));

    let data = [command, node_id];
    let frame = embassy_stm32::can::Frame::new_data(id, &data).unwrap();
    tx.write(&frame).await;
}

async fn send_sdo(tx: &mut CanTx<'static>, msg: CanOpenMessage) {
    let hyped_frame: HypedCanFrame = msg.into();

    let id = if hyped_frame.can_id <= 0x7FF {
        embassy_stm32::can::Id::Standard(unwrap!(embassy_stm32::can::StandardId::new(
            hyped_frame.can_id as u16
        )))
    } else {
        embassy_stm32::can::Id::Extended(unwrap!(embassy_stm32::can::ExtendedId::new(
            hyped_frame.can_id
        )))
    };

    let frame = embassy_stm32::can::Frame::new_data(id, &hyped_frame.data).unwrap();
    tx.write(&frame).await;
}

async fn send_motor_message(tx: &mut CanTx<'static>, msg: Messages) {
    match msg {
        Messages::EnterStopState => send_nmt(tx, 0x02, NODE_ID).await,
        Messages::EnterPreoperationalState => send_nmt(tx, 0x80, NODE_ID).await,
        Messages::EnterOperationalState => send_nmt(tx, 0x01, NODE_ID).await,
        other => {
            let can_open_msg: CanOpenMessage = other.into();
            send_sdo(tx, can_open_msg).await;
        }
    }
}
