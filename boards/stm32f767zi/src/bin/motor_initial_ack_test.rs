#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    can::{
        filter::Mask32, Can, CanRx, CanTx, Fifo, Frame, Rx0InterruptHandler, Rx1InterruptHandler,
        SceInterruptHandler, TxInterruptHandler,
    },
    peripherals::CAN1,
};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
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

    unwrap!(spawner.spawn(can_receiver(rx)));
    let frames = FRAMES.receiver();

    // TODO: ask what this is before testing
    info!("Resetting test mode...");
    configure(&mut tx, &frames, Messages::TestModeCommand(0)).await;

    info!("Configuring Modes of Operation...");
    configure(&mut tx, &frames, Messages::ModesOfOperation).await;

    info!("Configuring Sensor Type...");
    configure(&mut tx, &frames, Messages::SensorType).await;

    info!("Configuring Undervoltage Limit...");
    configure(&mut tx, &frames, Messages::UndervoltageLimit).await;

    info!("Configuring Test Stepper frequency...");
    configure(&mut tx, &frames, Messages::TestStepperFrequency).await;

    info!("Configuring Test Stepper Enable...");
    configure(&mut tx, &frames, Messages::TestStepperEnable).await;

    info!("Configuring maximum controller current...");
    configure(&mut tx, &frames, Messages::SetMaxCurrent).await;

    info!("Configuring secondary current protection...");
    configure(&mut tx, &frames, Messages::SecondaryCurrentProtection).await;

    info!("Configuring motor rated current...");
    configure(&mut tx, &frames, Messages::MotorRatedCurrent).await;

    info!("Configuring overvoltage limit...");
    configure(&mut tx, &frames, Messages::OvervoltageLimit).await;

    info!("CONFIG COMPLETE");

    info!("Checking for heartbeat producer object (0x1017:0)...");
    let heartbeat = sdo_upload(&mut tx, &frames, HEARTBEAT_TIME_INDEX, 0).await;
    if heartbeat.command == SDO_ABORT {
        info!(
            "0x1017:0 does not exist: abort code {=u32:#010x}",
            heartbeat.data
        );
    } else {
        info!(
            "0x1017:0 exists: producer heartbeat time = {=u32} ms",
            heartbeat.data & 0xFFFF
        );
        info!("Configuring heartbeat to {=u32} ms...", HEARTBEAT_TIME_MS);
        configure(
            &mut tx,
            &frames,
            CanOpenMessage {
                id: SDO_TX_ID,
                command: 0x2B,
                index: HEARTBEAT_TIME_INDEX,
                sub_index: 0x00,
                data: HEARTBEAT_TIME_MS,
            },
        )
        .await;
    }

    unwrap!(spawner.spawn(frame_logger(frames)));
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

type FrameReceiver = Receiver<'static, CriticalSectionRawMutex, Frame, 16>;

/// Non-heartbeat frames received from the motor controller
static FRAMES: Channel<CriticalSectionRawMutex, Frame, 16> = Channel::new();

/// Reads every frame off the bus, logging heartbeats and forwarding everything else to `FRAMES`
#[embassy_executor::task]
async fn can_receiver(mut rx: CanRx<'static>) {
    loop {
        match rx.read().await {
            Ok(envelope) => {
                let frame = envelope.frame;
                match frame.id() {
                    embassy_stm32::can::Id::Standard(id) if id.as_raw() == HEARTBEAT_ID => {
                        log_heartbeat(frame.data());
                    }
                    _ => FRAMES.send(frame).await,
                }
            }
            Err(e) => warn!("CAN receive error: {:?}", e),
        }
    }
}

/// Logs non-heartbeat frames once the config phase no longer consumes them
#[embassy_executor::task]
async fn frame_logger(frames: FrameReceiver) {
    loop {
        log_frame("Received", &frames.receive().await);
    }
}

fn log_frame(prefix: &str, frame: &Frame) {
    let raw_id: u32 = match frame.id() {
        embassy_stm32::can::Id::Standard(id) => id.as_raw() as u32,
        embassy_stm32::can::Id::Extended(id) => id.as_raw(),
    };
    info!(
        "{=str} id={=u32:x} data={=[u8]:#02x}",
        prefix,
        raw_id,
        frame.data()
    );
}

