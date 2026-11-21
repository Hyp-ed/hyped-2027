/// This tool resets the motor. You can set CMD to either NMT_ENTER_RESET or NMT_ENTER_PRE_OPERATIONAL

#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    can::{
        filter::Mask32, Can, Fifo, Frame, Id, Rx0InterruptHandler, Rx1InterruptHandler,
        SceInterruptHandler, StandardId, TxInterruptHandler,
    },
    peripherals::CAN1,
};
use embassy_time::{with_timeout, Duration, Timer};
use panic_probe as _;

const NODE_ID: u8 = 0x01;
const NMT_ID: u16 = 0x000;

#[allow(unused)]
const NMT_ENTER_PRE_OPERATIONAL: u8 = 0x80;
#[allow(unused)]
const NMT_ENTER_RESET: u8 = 0x81;
const CMD: u8 = NMT_ENTER_RESET;

const NODE_GUARD_ID: u16 = 0x700 + NODE_ID as u16;
const NMT_STATE_PRE_OPERATIONAL: u8 = 0x7F;
const NODE_GUARD_TIMEOUT: Duration = Duration::from_secs(1);

bind_interrupts!(struct Irqs {
    CAN1_RX0 => Rx0InterruptHandler<CAN1>;
    CAN1_RX1 => Rx1InterruptHandler<CAN1>;
    CAN1_SCE => SceInterruptHandler<CAN1>;
    CAN1_TX => TxInterruptHandler<CAN1>;
});

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

    let nmt = unwrap!(Frame::new_data(
        Id::Standard(unwrap!(StandardId::new(NMT_ID))),
        &[CMD, NODE_ID],
    ));
    let rtr = unwrap!(Frame::new_remote(
        Id::Standard(unwrap!(StandardId::new(NODE_GUARD_ID))),
        1
    ));

    loop {
        info!("Sending Preoperational message...");
        can.write(&nmt).await;
        Timer::after(Duration::from_secs(1)).await;

        info!("Requesting NMT state via RTR...");
        can.write(&rtr).await;

        let state = loop {
            let Ok(result) = with_timeout(NODE_GUARD_TIMEOUT, can.read()).await else {
                break None;
            };
            match result {
                Ok(envelope) => match envelope.frame.id() {
                    Id::Standard(id) if id.as_raw() == NODE_GUARD_ID => {
                        let Some(&byte) = envelope.frame.data().first() else {
                            continue;
                        };
                        break Some(byte & 0x7F);
                    }
                    _ => {}
                },
                Err(e) => warn!("CAN receive error: {:?}", e),
            }
        };

        match state {
            Some(NMT_STATE_PRE_OPERATIONAL) => break,
            Some(state) => warn!(
                "NMT state is {=u8:#04x}, expected pre-operational. Retrying...",
                state
            ),
            None => warn!("Timed out waiting for NMT state response. Retrying..."),
        }
    }

    info!("NMT state confirmed: pre-operational");

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
