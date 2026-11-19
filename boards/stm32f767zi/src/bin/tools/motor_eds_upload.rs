#![no_std]
#![no_main]

//! Uploads the EDS file stored on the motor controller (object 0x1021 "Store EDS")
//! using an SDO upload (initiate + segmented transfer), and prints the raw bytes over defmt
//! as a single `EDS = [0x.., ...]` array split over several lines. The data may be binary (e.g. zipped, see 0x1022), so nothing is
//! interpreted on the device; reassemble it on the host with `scripts/decode_eds.py`.

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    can::{
        filter::Mask32, Can, CanRx, CanTx, Fifo, Frame, Id, Rx0InterruptHandler,
        Rx1InterruptHandler, SceInterruptHandler, StandardId, TxInterruptHandler,
    },
    peripherals::CAN1,
};
use embassy_time::{with_timeout, Duration};
use core::fmt::Write;
use heapless::{String, Vec};
use panic_probe as _;

const NODE_ID: u8 = 0x01;
const SDO_TX_ID: u16 = 0x600 + NODE_ID as u16; // client -> server
const SDO_RX_ID: u16 = 0x580 + NODE_ID as u16; // server -> client

const STORE_EDS_INDEX: u16 = 0x1021;
const STORAGE_FORMAT_INDEX: u16 = 0x1022;

const SDO_TIMEOUT: Duration = Duration::from_millis(1000);
const ABORT_TIMEOUT: u32 = 0x0504_0000;
const ABORT_INVALID_CS: u32 = 0x0504_0001;
const ABORT_TOGGLE: u32 = 0x0503_0000;

const MAX_CHUNK_LEN: usize = 64;
/// "0xNN, " per byte
const MAX_CHUNK_STR_LEN: usize = MAX_CHUNK_LEN * 6;

bind_interrupts!(struct Irqs {
    CAN1_RX0 => Rx0InterruptHandler<CAN1>;
    CAN1_RX1 => Rx1InterruptHandler<CAN1>;
    CAN1_SCE => SceInterruptHandler<CAN1>;
    CAN1_TX => TxInterruptHandler<CAN1>;
});

#[derive(Format)]
enum SdoError {
    Timeout,
    /// Server aborted the transfer with the given abort code
    Aborted(u32),
    UnexpectedResponse(u8),
    ToggleMismatch,
}

/// Result of an SDO upload initiate
enum InitiateResponse {
    /// Expedited transfer: data fits in the initiate response
    Expedited { data: [u8; 4], len: usize },
    /// Segmented transfer follows, optionally with the total size
    Segmented { size: Option<u32> },
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    info!("Setting up CAN1 (main bus)...");
    let mut can = Can::new(p.CAN1, p.PD0, p.PD1, Irqs);

    can.modify_filters()
        .enable_bank(0, Fifo::Fifo0, Mask32::accept_all());
    can.modify_config().set_bitrate(500_000);
    can.enable().await;

    info!("CAN1 enabled");

    let (mut tx, mut rx) = can.split();

    // 0x1022 tells us how the EDS in 0x1021 is stored (0 = ASCII, otherwise vendor-specific, e.g. zipped)
    info!("Reading EDS storage format (0x1022:00)...");
    match upload_initiate(&mut tx, &mut rx, STORAGE_FORMAT_INDEX, 0x00).await {
        Ok(InitiateResponse::Expedited { data, .. }) => {
            info!("EDS storage format = {=u8}", data[0]);
            if data[0] != 0 {
                warn!("EDS is not stored as plain ASCII, output will be raw bytes");
            }
        }
        Ok(InitiateResponse::Segmented { .. }) => warn!("Unexpected segmented response for 0x1022"),
        Err(e) => warn!("Could not read 0x1022: {}", e),
    }

    info!("Uploading EDS (0x1021:00)...");
    match upload_eds(&mut tx, &mut rx).await {
        Ok(total) => info!("EDS UPLOAD COMPLETE ({=u32} bytes)", total),
        Err(e) => error!("EDS upload failed: {}", e),
    }

    loop {
        embassy_time::Timer::after(Duration::from_secs(1)).await;
    }
}

/// Uploads 0x1021:00 and prints it as a single hex array. Returns the number of bytes received.
async fn upload_eds(tx: &mut CanTx<'_>, rx: &mut CanRx<'_>) -> Result<u32, SdoError> {
    let mut chunk: Vec<u8, MAX_CHUNK_LEN> = Vec::new();
    let mut total: u32 = 0;

    match upload_initiate(tx, rx, STORE_EDS_INDEX, 0x00).await? {
        InitiateResponse::Expedited { data, len } => {
            println!("EDS = [");
            for &b in &data[..len] {
                push_byte(&mut chunk, b);
            }
            flush_chunk(&mut chunk);
            println!("]");
            return Ok(len as u32);
        }
        InitiateResponse::Segmented { size } => match size {
            Some(size) => info!("EDS size: {=u32} bytes", size),
            None => info!("EDS size not indicated"),
        },
    }

    println!("EDS = [");

    let mut toggle = false;
    loop {
        // Upload segment request: ccs = 3, toggle bit in bit 4
        let request = [0x60 | ((toggle as u8) << 4), 0, 0, 0, 0, 0, 0, 0];
        let response = sdo_request(tx, rx, &request).await?;

        let cmd = response[0];
        let scs = cmd >> 5;
        if scs != 0 {
            abort(tx, STORE_EDS_INDEX, 0x00, ABORT_INVALID_CS).await;
            return Err(SdoError::UnexpectedResponse(cmd));
        }
        if ((cmd >> 4) & 1 == 1) != toggle {
            abort(tx, STORE_EDS_INDEX, 0x00, ABORT_TOGGLE).await;
            return Err(SdoError::ToggleMismatch);
        }

        let unused = ((cmd >> 1) & 0x07) as usize;
        let last = cmd & 0x01 == 1;

        for &b in &response[1..8 - unused] {
            push_byte(&mut chunk, b);
        }
        total += (7 - unused) as u32;

        if last {
            flush_chunk(&mut chunk);
            println!("]");
            return Ok(total);
        }
        toggle = !toggle;
    }
}

