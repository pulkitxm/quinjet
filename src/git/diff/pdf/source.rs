use std::io::{self, Write};

use anyhow::{Result, ensure};
use lopdf::{Document, LoadOptions, Object};

pub(super) const SOURCE_LIMIT: usize = 8 * 1024 * 1024;

pub(super) fn decoded_source(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut document = Document::load_mem_with_options(
        bytes,
        LoadOptions::with_max_decompressed_size(SOURCE_LIMIT),
    )?;
    ensure!(!document.is_encrypted(), "PDF requires a password");
    let mut remaining = SOURCE_LIMIT;
    for object in document.objects.values_mut() {
        if let Object::Stream(stream) = object {
            let decoded = stream.get_plain_content_with_limit(remaining)?;
            remaining = remaining.saturating_sub(decoded.len());
            stream.set_plain_content(decoded);
        }
    }
    let mut output = BoundedBytes::default();
    document.save_to(&mut output)?;
    Ok(output.bytes)
}

pub(super) fn escaped_source(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut output = BoundedBytes::default();
    let mut column = 0;
    for byte in bytes {
        match byte {
            b'\n' => {
                output.write_all(b"\n")?;
                column = 0;
            }
            b'\\' => {
                output.write_all(b"\\\\")?;
                column += 2;
            }
            0x20..=0x7e => {
                output.write_all(&[*byte])?;
                column += 1;
            }
            _ => {
                write!(output, "\\x{byte:02x}")?;
                column += 4;
            }
        }
        if column >= 160 {
            output.write_all(b"\\\n")?;
            column = 0;
        }
    }
    Ok(output.bytes)
}

#[derive(Debug, Default)]
struct BoundedBytes {
    bytes: Vec<u8>,
}

impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > SOURCE_LIMIT.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("PDF source exceeds the 8 MiB limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
