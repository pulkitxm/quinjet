use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::Result;
use crossterm::terminal::{WindowSize, is_raw_mode_enabled, window_size};
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Picker, ProtocolType};

use super::{IMAGE_PICKER, ImageProtocol};

const HELPER_ARGUMENT: &str = "--internal-image-picker-query";
const QUERY_BUDGET: Duration = Duration::from_millis(500);
const MAX_RECORD_BYTES: usize = 64;

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
    let picker = Picker::from_query_stdio_with_options(QueryStdioOptions {
        timeout: QUERY_BUDGET,
        text_sizing_protocol: false,
    })?;
    let record = PickerRecord {
        protocol: match picker.protocol_type() {
            ProtocolType::Kitty => ImageProtocol::Kitty,
            ProtocolType::Iterm2 => ImageProtocol::Iterm2,
            ProtocolType::Sixel => ImageProtocol::Sixel,
            ProtocolType::Halfblocks => ImageProtocol::Halfblocks,
        },
        font_size: picker.font_size(),
    };
    record.write(&mut io::stderr().lock())?;
    Ok(())
}

pub(crate) fn initialize_image_picker() -> Result<()> {
    if IMAGE_PICKER.get().is_none() {
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
) -> Result<(Picker, ImageProtocol)> {
    let record = if override_value.eq_ignore_ascii_case("auto") {
        query()?
    } else {
        None
    };
    let Some(record) = record else {
        return Ok((picker_for_window(inferred, size), inferred));
    };
    let protocol = if inferred.is_native() {
        inferred
    } else {
        record.protocol
    };
    Ok((picker_with_font(protocol, record.font_size), protocol))
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

fn picker_for_window(protocol: ImageProtocol, size: Option<&WindowSize>) -> Picker {
    let font_size = size
        .and_then(|size| {
            let width = size.width.checked_div(size.columns)?;
            let height = size.height.checked_div(size.rows)?;
            (width > 0 && height > 0).then_some((width, height))
        })
        .unwrap_or((10, 20));
    picker_with_font(protocol, font_size)
}

#[expect(
    deprecated,
    reason = "the explicit font-size constructor keeps queries outside the terminal event reader"
)]
fn picker_with_font(protocol: ImageProtocol, font_size: (u16, u16)) -> Picker {
    let mut picker = Picker::from_fontsize(font_size);
    picker.set_protocol_type(match protocol {
        ImageProtocol::Kitty => ProtocolType::Kitty,
        ImageProtocol::Iterm2 => ProtocolType::Iterm2,
        ImageProtocol::Sixel => ProtocolType::Sixel,
        ImageProtocol::Halfblocks => ProtocolType::Halfblocks,
    });
    picker
}

pub(in crate::ui) fn selected_image_protocol() -> ImageProtocol {
    IMAGE_PICKER
        .get()
        .map_or_else(ImageProtocol::detect, |(_, protocol)| *protocol)
}

#[cfg(test)]
mod tests;
