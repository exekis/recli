use crate::error::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::io::Write;

/// handles the conversion of terminal events to PTY input
pub struct InputHandler;

impl InputHandler {
    /// convert a crossterm KeyEvent to bytes that can be sent to PTY
    /// returns None if this is a special hotkey that shouldn't be forwarded
    pub fn key_to_bytes(key_event: KeyEvent) -> Option<Vec<u8>> {
        if key_event.kind != KeyEventKind::Press {
            return None;
        }

        // check for ctrl+x hotkey before processing other keys
        if key_event.modifiers.contains(KeyModifiers::CONTROL) {
            if let KeyCode::Char('x') = key_event.code {
                // detected ctrl+x so handle it and don't forward to pty
                println!("\r\n[RECLI] Hotkey detected! (Ctrl+X intercepted)");
                return None;
            }
        }

        match key_event.code {
            // gegular characters encode as UTF-8
            KeyCode::Char(c) => {
                let mut bytes = [0u8; 4];
                let encoded = c.encode_utf8(&mut bytes);
                Some(encoded.as_bytes().to_vec())
            }
            // special keys
            KeyCode::Enter => Some(b"\r".to_vec()),
            KeyCode::Backspace => Some(vec![127]), // DEL character
            KeyCode::Tab => Some(b"\t".to_vec()),

            // arrow keys ANSI escape sequences
            KeyCode::Up => Some(b"\x1b[A".to_vec()),
            KeyCode::Down => Some(b"\x1b[B".to_vec()),
            KeyCode::Right => Some(b"\x1b[C".to_vec()),
            KeyCode::Left => Some(b"\x1b[D".to_vec()),

            // function keys
            KeyCode::F(n) => match n {
                1..=12 => {
                    // F1-F12 escape sequences
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
            'c' => Some(vec![3]),  // Ctrl+C
            'd' => Some(vec![4]),  // Ctrl+D (EOF)
            'z' => Some(vec![26]), // Ctrl+Z (suspend)
            'l' => Some(vec![12]), // Ctrl+L (clear screen)
            _ => None,
        }
    }
}

/// handles the output from PTY to terminal
pub struct OutputHandler;

impl OutputHandler {
    /// forward PTY output to stdout with error handling
    pub fn forward_to_stdout(buffer: &[u8]) -> Result<()> {
        let mut stdout = std::io::stdout();
        stdout.write_all(buffer)?;
        stdout.flush()?;
        Ok(())
    }

    /// process and potentially filter PTY output
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
