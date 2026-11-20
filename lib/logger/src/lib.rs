#![no_std]
#![allow(unused)]

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, AtomicU8, Ordering},
};

pub const LOG_BUFFER_SIZE: usize = 4096;

struct State {
    buffer: Buffer<LOG_BUFFER_SIZE>,
    encoder: defmt::Encoder,
    restore: critical_section::RestoreState,
    frame_start: usize,
    overflowed: bool,
}

struct Shared(UnsafeCell<State>);
unsafe impl Sync for Shared {}

static STATE: Shared = Shared(UnsafeCell::new(State {
    buffer: Buffer::new(),
    encoder: defmt::Encoder::new(),
    restore: critical_section::RestoreState::invalid(),
    frame_start: 0,
    overflowed: false,
}));

// static TAKEN: AtomicBool = AtomicBool::new(false);
static DROPPED: AtomicU32 = AtomicU32::new(0);
static LOG_DEPTH: AtomicU8 = AtomicU8::new(0);
static NESTED_DROPPED: AtomicU32 = AtomicU32::new(0);

pub struct Buffer<const N: usize> {
    buf: [u8; N],
    read: usize,
    write: usize,
}

// ring buffer
impl<const N: usize> Buffer<N> {
    fn default() -> Self {
        Self::new()
    }
    pub const fn new() -> Self {
        Buffer {
            buf: [0; N],
            read: 0,
            write: 0,
        }
    }
    pub fn push(&mut self, bytes: &[u8]) -> bool {
        let free = N - self.len();
        if bytes.len() > free {
            return false;
        }

        for &byte in bytes {
            self.buf[self.write % N] = byte;
            self.write = self.write.wrapping_add(1);
        }

        true
    }
    pub fn write_pos(&self) -> usize {
        self.write
    }
    pub fn truncate_to(&mut self, pos: usize) {
        self.write = pos;
    }
    pub fn len(&self) -> usize {
        self.write.wrapping_sub(self.read)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn read(&mut self, out: &mut [u8]) -> usize {
        let count = core::cmp::min(out.len(), self.len());

        for item in out.iter_mut().take(count) {
            *item = self.buf[self.read % N];
            self.read = self.read.wrapping_add(1);
        }

        count
    }
    pub const fn capacity(&self) -> usize {
        N
    }
}

#[cfg(target_os = "none")]
#[defmt::global_logger]
struct Logger;

#[cfg(target_os = "none")]
unsafe impl defmt::Logger for Logger {
    fn acquire() {
        if LOG_DEPTH.load(Ordering::Relaxed) > 0 {
            NESTED_DROPPED.fetch_add(1, Ordering::Relaxed);
            return ();
        }
        LOG_DEPTH.store(1, Ordering::Relaxed);

        let restore = unsafe { critical_section::acquire() };

        let state = unsafe { &mut *STATE.0.get() };
        state.restore = restore;
        state.frame_start = state.buffer.write_pos();
        state.overflowed = false;

        let State {
            encoder,
            buffer,
            overflowed,
            ..
        } = state;
        encoder.start_frame(|bytes| {
            if !buffer.push(bytes) {
                *overflowed = true;
            }
        });
    }
    unsafe fn write(bytes: &[u8]) {
        if LOG_DEPTH.load(Ordering::Relaxed) > 1 {
            return ();
        }
        let state = &mut *STATE.0.get();
        let State {
            encoder,
            buffer,
            overflowed,
            ..
        } = state;
        encoder.write(bytes, |encoded| {
            if !buffer.push(encoded) {
                *overflowed = true;
            }
        })
    }
    unsafe fn flush() {
        // Nothing to do here
    }
    unsafe fn release() {
        if LOG_DEPTH.load(Ordering::Relaxed) > 1 {
            LOG_DEPTH.store(0, Ordering::Relaxed);
            return ();
        }
        let state = &mut *STATE.0.get();
        {
            let State {
                encoder,
                buffer,
                overflowed,
                ..
            } = &mut *state;
            encoder.start_frame(|bytes| {
                if !buffer.push(bytes) {
                    *overflowed = true;
                }
            });
        }

        if state.overflowed {
            state.buffer.truncate_to(state.frame_start);
            DROPPED.fetch_add(1, Ordering::Relaxed);
        }

        LOG_DEPTH.store(0, Ordering::Relaxed);
        critical_section::release(state.restore);
    }
}

pub fn read(out: &mut [u8]) -> usize {
    critical_section::with(|_| {
        let state = unsafe { &mut *STATE.0.get() };
        state.buffer.read(out)
    })
}

pub fn dropped() -> u32 {
    DROPPED.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small buffer so wrap-around happens quickly in tests.
    type SmallBuffer = Buffer<8>;

    /// Read everything currently waiting in the buffer into a Vec-free array.
    /// Returns (bytes, count).
    fn drain(buf: &mut SmallBuffer) -> ([u8; 8], usize) {
        let mut out = [0u8; 8];
        let n = buf.read(&mut out);
        (out, n)
    }

    // ---------------------------------------------------------------
    // Empty buffer
    // ---------------------------------------------------------------

    #[test]
    fn new_buffer_is_empty() {
        let mut buf = SmallBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.capacity(), 8);

        let mut out = [0u8; 4];
        assert_eq!(
            buf.read(&mut out),
            0,
            "reading an empty buffer returns 0 bytes"
        );
    }

