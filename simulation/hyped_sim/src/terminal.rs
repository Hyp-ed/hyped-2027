use std::mem;

const TELNET_SE: u8 = 240;
const TELNET_SB: u8 = 250;
const TELNET_WILL: u8 = 251;
const TELNET_DONT: u8 = 254;
const TELNET_IAC: u8 = 255;

#[derive(Default)]
enum TelnetState {
    #[default]
    Data,
    Command,
    Option,
    Subnegotiation,
    SubnegotiationCommand,
}

#[derive(Default)]
enum EscapeState {
    #[default]
    Text,
    Started,
    Csi,
}

/// Turns the bytes of a Renode socket into plain text. Telnet negotiation, ANSI escapes and
/// control characters are removed. Any of them, and any UTF-8 character, can span two reads, so
/// the state is kept between calls.
#[derive(Default)]
pub struct TerminalDecoder {
    telnet: TelnetState,
    escape: EscapeState,
    incomplete_utf8: Vec<u8>,
}

impl TerminalDecoder {
    pub fn decode(&mut self, bytes: &[u8]) -> String {
        let data = self.strip_telnet(bytes);
        let mut rest = data.as_slice();
        let mut text = String::new();

        loop {
            match str::from_utf8(rest) {
                Ok(valid) => {
                    self.push_text(valid, &mut text);
                    return text;
                }
                Err(e) => {
                    let (valid, invalid) = rest.split_at(e.valid_up_to());
                    // Checked by from_utf8
                    self.push_text(str::from_utf8(valid).unwrap_or_default(), &mut text);
                    match e.error_len() {
                        Some(len) => {
                            text.push(char::REPLACEMENT_CHARACTER);
                            rest = &invalid[len..];
                        }
                        None => {
                            self.incomplete_utf8 = invalid.to_vec();
                            return text;
                        }
                    }
                }
            }
        }
    }

    fn strip_telnet(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut data = mem::take(&mut self.incomplete_utf8);
        for &byte in bytes {
            self.telnet = match mem::take(&mut self.telnet) {
                TelnetState::Data if byte == TELNET_IAC => TelnetState::Command,
                TelnetState::Data => {
                    data.push(byte);
                    TelnetState::Data
                }
                TelnetState::Command => match byte {
                    TELNET_WILL..=TELNET_DONT => TelnetState::Option,
                    TELNET_SB => TelnetState::Subnegotiation,
                    TELNET_IAC => {
                        data.push(byte);
                        TelnetState::Data
                    }
                    _ => TelnetState::Data,
                },
                TelnetState::Option => TelnetState::Data,
                TelnetState::Subnegotiation if byte == TELNET_IAC => {
                    TelnetState::SubnegotiationCommand
                }
                TelnetState::Subnegotiation => TelnetState::Subnegotiation,
                TelnetState::SubnegotiationCommand if byte == TELNET_SE => TelnetState::Data,
                TelnetState::SubnegotiationCommand => TelnetState::Subnegotiation,
            };
        }
        data
    }

    fn push_text(&mut self, input: &str, text: &mut String) {
        for c in input.chars() {
            self.escape = match mem::take(&mut self.escape) {
                EscapeState::Text if c == '\x1b' => EscapeState::Started,
                EscapeState::Text => {
                    if !c.is_control() || c == '\n' || c == '\t' {
                        text.push(c);
                    }
                    EscapeState::Text
                }
                EscapeState::Started if c == '[' => EscapeState::Csi,
                EscapeState::Started => EscapeState::Text,
                EscapeState::Csi if ('@'..='~').contains(&c) => EscapeState::Text,
                EscapeState::Csi => EscapeState::Csi,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_renode_terminal_preamble() {
        let mut decoder = TerminalDecoder::default();
        let text =
            decoder.decode(b"\xff\xfd\x00\xff\xfd\x1f\xff\xfb\x01\xff\xfb\x03\xff\xfc\"boot");
        assert_eq!(text, "boot");
    }

    #[test]
    fn strips_ansi_and_carriage_returns() {
        let mut decoder = TerminalDecoder::default();
        assert_eq!(decoder.decode(b"\x1b[91mfail\x1b[0m\r\n\tok"), "fail\n\tok");
    }

    #[test]
    fn keeps_state_between_reads() {
        let mut decoder = TerminalDecoder::default();
        assert_eq!(decoder.decode(b"a\xff"), "a");
        assert_eq!(decoder.decode(b"\xfb\x01b\x1b["), "b");
        assert_eq!(decoder.decode(b"0mc\xc3"), "c");
        assert_eq!(decoder.decode(b"\xa9"), "é");
    }

    #[test]
    fn replaces_invalid_utf8() {
        let mut decoder = TerminalDecoder::default();
        assert_eq!(decoder.decode(b"a\x80b"), "a\u{fffd}b");
    }
}