/// Sends an SDO upload initiate request for `index:sub_index`.
async fn upload_initiate(
    tx: &mut CanTx<'_>,
    rx: &mut CanRx<'_>,
    index: u16,
    sub_index: u8,
) -> Result<InitiateResponse, SdoError> {
    let [idx_lo, idx_hi] = index.to_le_bytes();
    let request = [0x40, idx_lo, idx_hi, sub_index, 0, 0, 0, 0];
    let response = sdo_request(tx, rx, &request).await?;

    let cmd = response[0];
    if cmd >> 5 != 2 {
        abort(tx, index, sub_index, ABORT_INVALID_CS).await;
        return Err(SdoError::UnexpectedResponse(cmd));
    }

    let expedited = (cmd >> 1) & 1 == 1;
    let size_indicated = cmd & 1 == 1;
    let payload = [response[4], response[5], response[6], response[7]];

    if expedited {
        let len = if size_indicated {
            4 - ((cmd >> 2) & 0x03) as usize
        } else {
            4
        };
        Ok(InitiateResponse::Expedited { data: payload, len })
    } else {
        Ok(InitiateResponse::Segmented {
            size: size_indicated.then(|| u32::from_le_bytes(payload)),
        })
    }
}

/// Sends an SDO frame and waits for the matching server response, handling aborts.
async fn sdo_request(
    tx: &mut CanTx<'_>,
    rx: &mut CanRx<'_>,
    data: &[u8; 8],
) -> Result<[u8; 8], SdoError> {
    let id = Id::Standard(unwrap!(StandardId::new(SDO_TX_ID)));
    tx.write(&unwrap!(Frame::new_data(id, data))).await;

    let response = match with_timeout(SDO_TIMEOUT, wait_for_sdo_response(rx)).await {
        Ok(r) => r,
        Err(_) => {
            let index = u16::from_le_bytes([data[1], data[2]]);
            abort(tx, index, data[3], ABORT_TIMEOUT).await;
            return Err(SdoError::Timeout);
        }
    };

    if response[0] == 0x80 {
        let code = u32::from_le_bytes([response[4], response[5], response[6], response[7]]);
        return Err(SdoError::Aborted(code));
    }
    Ok(response)
}

/// Waits for the next frame on the SDO response COB-ID, ignoring all other traffic.
async fn wait_for_sdo_response(rx: &mut CanRx<'_>) -> [u8; 8] {
    loop {
        match rx.read().await {
            Ok(envelope) => {
                let frame = envelope.frame;
                let raw_id: u32 = match frame.id() {
                    Id::Standard(id) => id.as_raw() as u32,
                    Id::Extended(id) => id.as_raw(),
                };
                if raw_id != SDO_RX_ID as u32 {
                    continue;
                }
                let mut data = [0u8; 8];
                let len = frame.data().len().min(8);
                data[..len].copy_from_slice(&frame.data()[..len]);
                return data;
            }
            Err(e) => warn!("CAN receive error: {:?}", e),
        }
    }
}

async fn abort(tx: &mut CanTx<'_>, index: u16, sub_index: u8, code: u32) {
    let [idx_lo, idx_hi] = index.to_le_bytes();
    let [c0, c1, c2, c3] = code.to_le_bytes();
    let data = [0x80, idx_lo, idx_hi, sub_index, c0, c1, c2, c3];
    let id = Id::Standard(unwrap!(StandardId::new(SDO_TX_ID)));
    tx.write(&unwrap!(Frame::new_data(id, &data))).await;
}

/// Appends a byte to the chunk buffer, printing the chunk when the buffer is full.
fn push_byte(chunk: &mut Vec<u8, MAX_CHUNK_LEN>, b: u8) {
    if chunk.push(b).is_err() {
        flush_chunk(chunk);
        let _ = chunk.push(b);
    }
}

fn flush_chunk(chunk: &mut Vec<u8, MAX_CHUNK_LEN>) {
    if chunk.is_empty() {
        return;
    }
    // Print the bytes without brackets so consecutive lines join into one array
    let mut line: String<MAX_CHUNK_STR_LEN> = String::new();
    for &b in chunk.iter() {
        let _ = core::write!(line, "0x{:02x}, ", b);
    }
    println!("{=str}", line.as_str());
    chunk.clear();
}