    #[test]
    fn read_into_empty_slice_returns_zero() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(&[1, 2, 3]));

        let mut out = [0u8; 0];
        assert_eq!(buf.read(&mut out), 0);
        assert_eq!(buf.len(), 3, "nothing should have been removed");
    }

    // ---------------------------------------------------------------
    // Push and read
    // ---------------------------------------------------------------

    #[test]
    fn push_then_read_returns_same_bytes_in_order() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(&[1, 2, 3]));
        assert_eq!(buf.len(), 3);

        let (out, n) = drain(&mut buf);
        assert_eq!(n, 3);
        assert_eq!(&out[..n], &[1, 2, 3]);
        assert!(buf.is_empty());
    }

    #[test]
    fn multiple_pushes_are_read_in_order() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(&[1, 2]));
        assert!(buf.push(&[3]));
        assert!(buf.push(&[4, 5]));

        let (out, n) = drain(&mut buf);
        assert_eq!(&out[..n], &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn pushing_empty_slice_succeeds_and_changes_nothing() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(&[]));
        assert!(buf.is_empty());
    }

    #[test]
    fn partial_reads_return_bytes_in_chunks() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(&[1, 2, 3, 4, 5, 6, 7]));

        // Read in chunks of 3, like the CAN forwarder reading 7 at a time.
        let mut out = [0u8; 3];

        assert_eq!(buf.read(&mut out), 3);
        assert_eq!(out, [1, 2, 3]);

        assert_eq!(buf.read(&mut out), 3);
        assert_eq!(out, [4, 5, 6]);

        assert_eq!(buf.read(&mut out), 1, "only one byte left");
        assert_eq!(out[0], 7);

        assert_eq!(buf.read(&mut out), 0);
        assert!(buf.is_empty());
    }

    // ---------------------------------------------------------------
    // Full buffer
    // ---------------------------------------------------------------

    #[test]
    fn can_fill_to_exact_capacity() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(&[0; 8]));
        assert_eq!(buf.len(), buf.capacity());
    }

    #[test]
    fn push_into_full_buffer_is_rejected() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(&[1, 2, 3, 4, 5, 6, 7, 8]));

        assert!(!buf.push(&[9]), "no room left, push must fail");
        assert_eq!(buf.len(), 8, "a failed push must not change the length");

        let (out, n) = drain(&mut buf);
        assert_eq!(
            &out[..n],
            &[1, 2, 3, 4, 5, 6, 7, 8],
            "existing data must be untouched"
        );
    }

    #[test]
    fn push_is_all_or_nothing() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(&[1, 2, 3, 4, 5, 6])); // 2 bytes free

        assert!(!buf.push(&[7, 8, 9]), "3 bytes don't fit in 2 free slots");
        assert_eq!(buf.len(), 6, "none of the 3 bytes may have been written");

        assert!(buf.push(&[7, 8]), "exactly 2 bytes still fit");
        let (out, n) = drain(&mut buf);
        assert_eq!(&out[..n], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn reading_frees_space_for_new_pushes() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(&[1, 2, 3, 4, 5, 6, 7, 8]));
        assert!(!buf.push(&[9]));

        let mut out = [0u8; 3];
        assert_eq!(buf.read(&mut out), 3);

        assert!(buf.push(&[9, 10, 11]), "3 slots were freed by the read");
        assert!(!buf.push(&[12]));
    }

    // ---------------------------------------------------------------
    // Wrap-around
    // ---------------------------------------------------------------

    #[test]
    fn data_survives_wrapping_past_end_of_array() {
        let mut buf = SmallBuffer::new();

        // Move the read/write positions to slot 6 of 8.
        assert!(buf.push(&[0; 6]));
        let mut skip = [0u8; 6];
        assert_eq!(buf.read(&mut skip), 6);

        // These 5 bytes go into slots 6, 7, 0, 1, 2: across the end of the array.
        assert!(buf.push(&[10, 20, 30, 40, 50]));

        let (out, n) = drain(&mut buf);
        assert_eq!(&out[..n], &[10, 20, 30, 40, 50]);
    }

    #[test]
    fn many_laps_around_the_buffer_keep_data_correct() {
        let mut buf = SmallBuffer::new();
        let mut next_value: u8 = 0;

        // Go round the 8-slot buffer many times with awkward chunk sizes.
        for _ in 0..100 {
            let mut chunk = [0u8; 5];
            for byte in chunk.iter_mut() {
                *byte = next_value;
                next_value = next_value.wrapping_add(1);
            }
            assert!(buf.push(&chunk));

            let mut out = [0u8; 5];
            assert_eq!(buf.read(&mut out), 5);
            assert_eq!(out, chunk);
            assert!(buf.is_empty());
        }
    }

    // ---------------------------------------------------------------
    // Rollback (used by the logger to discard a half-written message)
    // ---------------------------------------------------------------

    #[test]
    fn truncate_discards_bytes_after_saved_position() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(b"AB"));

        let pos = buf.write_pos();
        assert!(buf.push(b"CDE"));
        assert_eq!(buf.len(), 5);

        buf.truncate_to(pos);
        assert_eq!(buf.len(), 2);

        assert!(buf.push(b"F"));
        let (out, n) = drain(&mut buf);
        assert_eq!(&out[..n], b"ABF");
    }

    #[test]
    fn truncate_to_current_position_changes_nothing() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(b"ABC"));

        let pos = buf.write_pos();
        buf.truncate_to(pos);

        let (out, n) = drain(&mut buf);
        assert_eq!(&out[..n], b"ABC");
    }

    #[test]
    fn truncate_frees_space_for_new_pushes() {
        let mut buf = SmallBuffer::new();
        assert!(buf.push(b"AB"));

        let pos = buf.write_pos();
        assert!(buf.push(b"CDEFGH")); // now full
        assert!(!buf.push(b"X"));

        buf.truncate_to(pos);
        assert!(buf.push(b"XYZ"), "rolled-back space can be reused");

        let (out, n) = drain(&mut buf);
        assert_eq!(&out[..n], b"ABXYZ");
    }

    #[test]
    fn truncate_works_across_the_wrap_point() {
        let mut buf = SmallBuffer::new();

        // Move positions to slot 6.
        assert!(buf.push(&[0; 6]));
        let mut skip = [0u8; 6];
        buf.read(&mut skip);

        assert!(buf.push(b"A")); // slot 6
        let pos = buf.write_pos();
        assert!(buf.push(b"BCD")); // slots 7, 0, 1 (wraps)

        buf.truncate_to(pos);
        assert!(buf.push(b"E"));

        let (out, n) = drain(&mut buf);
        assert_eq!(&out[..n], b"AE");
    }

    /// Mimics what the logger does with one message: push in pieces,
    /// overflow partway, roll back the whole message, keep earlier messages.
    #[test]
    fn overflowing_message_is_rolled_back_whole() {
        let mut buf = SmallBuffer::new();

        // Message 1 fits.
        assert!(buf.push(b"m1."));

        // Message 2 is pushed in pieces and overflows partway through.
        let frame_start = buf.write_pos();
        let mut overflowed = false;
        for piece in [&b"m2"[..], &b"-long"[..]] {
            if !buf.push(piece) {
                overflowed = true;
            }
        }
        assert!(overflowed);

        buf.truncate_to(frame_start);

        let (out, n) = drain(&mut buf);
        assert_eq!(&out[..n], b"m1.", "only the complete first message remains");
    }

    // ---------------------------------------------------------------
    // Counter overflow (positions wrap round usize::MAX)
    // ---------------------------------------------------------------

    #[test]
    fn works_when_position_counters_wrap_around_usize_max() {
        // Start the counters just below usize::MAX so they overflow during the test.
        // Only possible from inside this module because the fields are private.
        let mut buf = SmallBuffer {
            buf: [0; 8],
            read: usize::MAX - 2,
            write: usize::MAX - 2,
        };
        assert!(buf.is_empty());

        assert!(buf.push(&[1, 2, 3, 4, 5])); // `write` overflows past usize::MAX
        assert_eq!(buf.len(), 5);

        let (out, n) = drain(&mut buf);
        assert_eq!(&out[..n], &[1, 2, 3, 4, 5]);
        assert!(buf.is_empty());
    }
}
