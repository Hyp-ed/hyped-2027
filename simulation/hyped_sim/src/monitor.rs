use std::{
    io::{self, Read, Write},
    net::TcpStream,
    sync::mpsc::Receiver,
};

use crate::terminal::TerminalDecoder;

const READ_BUFFER_SIZE: usize = 4096;
const PROMPT_END: &[u8] = b") \x1b[0m";
const ESCAPE_START: &[u8] = b"\x1b[";
const ERROR_COLOUR: &[u8] = b"\x1b[91m";

pub struct Job {
    pub id: i32,
    pub command: String,
}

pub enum MonitorEvent {
    Output(String),
    Finished { id: i32, succeeded: bool },
}

/// `@` paths end at the first unescaped space and the monitor can't tokenize quoted ones.
pub fn include_command(path: &str) -> String {
    format!("include @{}", path.replace(' ', "\\ "))
}

/// Runs the jobs in order, each one waiting for the monitor to finish the one before it.
pub fn run(mut stream: TcpStream, jobs: Receiver<Job>, on_event: impl Fn(MonitorEvent)) {
    let mut decoder = TerminalDecoder::default();
    let mut connected = read_response(&mut stream, &mut decoder, &on_event).is_ok();

    for Job { id, command } in jobs {
        let succeeded = connected
            && match stream
                .write_all(format!("{command}\n").as_bytes())
                .and_then(|()| read_response(&mut stream, &mut decoder, &on_event))
            {
                Ok(succeeded) => succeeded,
                Err(e) => {
                    connected = false;
                    on_event(MonitorEvent::Output(format!(
                        "\nLost the connection to the Renode monitor: {e}\n"
                    )));
                    false
                }
            };
        on_event(MonitorEvent::Finished { id, succeeded });
    }
}

/// Forwards the output up to the next prompt, returning whether it was free of errors.
fn read_response(
    stream: &mut TcpStream,
    decoder: &mut TerminalDecoder,
    on_event: &impl Fn(MonitorEvent),
) -> io::Result<bool> {
    let mut response = Vec::new();
    let mut buffer = [0; READ_BUFFER_SIZE];

    loop {
        let size = stream.read(&mut buffer)?;
        if size == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        on_event(MonitorEvent::Output(decoder.decode(&buffer[..size])));
        response.extend_from_slice(&buffer[..size]);
        if let Some(succeeded) = parse_response(&response) {
            return Ok(succeeded);
        }
    }
}

/// The monitor has no status codes, but it prints errors in red and ends every response with a
/// coloured prompt. The prompt of a machine-less monitor is that same red, so it's left out.
fn parse_response(response: &[u8]) -> Option<bool> {
    let before_end = response.strip_suffix(PROMPT_END)?;
    let prompt_start = rfind(before_end, ESCAPE_START)?;
    Some(rfind(&response[..prompt_start], ERROR_COLOUR).is_none())
}

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, net::TcpListener, sync::mpsc, thread};

    use super::*;

    #[test]
    fn escapes_spaces_in_paths() {
        assert_eq!(include_command("/a b/c.resc"), "include @/a\\ b/c.resc");
    }

    #[test]
    fn incomplete_response_has_no_result() {
        assert_eq!(parse_response(b"Starting emulation...\r\n"), None);
    }

    #[test]
    fn prompt_colour_is_not_an_error() {
        assert_eq!(
            parse_response(b"i @x\n\r\x1b[91m(monitor) \x1b[0m"),
            Some(true)
        );
        assert_eq!(parse_response(b"i @x\n\r\x1b[33m(m0) \x1b[0m"), Some(true));
    }

    #[test]
    fn red_output_is_an_error() {
        let response = b"i @x\n\r\x1b[91mNo such command\r\n\x1b[0m\x1b[91m(monitor) \x1b[0m";
        assert_eq!(parse_response(response), Some(false));
    }

    #[test]
    fn jobs_report_their_outcome_in_order() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut client, _) = listener.accept().unwrap();
            client
                .write_all(b"\xff\xfb\x01banner\n\r\x1b[91m(monitor) \x1b[0m")
                .unwrap();
            let mut buffer = [0; 64];
            let size = client.read(&mut buffer).unwrap();
            assert_eq!(&buffer[..size], b"good\n");
            client.write_all(b"\x1b[33m(m0) \x1b[0m").unwrap();
            let size = client.read(&mut buffer).unwrap();
            assert_eq!(&buffer[..size], b"bad\n");
            client
                .write_all(b"\x1b[91mnope\r\n\x1b[0m\x1b[33m(m0) \x1b[0m")
                .unwrap();
            // The connection closes here, failing the last job
        });

        let (jobs, receiver) = mpsc::channel();
        for (id, command) in [(1, "good"), (2, "bad"), (3, "lost")] {
            jobs.send(Job {
                id,
                command: command.into(),
            })
            .unwrap();
        }
        drop(jobs);

        let finished = RefCell::new(Vec::new());
        let output = RefCell::new(String::new());
        run(
            TcpStream::connect(("127.0.0.1", port)).unwrap(),
            receiver,
            |event| match event {
                MonitorEvent::Output(text) => output.borrow_mut().push_str(&text),
                MonitorEvent::Finished { id, succeeded } => {
                    finished.borrow_mut().push((id, succeeded))
                }
            },
        );
        server.join().unwrap();

        assert_eq!(*finished.borrow(), [(1, true), (2, false), (3, false)]);
        assert!(output.borrow().starts_with("banner\n(monitor) "));
        assert!(output.borrow().contains("Lost the connection"));
    }
}
