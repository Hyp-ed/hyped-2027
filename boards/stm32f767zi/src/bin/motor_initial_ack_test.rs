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
use embassy_time::{with_timeout, Duration, Instant, Ticker, Timer};
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

    let (tx, mut rx) = can.split();
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

    info!("Configuring TPDO 0...");
    configure_tpdo0(tx, &frames).await;

    let sync_cob = sdo_upload(tx, &frames, SYNC_COB_ID_INDEX, 0x00).await;
    if sync_cob.command == SDO_ABORT {
        warn!(
            "0x1005 (SYNC COB-ID) not readable: abort code {=u32:#010x}",
            sync_cob.data
        );
    } else {
        info!(
            "Drive consumes SYNC on COB-ID {=u32:#x}",
            sync_cob.data & 0x7FF
        );
    }
    unwrap!(spawner.spawn(sync_producer(tx)));

    info!("CONFIG COMPLETE");

    info!("Checking for heartbeat producer object (0x1017:0)...");
    let heartbeat = sdo_upload(tx, &frames, HEARTBEAT_TIME_INDEX, 0).await;
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
    }

    Timer::after(Duration::from_secs(5)).await;

    // The controller is already pre-operational after sending its bootup frame
    // info!("Sending Preoperational message...");
    // send_motor_message(tx, Messages::EnterPreoperationalState).await;
    // Timer::after(Duration::from_secs(30)).await;
    // wait_after_send().await;

    info!("Checking for statusword object (0x6041:0)...");
    let status = sdo_upload(tx, &frames, STATUSWORD_INDEX, 0).await;
    if status.command == SDO_ABORT {
        error!(
            "0x6041:0 does not exist: abort code {=u32:#010x}. Not enabling the drive.",
            status.data
        );
        loop {
            Timer::after(Duration::from_secs(1)).await;
        }
    }
    let statusword = status.data as u16;
    info!(
        "0x6041:0 exists: statusword={=u16:#06x} ({})",
        statusword,
        DriveState::decode(statusword)
    );

    info!("Sending Operational message...");
    send_motor_message(tx, Messages::EnterOperationalState).await;
    wait_after_send().await;

    if DriveState::decode(statusword) == DriveState::Fault {
        info!("Drive is faulted, sending Fault Reset...");
        set_controlword(tx, &frames, CONTROLWORD_FAULT_RESET).await;
        wait_for_state(tx, &frames, DriveState::SwitchOnDisabled).await;
    }

    Timer::after(Duration::from_secs(60)).await;

    info!("Sending Shutdown message...");
    set_controlword(tx, &frames, Messages::Shutdown).await;
    wait_for_state(tx, &frames, DriveState::ReadyToSwitchOn).await;

    info!("Sending Switch On message...");
    set_controlword(tx, &frames, Messages::SwitchOn).await;
    wait_for_state(tx, &frames, DriveState::SwitchedOn).await;

    info!("Sending Start Drive message...");
    set_controlword(tx, &frames, Messages::StartDrive).await;
    wait_for_state(tx, &frames, DriveState::OperationEnabled).await;

    unwrap!(spawner.spawn(frame_logger(frames)));

    info!("Starting motor");
    info!("Incrementing Test Mode Command to 1000...");

    info!("Setting Test Mode Command to 200");
    send_motor_message(tx, Messages::TestModeCommand(200)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Setting Test Mode Command to 400");
    send_motor_message(tx, Messages::TestModeCommand(400)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Setting Test Mode Command to 600");
    send_motor_message(tx, Messages::TestModeCommand(600)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Setting Test Mode Command to 800");
    send_motor_message(tx, Messages::TestModeCommand(800)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Setting Test Mode Command to 1000");
    send_motor_message(tx, Messages::TestModeCommand(1000)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Ramping down Test Mode Command to 0...");
    send_motor_message(tx, Messages::TestModeCommand(500)).await;
    Timer::after(Duration::from_secs(3)).await;

    info!("Stopping motor...");
    send_motor_message(tx, Messages::EnterStopState).await;

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
                    embassy_stm32::can::Id::Standard(id) if id.as_raw() == TPDO0_COB_ID => {
                        log_tpdo0(frame.data());
                    }
                    embassy_stm32::can::Id::Standard(id)
                        if TPDO_DEFAULT_COB_IDS[1..].contains(&id.as_raw()) =>
                    {
                        log_frame("Unmapped TPDO", &frame);
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

const TPDO_COMM_INDEX: u16 = 0x1800;
const TPDO_MAPPING_INDEX: u16 = 0x1A00;
/// Default COB-IDs of TPDO 0-3
const TPDO_DEFAULT_COB_IDS: [u16; 4] = [
    0x180 + NODE_ID as u16,
    0x280 + NODE_ID as u16,
    0x380 + NODE_ID as u16,
    0x480 + NODE_ID as u16,
];
const TPDO0_COB_ID: u16 = TPDO_DEFAULT_COB_IDS[0];
/// CiA 301 COB-ID bit 31: set means the PDO is *not* valid (disabled)
const PDO_COB_ID_INVALID: u32 = 1 << 31;
/// Synchronous cyclic: sent once after every SYNC
const TPDO0_TRANSMISSION_TYPE: u32 = 0x01;

/// (index, sub-index, size in bits) of each object mapped into TPDO 0, in payload order
const TPDO0_MAPPING: [(u16, u8, u8); 4] = [
    (0x2026, 0x01, 8),  // controller temperature
    (0x2026, 0x02, 8),  // capacitor temperature
    (0x6079, 0x00, 16), // DC link voltage
    (0x606C, 0x00, 32), // velocity
];

/// Build an expedited SDO download of `size` bytes
fn sdo_write(index: u16, sub_index: u8, size: u8, data: u32) -> CanOpenMessage {
    let command = match size {
        1 => 0x2F,
        2 => 0x2B,
        3 => 0x27,
        4 => 0x23,
        _ => defmt::panic!("Invalid expedited SDO size {=u8}", size),
    };
    CanOpenMessage {
        id: SDO_TX_ID,
        command,
        index,
        sub_index,
        data,
    }
}

/// Check that the TPDO 0 communication and mapping objects exist, then map `TPDO0_MAPPING`
/// into it and enable it. Logs and skips the setup if either object is missing.
async fn configure_tpdo0(tx: SharedTx, frames: &FrameReceiver) {
    let comm = sdo_upload(tx, frames, TPDO_COMM_INDEX, 0x00).await;
    if comm.command == SDO_ABORT {
        error!(
            "0x1800 does not exist: abort code {=u32:#010x}. Skipping TPDO setup.",
            comm.data
        );
        return;
    }
    let comm_entries = comm.data as u8;
    info!("0x1800 exists with {=u8} entries", comm_entries);

    let mapping = sdo_upload(tx, frames, TPDO_MAPPING_INDEX, 0x00).await;
    if mapping.command == SDO_ABORT {
        error!(
            "0x1A00 does not exist: abort code {=u32:#010x}. Skipping TPDO setup.",
            mapping.data
        );
        return;
    }
    info!(
        "0x1A00 exists with {=u8} mapped objects",
        mapping.data as u8
    );

    let cob = sdo_upload(tx, frames, TPDO_COMM_INDEX, 0x01).await;
    assert_not_abort(&cob);
    info!("TPDO 0 current COB-ID: {=u32:#010x}", cob.data);

    // CiA 301: the PDO must be disabled before its parameters or mapping can change. This drive
    // has 0x2084 (Allow PDO COB-ID change) = 0 and may refuse any COB-ID write, in which case we
    // try to change the mapping with the PDO still enabled.
    let disable: CanOpenMessage =
        sdo_write(TPDO_COMM_INDEX, 0x01, 4, cob.data | PDO_COB_ID_INVALID).into();
    send_sdo(tx, disable.clone()).await;
    let disable_ack = read_sdo_response(frames, disable.index, disable.sub_index).await;
    let disabled = disable_ack.command != SDO_ABORT;
    if !disabled {
        warn!(
            "Drive refused to disable TPDO 0 (abort {=u32:#010x}), changing mapping while enabled",
            disable_ack.data
        );
    }

    configure(
        tx,
        frames,
        sdo_write(TPDO_COMM_INDEX, 0x02, 1, TPDO0_TRANSMISSION_TYPE),
    )
    .await;

    // Mapping count must be 0 while entries are written
    configure(tx, frames, sdo_write(TPDO_MAPPING_INDEX, 0x00, 1, 0)).await;
    let mut total_bits: u32 = 0;
    for (i, &(index, sub_index, bits)) in TPDO0_MAPPING.iter().enumerate() {
        let entry = (u32::from(index) << 16) | (u32::from(sub_index) << 8) | u32::from(bits);
        configure(
            tx,
            frames,
            sdo_write(TPDO_MAPPING_INDEX, i as u8 + 1, 4, entry),
        )
        .await;
        total_bits += u32::from(bits);
    }
    defmt::assert!(total_bits <= 64, "TPDO 0 mapping exceeds 8 bytes");
    configure(
        tx,
        frames,
        sdo_write(TPDO_MAPPING_INDEX, 0x00, 1, TPDO0_MAPPING.len() as u32),
    )
    .await;

    // Re-enable with the drive's own COB-ID, only clearing bit 31
    let enabled_cob = cob.data & !PDO_COB_ID_INVALID;
    if disabled {
        configure(
            tx,
            frames,
            sdo_write(TPDO_COMM_INDEX, 0x01, 4, enabled_cob),
        )
        .await;
    }
    info!("TPDO 0 enabled on COB-ID {=u32:#x}", enabled_cob & 0x7FF);
}

/// Decode a TPDO 0 frame according to `TPDO0_MAPPING`
fn log_tpdo0(data: &[u8]) {
    if data.len() != 8 {
        warn!(
            "TPDO 0 with unexpected DLC {=usize}: {=[u8]:#02x}",
            data.len(),
            data
        );
        return;
    }
    let controller_temp = data[0];
    let capacitor_temp = data[1];
    let dc_link_voltage = u16::from_le_bytes([data[2], data[3]]);
    let velocity = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    info!(
        "TPDO 0: controller_temp={=u8} capacitor_temp={=u8} dc_link_voltage={=u16} velocity={=i32}",
        controller_temp, capacitor_temp, dc_link_voltage, velocity
    );
}

type SharedTx = &'static Mutex<CriticalSectionRawMutex, CanTx<'static>>;

const SYNC_COB_ID: u16 = 0x080;
const SYNC_COB_ID_INDEX: u16 = 0x1005;
const SYNC_PERIOD: Duration = Duration::from_millis(100);

/// Sends a SYNC (COB-ID 0x80, no data) every `SYNC_PERIOD`, which triggers the sync cyclic TPDOs
#[embassy_executor::task]
async fn sync_producer(tx: SharedTx) {
    let id =
        embassy_stm32::can::Id::Standard(unwrap!(embassy_stm32::can::StandardId::new(SYNC_COB_ID)));
    let frame = unwrap!(Frame::new_data(id, &[]));
    let mut ticker = Ticker::every(SYNC_PERIOD);
    loop {
        tx.lock().await.write(&frame).await;
        ticker.next().await;
    }
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

const STATUSWORD_INDEX: u16 = 0x6041;
const STATE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const STATE_TIMEOUT: Duration = Duration::from_secs(5);
const CONTROLWORD_FAULT_RESET: CanOpenMessage = CanOpenMessage {
    id: SDO_TX_ID,
    command: 0x2B,
    index: 0x6040,
    sub_index: 0x00,
    data: 0x0080,
};

/// CiA 402 drive states, decoded from the statusword
#[derive(PartialEq, Clone, Copy, Format)]
enum DriveState {
    NotReadyToSwitchOn,
    SwitchOnDisabled,
    ReadyToSwitchOn,
    SwitchedOn,
    OperationEnabled,
    QuickStopActive,
    FaultReactionActive,
    Fault,
    Unknown,
}

impl DriveState {
    fn decode(statusword: u16) -> Self {
        match statusword & 0x4F {
            0x00 => return Self::NotReadyToSwitchOn,
            0x40 => return Self::SwitchOnDisabled,
            0x0F => return Self::FaultReactionActive,
            0x08 => return Self::Fault,
            _ => {}
        }
        match statusword & 0x6F {
            0x21 => Self::ReadyToSwitchOn,
            0x23 => Self::SwitchedOn,
            0x27 => Self::OperationEnabled,
            0x07 => Self::QuickStopActive,
            _ => Self::Unknown,
        }
    }
}

/// Write the controlword and assert the controller acknowledged it
async fn set_controlword(tx: SharedTx, frames: &FrameReceiver, msg: impl Into<CanOpenMessage>) {
    let write: CanOpenMessage = msg.into();
    send_sdo(tx, write.clone()).await;
    let ack = read_sdo_response(frames, write.index, write.sub_index).await;
    assert_not_abort(&ack);
    defmt::assert_eq!(
        ack.command,
        SDO_DOWNLOAD_RESPONSE,
        "Unexpected controlword download response"
    );
}

async fn read_statusword(tx: SharedTx, frames: &FrameReceiver) -> u16 {
    let read = sdo_upload(tx, frames, STATUSWORD_INDEX, 0).await;
    assert_not_abort(&read);
    read.data as u16
}

/// Poll the statusword until the drive reports `expected`, panicking on fault or timeout
async fn wait_for_state(tx: SharedTx, frames: &FrameReceiver, expected: DriveState) {
    let deadline = Instant::now() + STATE_TIMEOUT;
    loop {
        let statusword = read_statusword(tx, frames).await;
        let state = DriveState::decode(statusword);
        if state == expected {
            info!(
                "Drive state confirmed: {} (statusword={=u16:#06x})",
                state, statusword
            );
            return;
        }
        if matches!(state, DriveState::Fault | DriveState::FaultReactionActive) {
            defmt::panic!(
                "Drive faulted while waiting for {}: statusword={=u16:#06x}",
                expected,
                statusword
            );
        }
        if Instant::now() >= deadline {
            defmt::panic!(
                "Timed out waiting for {}: last state {} (statusword={=u16:#06x})",
                expected,
                state,
                statusword
            );
        }
        Timer::after(STATE_POLL_INTERVAL).await;
    }
}

async fn wait_after_send() {
    Timer::after(Duration::from_secs(1)).await;
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