/// Log a heartbeat frame: 1 byte, bit 7 reserved, bits 6..0 the NMT state
fn log_heartbeat(data: &[u8]) {
    if data.len() != 1 {
        warn!(
            "Heartbeat with unexpected DLC {=usize}: {=[u8]:#02x}",
            data.len(),
            data
        );
    }
    let Some(&byte) = data.first() else {
        return;
    };
    let reserved = byte >> 7;
    let state = byte & 0x7F;
    let name = match state {
        0x00 => "Boot-up",
        0x04 => "Stopped",
        0x05 => "Operational",
        0x7F => "Pre-operational",
        _ => "Unknown",
    };
    info!(
        "Heartbeat: state={=u8:#04x} ({=str}) r={=u8}",
        state, name, reserved
    );
}

const SDO_RX_ID: u16 = 0x580 + NODE_ID as u16;
const HEARTBEAT_ID: u16 = 0x700 + NODE_ID as u16;
const SDO_TX_ID: u32 = 0x600 + NODE_ID as u32;
const SDO_UPLOAD_REQUEST: u8 = 0x40;
const SDO_DOWNLOAD_RESPONSE: u8 = 0x60;
const SDO_ABORT: u8 = 0x80;
const SDO_RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);
const HEARTBEAT_TIME_INDEX: u16 = 0x1017;
const HEARTBEAT_TIME_MS: u32 = 100;

/// Write an SDO, assert the controller acknowledged it, then read the object back and verify it
/// holds the value that was written.
async fn configure(
    tx: &mut CanTx<'static>,
    frames: &FrameReceiver,
    msg: impl Into<CanOpenMessage>,
) {
    let write: CanOpenMessage = msg.into();

    send_sdo(tx, write.clone()).await;
    let ack = read_sdo_response(frames, write.index, write.sub_index).await;
    assert_not_abort(&ack);
    defmt::assert_eq!(
        ack.command,
        SDO_DOWNLOAD_RESPONSE,
        "Unexpected download response command for {=u16:#06x}:{=u8}",
        write.index,
        write.sub_index
    );

    let read = sdo_upload(tx, frames, write.index, write.sub_index).await;
    assert_not_abort(&read);

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

/// Request an object from the controller. The response may be an abort, which the caller handles.
async fn sdo_upload(
    tx: &mut CanTx<'static>,
    frames: &FrameReceiver,
    index: u16,
    sub_index: u8,
) -> CanOpenMessage {
    send_sdo(
        tx,
        CanOpenMessage {
            id: SDO_TX_ID,
            command: SDO_UPLOAD_REQUEST,
            index,
            sub_index,
            data: 0,
        },
    )
    .await;
    read_sdo_response(frames, index, sub_index).await
}

fn assert_not_abort(response: &CanOpenMessage) {
    if response.command == SDO_ABORT {
        defmt::panic!(
            "SDO abort for {=u16:#06x}:{=u8}: code {=u32:#010x}",
            response.index,
            response.sub_index,
            response.data
        );
    }
}

/// Wait for the next SDO response, skipping other frames, and assert it refers to the given object
async fn read_sdo_response(frames: &FrameReceiver, index: u16, sub_index: u8) -> CanOpenMessage {
    loop {
        let Ok(frame) = with_timeout(SDO_RESPONSE_TIMEOUT, frames.receive()).await else {
            defmt::panic!("Timed out waiting for SDO response");
        };

        match frame.id() {
            embassy_stm32::can::Id::Standard(id) if id.as_raw() == SDO_RX_ID => {
                let data = frame.data();
                defmt::assert_eq!(data.len(), 8, "SDO response must be 8 bytes");
                let response = CanOpenMessage {
                    id: SDO_RX_ID as u32,
                    command: data[0],
                    index: u16::from_le_bytes([data[1], data[2]]),
                    sub_index: data[3],
                    data: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
                };
                defmt::assert_eq!(response.index, index, "SDO response index mismatch");
                defmt::assert_eq!(
                    response.sub_index,
                    sub_index,
                    "SDO response sub-index mismatch"
                );
                return response;
            }
            _ => log_frame("Skipping", &frame),
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
