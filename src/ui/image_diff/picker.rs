use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal, Read, Write};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::Result;
use crossterm::terminal::{WindowSize, is_raw_mode_enabled, window_size};
use ratatui_image::picker::cap_parser::{Parser, QueryStdioOptions, Response};

use super::{IMAGE_PICKER, ImageProtocol};

const HELPER_ARGUMENT: &str = "--internal-image-picker-query";
const QUERY_BUDGET: Duration = Duration::from_millis(500);
const MAX_RECORD_BYTES: usize = 64;
const MAX_QUERY_BYTES: u64 = 8192;

#[derive(Debug, Clone, Copy)]
pub(super) struct ImagePicker {
    pub(super) font_size: (u16, u16),
    pub(super) is_tmux: bool,
}

pub(crate) fn image_picker_helper() -> Option<ExitCode> {
    if !helper_requested(wild::args_os().skip(1)) {
        return None;
    }
    Some(if write_query_result().is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn helper_requested(mut arguments: impl Iterator<Item = OsString>) -> bool {
    arguments.next().as_deref() == Some(OsStr::new(HELPER_ARGUMENT)) && arguments.next().is_none()
}

fn write_query_result() -> Result<()> {
    anyhow::ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "image discovery requires an inherited terminal"
    );
    let record = query_record(
        &mut io::stdin().lock().take(MAX_QUERY_BYTES),
        &mut io::stdout().lock(),
        ImageProtocol::detect(),
        picker_for_window(window_size().ok().as_ref()),
    )?;
    record.write(&mut io::stderr().lock())?;
    Ok(())
}

fn query_record(
    reader: &mut impl Read,
    writer: &mut impl Write,
    inferred: ImageProtocol,
    picker: ImagePicker,
) -> Result<PickerRecord> {
    let query = Parser::query(
        picker.is_tmux,
        QueryStdioOptions {
            timeout: QUERY_BUDGET,
            text_sizing_protocol: false,
        },
    );
    writer.write_all(query.as_bytes())?;
    writer.flush()?;
    let mut parser = Parser::new();
    let mut record = PickerRecord {
        protocol: inferred,
        font_size: picker.font_size,
    };
    let mut bytes = [0; 128];
    loop {
        let count = reader.read(&mut bytes)?;
        anyhow::ensure!(
            count != 0,
            "terminal discovery ended before its status reply"
        );
        for byte in bytes.iter().take(count) {
            for response in parser.push(char::from(*byte)) {
                match response {
                    Response::Kitty if !inferred.is_native() => {
                        record.protocol = ImageProtocol::Kitty;
                    }
                    Response::Sixel
                        if !inferred.is_native() && record.protocol != ImageProtocol::Kitty =>
                    {
                        record.protocol = ImageProtocol::Sixel;
                    }
                    Response::CellSize(Some(size)) => record.font_size = size,
                    Response::Status => return Ok(record),
                    _ => {}
                }
            }
        }
    }
}

pub(crate) fn initialize_image_picker() -> Result<()> {
    if IMAGE_PICKER.get().is_none() {
        if tmux_detected() {
            crate::cli::terminal_query::enable_tmux_passthrough(QUERY_BUDGET)?;
        }
        let inferred = ImageProtocol::detect();
        let override_value = std::env::var("QUINJET_IMAGE_PROTOCOL").unwrap_or_default();
        let state = choose_picker(
            inferred,
            &override_value,
            window_size().ok().as_ref(),
            query_in_child,
        )?;
        let _state = IMAGE_PICKER.get_or_init(|| state);
    }
    Ok(())
}

fn choose_picker(
    inferred: ImageProtocol,
    override_value: &str,
    size: Option<&WindowSize>,
    query: impl FnOnce() -> Result<Option<PickerRecord>>,
) -> Result<(ImagePicker, ImageProtocol)> {
    let record = if override_value.eq_ignore_ascii_case("auto") {
        query()?
    } else {
        None
    };
    let Some(record) = record else {
        return Ok((picker_for_window(size), inferred));
    };
    let protocol = if inferred.is_native() {
        inferred
    } else {
        record.protocol
    };
    Ok((picker_with_font(record.font_size), protocol))
}

fn query_in_child() -> Result<Option<PickerRecord>> {
    if !is_raw_mode_enabled().unwrap_or(false) {
        return Ok(None);
    }
    let record =
        crate::cli::terminal_query::query_helper(HELPER_ARGUMENT, QUERY_BUDGET, MAX_RECORD_BYTES)?;
    Ok(record.and_then(|bytes| PickerRecord::parse(&bytes)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PickerRecord {
    protocol: ImageProtocol,
    font_size: (u16, u16),
}

impl PickerRecord {
    fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_RECORD_BYTES {
            return None;
        }
        let text = std::str::from_utf8(bytes).ok()?.strip_suffix('\n')?;
        let mut fields = text.split(' ');
        let protocol = match fields.next()? {
            "kitty" => ImageProtocol::Kitty,
            "iterm2" => ImageProtocol::Iterm2,
            "sixel" => ImageProtocol::Sixel,
            "halfblocks" => ImageProtocol::Halfblocks,
            _ => return None,
        };
        let width = fields.next()?.parse().ok()?;
        let height = fields.next()?.parse().ok()?;
        if width == 0 || height == 0 || fields.next().is_some() {
            return None;
        }
        Some(Self {
            protocol,
            font_size: (width, height),
        })
    }

    fn write(self, writer: &mut impl Write) -> io::Result<()> {
        let protocol = match self.protocol {
            ImageProtocol::Kitty => "kitty",
            ImageProtocol::Iterm2 => "iterm2",
            ImageProtocol::Sixel => "sixel",
            ImageProtocol::Halfblocks => "halfblocks",
        };
        writeln!(
            writer,
            "{protocol} {} {}",
            self.font_size.0, self.font_size.1
        )?;
        writer.flush()
    }
}

fn picker_for_window(size: Option<&WindowSize>) -> ImagePicker {
    let font_size = size
        .and_then(|size| {
            let width = size.width.checked_div(size.columns)?;
            let height = size.height.checked_div(size.rows)?;
            (width > 0 && height > 0).then_some((width, height))
        })
        .unwrap_or((10, 20));
    picker_with_font(font_size)
}

pub(super) fn fallback_picker() -> ImagePicker {
    picker_for_window(None)
}

fn picker_with_font(font_size: (u16, u16)) -> ImagePicker {
    ImagePicker {
        font_size,
        is_tmux: tmux_detected(),
    }
}

fn tmux_detected() -> bool {
    std::env::var("TERM").is_ok_and(|term| term.starts_with("tmux"))
        || std::env::var("TERM_PROGRAM").is_ok_and(|program| program == "tmux")
}

pub(in crate::ui) fn selected_image_protocol() -> ImageProtocol {
    IMAGE_PICKER
        .get()
        .map_or_else(ImageProtocol::detect, |(_, protocol)| *protocol)
}

#[cfg(test)]
mod tests;
