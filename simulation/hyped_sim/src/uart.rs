use std::{
    io::{ErrorKind, Read},
    net::TcpStream,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use crate::terminal::TerminalDecoder;

const READ_BUFFER_SIZE: usize = 4096;
/// Short because whatever the UART prints before we connect is lost, and the socket only opens
/// while a script is loading.
const RECONNECT_INTERVAL: Duration = Duration::from_millis(100);
/// Limits how often QML gets woken up by a chatty UART. Also how long a read blocks, and so how
/// long a cancelled subscription can linger.
const FLUSH_INTERVAL: Duration = Duration::from_millis(50);

pub enum UartEvent {
    Connected(bool),
    Text(String),
}

/// A connection to a Renode server socket terminal. It's retried for as long as this exists,
/// because the socket goes away whenever the emulation is cleared.
pub struct Subscription {
    cancelled: Arc<AtomicBool>,
}

impl Subscription {
    pub fn new(port: u16, on_event: impl Fn(UartEvent) + Send + 'static) -> Self {
        let cancelled = Arc::new(AtomicBool::new(false));
        let thread_cancelled = cancelled.clone();
        thread::spawn(move || {
            while !thread_cancelled.load(Ordering::Relaxed) {
                if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
                    on_event(UartEvent::Connected(true));
                    read_until_closed(stream, &thread_cancelled, &on_event);
                    on_event(UartEvent::Connected(false));
                }
                thread::sleep(RECONNECT_INTERVAL);
            }
        });
        Self { cancelled }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

fn read_until_closed(mut stream: TcpStream, cancelled: &AtomicBool, on_event: &impl Fn(UartEvent)) {
    if stream.set_read_timeout(Some(FLUSH_INTERVAL)).is_err() {
        return;
    }

    let mut decoder = TerminalDecoder::default();
    let mut buffer = [0; READ_BUFFER_SIZE];
    let mut pending = String::new();
    let mut last_flush = Instant::now();

    while !cancelled.load(Ordering::Relaxed) {
        let open = match stream.read(&mut buffer) {
            Ok(0) => false,
            Ok(size) => {
                pending.push_str(&decoder.decode(&buffer[..size]));
                true
            }
            Err(e) => matches!(
                e.kind(),
                ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
            ),
        };

        if !pending.is_empty() && (!open || last_flush.elapsed() >= FLUSH_INTERVAL) {
            on_event(UartEvent::Text(std::mem::take(&mut pending)));
            last_flush = Instant::now();
        }
        if !open {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Write, net::TcpListener, sync::mpsc};

    use super::*;

    const TIMEOUT: Duration = Duration::from_secs(5);

    #[test]
    fn delivers_text_and_reconnects_when_the_socket_closes() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (events, received) = mpsc::channel();
        let subscription = Subscription::new(port, move |event| events.send(event).unwrap());

        for message in ["first", "second"] {
            let (mut client, _) = listener.accept().unwrap();
            assert!(matches!(
                received.recv_timeout(TIMEOUT),
                Ok(UartEvent::Connected(true))
            ));
            client.write_all(b"\xff\xfb\x01").unwrap();
            client.write_all(message.as_bytes()).unwrap();
            drop(client);

            let mut text = String::new();
            loop {
                match received.recv_timeout(TIMEOUT).unwrap() {
                    UartEvent::Text(chunk) => text.push_str(&chunk),
                    UartEvent::Connected(connected) => {
                        assert!(!connected);
                        break;
                    }
                }
            }
            assert_eq!(text, message);
        }
        drop(subscription);
    }

    #[test]
    fn dropping_the_subscription_closes_the_connection() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let subscription = Subscription::new(port, |_| {});

        let (mut client, _) = listener.accept().unwrap();
        drop(subscription);

        client.set_read_timeout(Some(TIMEOUT)).unwrap();
        assert_eq!(client.read(&mut [0; 1]).unwrap(), 0);
    }
}
