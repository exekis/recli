use crate::error::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use std::io::Write;

/// handles the conversion of terminal events to pty input
pub struct InputHandler;

impl InputHandler {
    /// convert a crossterm keyevent to bytes that can be sent to pty
    /// returns none if this is a special hotkey that shouldn't be forwarded
    pub fn key_to_bytes(key_event: KeyEvent) -> Option<Vec<u8>> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

    // no hotkey interception; all keys pass through to the shell

        match key_event.code {
            // regular characters encode as utf-8
            KeyCode::Char(c) => {
                let mut bytes = [0u8; 4];
                let encoded = c.encode_utf8(&mut bytes);
                Some(encoded.as_bytes().to_vec())
            }
            // special keys
            KeyCode::Enter => Some(b"\r".to_vec()),
            KeyCode::Backspace => Some(vec![127]), // DEL character
            KeyCode::Tab => Some(b"\t".to_vec()),

            // arrow keys ansi escape sequences
            KeyCode::Up => Some(b"\x1b[A".to_vec()),
            KeyCode::Down => Some(b"\x1b[B".to_vec()),
            KeyCode::Right => Some(b"\x1b[C".to_vec()),
            KeyCode::Left => Some(b"\x1b[D".to_vec()),

            // function keys
            KeyCode::F(n) => match n {
                1..=12 => {
                    // f1-f12 escape sequences
                    Some(format!("\x1b[{};2~", n + 10).into_bytes())
                }
                _ => None,
            },

            // page navigation
            KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
            KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
            KeyCode::Home => Some(b"\x1b[H".to_vec()),
            KeyCode::End => Some(b"\x1b[F".to_vec()),

            // insert/delete
            KeyCode::Insert => Some(b"\x1b[2~".to_vec()),
            KeyCode::Delete => Some(b"\x1b[3~".to_vec()),

            // escape key
            KeyCode::Esc => Some(b"\x1b".to_vec()),

            // ignore other keys for now
            _ => None,
        }
    }

    /// special key combinations
    pub fn handle_control_key(c: char) -> Option<Vec<u8>> {
        match c {
            'c' => Some(vec![3]),  // ctrl+c
            'd' => Some(vec![4]),  // ctrl+d (eof)
            'z' => Some(vec![26]), // ctrl+z (suspend)
            'l' => Some(vec![12]), // ctrl+l (clear screen)
            _ => None,
        }
    }
}

/// handles the output from pty to terminal
pub struct OutputHandler;

impl OutputHandler {
    /// forward pty output to stdout with error handling
    pub fn forward_to_stdout(buffer: &[u8]) -> Result<()> {
        let mut stdout = std::io::stdout();
        stdout.write_all(buffer)?;
        stdout.flush()?;
        Ok(())
    }

    /// process and potentially filter pty output
    /// (where ill add features like error detection)
    pub fn process_output(buffer: &[u8]) -> Vec<u8> {
        // for now just pass through unchanged
        // TODO: later enhancements:
        // - detect error patterns
        // - log command output
        // - parse ANSI escape sequences
        buffer.to_vec()
    }
}
