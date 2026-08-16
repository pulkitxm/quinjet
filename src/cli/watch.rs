use std::io::{self, IsTerminal, Write};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use serde::Serialize;

pub(crate) struct Frame<T> {
    pub value: T,
    pub text: String,
    pub finished: bool,
    pub code: u8,
}

pub(crate) fn run<T: Serialize>(
    interval: Duration,
    json: bool,
    mut read: impl FnMut() -> Result<Frame<T>>,
) -> Result<u8> {
    let repaint = !json && io::stdout().is_terminal();
    loop {
        let frame = read()?;
        let mut stdout = io::stdout().lock();
        if json {
            writeln!(stdout, "{}", serde_json::to_string(&frame.value)?)?;
        } else {
            if repaint {
                write!(stdout, "\x1b[H\x1b[2J")?;
            }
            write!(stdout, "{}", frame.text)?;
        }
        if frame.finished {
            stdout.flush()?;
            return Ok(frame.code);
        }
        if !json {
            writeln!(
                stdout,
                "\nwatching, refreshing every {}s (Ctrl+C to stop)",
                interval.as_secs()
            )?;
        }
        stdout.flush()?;
        drop(stdout);
        thread::sleep(interval);
    }
}
