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
    mutex::Mutex,
};
use embassy_time::{with_timeout, Duration, Timer};
use hyped_can::HypedCanFrame;
use hyped_motors::{can_open_message::CanOpenMessage, can_open_processor::Messages};
use panic_probe as _;
use static_cell::StaticCell;

use core::sync::atomic::{AtomicBool, Ordering};

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

    let (tx, rx) = can.split();
    static TX: StaticCell<Mutex<CriticalSectionRawMutex, CanTx<'static>>> = StaticCell::new();
    let tx: SharedTx = TX.init(Mutex::new(tx));
    //
    // match rx.read().await {
    //     Ok(envelope) => {
    //         let frame = envelope.frame;
    //         match frame.id() {
    //             embassy_stm32::can::Id::Standard(id) => {
    //                 const BOOT_UP_ID: u16 = 0x700;
    //                 defmt::assert_eq!(id.as_raw(), BOOT_UP_ID + NODE_ID as u16);
    //             }
    //             embassy_stm32::can::Id::Extended(id) => {
    //                 defmt::panic!(
    //                     "Bad bootup frame id: Expected standard, received extended: {}",
    //                     id.as_raw()
    //                 )
    //             }
    //         }
    //     }
    //     Err(e) => {
    //         error!("{}", e);
    //     }
    // }
    //
    info!("Received bootup frame!");

    unwrap!(spawner.spawn(can_receiver(rx)));
    let frames = FRAMES.receiver();

    info!("Sending Preoperational message...");
    send_motor_message(tx, Messages::EnterPreoperationalState).await;
    Timer::after(Duration::from_secs(1)).await;

    // TODO: ask what this is before testing
    info!("Resetting test mode...");
    configure(tx, &frames, Messages::TestModeCommand(0)).await;

    info!("Configuring Modes of Operation...");
    configure(tx, &frames, Messages::ModesOfOperation).await;

    info!("Configuring Sensor Type...");
    configure(tx, &frames, Messages::SensorType).await;

    info!("Configuring Undervoltage Limit...");
    configure(tx, &frames, Messages::UndervoltageLimit).await;

    info!("Configuring Test Stepper frequency...");
    configure(tx, &frames, Messages::TestStepperFrequency).await;

    info!("Configuring Test Stepper Enable...");
    configure(tx, &frames, Messages::TestStepperEnable).await;

    info!("Configuring maximum controller current...");
    configure(tx, &frames, Messages::SetMaxCurrent).await;

    info!("Configuring secondary current protection...");
    configure(tx, &frames, Messages::SecondaryCurrentProtection).await;

    info!("Configuring motor rated current...");
    configure(tx, &frames, Messages::MotorRatedCurrent).await;

    info!("Configuring overvoltage limit...");
    configure(tx, &frames, Messages::OvervoltageLimit).await;

    info!("Configuring heartbeat to {=u32} ms...", HEARTBEAT_TIME_MS);
    configure(
        tx,
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

    info!("CONFIG COMPLETE");

    info!("Sending Operational message...");
    send_motor_message(tx, Messages::EnterOperationalState).await;
    LOG_ALL_FRAMES.store(true, Ordering::Relaxed);

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}

type FrameReceiver = Receiver<'static, CriticalSectionRawMutex, Frame, 16>;

/// Non-heartbeat frames received from the motor controller
static FRAMES: Channel<CriticalSectionRawMutex, Frame, 16> = Channel::new();

/// Set once the drive is sent to operational, after which nothing consumes `FRAMES`
static LOG_ALL_FRAMES: AtomicBool = AtomicBool::new(false);

/// Reads every frame off the bus, logging heartbeats. Other frames are forwarded to `FRAMES` for
/// the config phase, then logged once `LOG_ALL_FRAMES` is set.
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
                    _ if LOG_ALL_FRAMES.load(Ordering::Relaxed) => log_frame("Received", &frame),
                    _ => FRAMES.send(frame).await,
                }
            }
            Err(e) => warn!("CAN receive error: {:?}", e),
        }
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

type SharedTx = &'static Mutex<CriticalSectionRawMutex, CanTx<'static>>;

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
async fn configure(tx: SharedTx, frames: &FrameReceiver, msg: impl Into<CanOpenMessage>) {
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
    tx: SharedTx,
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

async fn send_nmt(tx: SharedTx, command: u8, node_id: u8) {
    let id = embassy_stm32::can::Id::Standard(unwrap!(embassy_stm32::can::StandardId::new(0x000)));

    let data = [command, node_id];
    let frame = embassy_stm32::can::Frame::new_data(id, &data).unwrap();
    tx.lock().await.write(&frame).await;
}

async fn send_sdo(tx: SharedTx, msg: CanOpenMessage) {
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
    tx.lock().await.write(&frame).await;
}

async fn send_motor_message(tx: SharedTx, msg: Messages) {
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
