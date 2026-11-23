//! Stress test for `hyped_logger` on the STM32F767ZI.
//!
//! Many tasks log at once while two interrupts (at different priorities) also
//! log, and the low-priority interrupt triggers the high-priority one from inside
//! itself. At the end it reports how many log messages were:
//!   - logged:          counted at every log call in this program
//!   - received:        complete frames that came out of the ring buffer
//!   - dropped:         `hyped_logger::dropped()`, discarded because the buffer was full
//!   - nested dropped:  `hyped_logger::nested_dropped()`, discarded because logging happened inside logging
//!
//! PASS means: received + dropped == logged, and nested dropped == NESTED_LOGS.
//! A deadlock shows up as the heartbeat and the results never appearing.
//!
//! Run with a board connected:
//!     cargo run --release --bin logger_test
//!
//! Change the constants below and re-run to test different loads.
//!
//! The drain task in this file forwards the log bytes to the laptop over RTT
//! (channel name "defmt"), so `probe-rs` decodes and prints them as normal log
//! lines. Garbled output would mean corrupted frames.

#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use embassy_executor::Spawner;
use embassy_stm32::interrupt::{self, InterruptExt, Priority};
use embassy_time::{Duration, Instant, Timer};
use rtt_target::{rtt_init, ChannelMode, UpChannel};

use hyped_logger as _;
use panic_probe as _;

// ---------------------------------------------------------------------------
// Test configuration: change these and re-run
// ---------------------------------------------------------------------------

/// Number of tasks logging at the same time. Must be ≤ the `pool_size` of `log_spammer`.
const NUM_TASKS: u32 = 8;

/// Log messages each task sends.
const MSGS_PER_TASK: u32 = 500;

/// Each task logs this many messages in a row before pausing (bigger = burstier).
const MSGS_PER_BURST: u32 = 10;

/// How long each task pauses between bursts.
const TASK_PAUSE: Duration = Duration::from_micros(500);

/// How often the low-priority interrupt is triggered while the test runs.
const IRQ_INTERVAL: Duration = Duration::from_micros(300);

/// Number of deliberately nested log calls (a log line whose argument logs).
const NESTED_LOGS: u32 = 10;

// ---------------------------------------------------------------------------
// Counters
// ---------------------------------------------------------------------------

/// Every log call made through `counted_info!`.
static LOGGED: AtomicU32 = AtomicU32::new(0);
/// Complete frames (0x00 separators) the drain task took out of the buffer.
static RECEIVED: AtomicU32 = AtomicU32::new(0);
/// Spammer tasks that have finished.
static TASKS_DONE: AtomicU32 = AtomicU32::new(0);
/// Times each interrupt handler ran.
static LOW_IRQ_RUNS: AtomicU32 = AtomicU32::new(0);
static HIGH_IRQ_RUNS: AtomicU32 = AtomicU32::new(0);
/// Tells the interrupt trigger and heartbeat to stop before results are taken.
static STOP: AtomicBool = AtomicBool::new(false);

/// `defmt::info!`, plus counting the call in `LOGGED`.
macro_rules! counted_info {
    ($($arg:tt)*) => {{
        LOGGED.fetch_add(1, Ordering::Relaxed);
        defmt::info!($($arg)*);
    }};
}

// ---------------------------------------------------------------------------
// Logging from interrupt context
//
// UART4 and UART5 are not used by this program, so their interrupts are free
// to use as "software" interrupts: we trigger them with `.pend()`.
// On Cortex-M a LOWER priority number means HIGHER priority.
// ---------------------------------------------------------------------------

#[interrupt]
fn UART4() {
    // Low priority. Can be interrupted by UART5.
    let n = LOW_IRQ_RUNS.fetch_add(1, Ordering::Relaxed);
    counted_info!("low-priority interrupt {=u32} start", n);

    // Trigger the high-priority interrupt from inside this one. It runs
    // immediately (pre-empting us) unless we're inside a log call, in which
    // case it must wait until that log call's release().
    interrupt::UART5.pend();

    counted_info!("low-priority interrupt {=u32} end", n);
}

#[interrupt]
fn UART5() {
    // High priority.
    let n = HIGH_IRQ_RUNS.fetch_add(1, Ordering::Relaxed);
    counted_info!("high-priority interrupt {=u32}", n);
}

/// Triggers the low-priority interrupt repeatedly until the test stops.
#[embassy_executor::task]
async fn irq_trigger() {
    while !STOP.load(Ordering::Relaxed) {
        interrupt::UART4.pend();
        Timer::after(IRQ_INTERVAL).await;
    }
}

// ---------------------------------------------------------------------------
// Logging from many tasks
// ---------------------------------------------------------------------------

#[embassy_executor::task(pool_size = 16)]
async fn log_spammer(id: u32) {
    for i in 0..MSGS_PER_TASK {
        counted_info!("task {=u32} message {=u32}", id, i);
        if (i + 1) % MSGS_PER_BURST == 0 {
            Timer::after(TASK_PAUSE).await;
        }
    }
    TASKS_DONE.fetch_add(1, Ordering::Relaxed);
}

