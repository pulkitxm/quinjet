use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::thread;

use anyhow::{Context, Result, anyhow};

use super::super::{MAX_GH_ERROR_BYTES, MAX_GH_METADATA_BYTES, process};

pub(super) fn read_records(
    command: &mut Command,
    delimiter: u8,
    mut consume: impl FnMut(&[u8]),
) -> Result<bool> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().context("Git did not expose stdout")?;
    let stderr = child.stderr.take().context("Git did not expose stderr")?;
    let errors = thread::spawn(move || process::read_and_drain(stderr, MAX_GH_ERROR_BYTES));
    let mut reader = BufReader::new(stdout);
    let mut record = Vec::new();
    let result = (|| -> Result<bool> {
        loop {
            record.clear();
            let read = reader
                .by_ref()
                .take((MAX_GH_METADATA_BYTES + 1) as u64)
                .read_until(delimiter, &mut record)?;
            if read == 0 {
                return Ok(true);
            }
            if read > MAX_GH_METADATA_BYTES {
                return Ok(false);
            }
            consume(&record);
        }
    })();
    if !matches!(result, Ok(true)) {
        drop(child.kill());
    }
    drop(reader);
    let status = child.wait();
    let errors = errors
        .join()
        .map_err(|_| anyhow!("Git stderr reader panicked"))?;
    drop(errors?);
    Ok(result? && status?.success())
}