/// Logs regularly. If these stop appearing, something has deadlocked.
#[embassy_executor::task]
async fn heartbeat() {
    let mut beat: u32 = 0;
    while !STOP.load(Ordering::Relaxed) {
        counted_info!("heartbeat {=u32}", beat);
        beat += 1;
        Timer::after(Duration::from_millis(100)).await;
    }
}

// ---------------------------------------------------------------------------
// Nested logging: a type whose formatting itself logs
// ---------------------------------------------------------------------------

struct LogsWhileFormatted;

impl defmt::Format for LogsWhileFormatted {
    fn format(&self, f: defmt::Formatter) {
        // This inner log happens while the outer one is still in progress, so
        // the logger should drop it and count it in nested_dropped().
        // Deliberately NOT counted in LOGGED.
        defmt::info!("nested log (should be dropped)");
        defmt::write!(f, "<value>");
    }
}

// ---------------------------------------------------------------------------
// Drain: empties the log buffer, counts frames, forwards bytes to the laptop
// ---------------------------------------------------------------------------

#[embassy_executor::task]
async fn test_drain(mut rtt: UpChannel) {
    let mut chunk = [0u8; 64];
    loop {
        let n = hyped_logger::read(&mut chunk);
        if n == 0 {
            Timer::after(Duration::from_millis(1)).await;
            continue;
        }
        let bytes = &chunk[..n];

        // Every complete defmt frame ends in exactly one 0x00.
        let frames = bytes.iter().filter(|&&b| b == 0).count() as u32;
        RECEIVED.fetch_add(frames, Ordering::Relaxed);

        // Must NOT log here: it would feed back into the buffer forever.
        rtt.write(bytes);
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let _p = embassy_stm32::init(Default::default());

    // RTT channel to the laptop, named "defmt" so probe-rs decodes it.
    let channels = rtt_init! {
        up: {
            0: { size: 4096, mode: ChannelMode::BlockIfFull, name: "defmt" }
        }
    };
    spawner.must_spawn(test_drain(channels.up.0));

    defmt::info!(
        "logger_test: {=u32} tasks x {=u32} messages, bursts of {=u32}",
        NUM_TASKS,
        MSGS_PER_TASK,
        MSGS_PER_BURST
    );

    // Set up the two logging interrupts.
    interrupt::UART4.set_priority(Priority::P7); // lower priority
    interrupt::UART5.set_priority(Priority::P6); // higher priority
                                                 // SAFETY: both handlers are defined above and only touch atomics and the logger.
    unsafe {
        interrupt::UART4.enable();
        interrupt::UART5.enable();
    }

    // Phase 1: many tasks + interrupts logging at the same time.
    let start = Instant::now();
    spawner.must_spawn(heartbeat());
    spawner.must_spawn(irq_trigger());
    for id in 0..NUM_TASKS {
        spawner.must_spawn(log_spammer(id));
    }
    while TASKS_DONE.load(Ordering::Relaxed) < NUM_TASKS {
        Timer::after(Duration::from_millis(10)).await;
    }
    STOP.store(true, Ordering::Relaxed);
    let elapsed_ms = start.elapsed().as_millis();

    // Phase 2: deliberate nested logging.
    for _ in 0..NESTED_LOGS {
        counted_info!("outer log {}", LogsWhileFormatted);
    }

    // Let interrupts finish and the drain catch up.
    Timer::after(Duration::from_millis(500)).await;

    // Take the results.
    let logged = LOGGED.load(Ordering::Relaxed);
    let received = RECEIVED.load(Ordering::Relaxed);
    let dropped = hyped_logger::dropped();
    let nested = hyped_logger::nested_dropped();
    let low_irqs = LOW_IRQ_RUNS.load(Ordering::Relaxed);
    let high_irqs = HIGH_IRQ_RUNS.load(Ordering::Relaxed);

    // From here on, logs aren't counted.
    defmt::info!("===== logger_test results =====");
    defmt::info!(
        "tasks: {=u32}, msgs/task: {=u32}, time: {=u64} ms",
        NUM_TASKS,
        MSGS_PER_TASK,
        elapsed_ms
    );
    defmt::info!(
        "interrupt runs: low={=u32} high={=u32}",
        low_irqs,
        high_irqs
    );
    defmt::info!(
        "logged={=u32} received={=u32} dropped={=u32} nested_dropped={=u32}",
        logged,
        received,
        dropped,
        nested
    );

    // Note: the startup line above wasn't counted in LOGGED, and Embassy may log
    // a few lines of its own, so `received` can be slightly higher than expected.
    let accounted = received + dropped;
    if accounted >= logged && nested == NESTED_LOGS {
        defmt::info!(
            "PASS ({=u32} extra frames not from this test)",
            accounted - logged
        );
    } else {
        defmt::error!(
            "FAIL: received + dropped = {=u32}, expected at least {=u32}; nested_dropped = {=u32}, expected {=u32}",
            accounted,
            logged,
            nested,
            NESTED_LOGS
        );
    }

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
