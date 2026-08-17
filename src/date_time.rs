use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

const DISPLAY_FORMAT: &[time::format_description::BorrowedFormatItem<'_>] = format_description!(
    "[weekday repr:short] [month repr:short] [day padding:none] [hour repr:12 padding:none]:[minute] [period case:upper]"
);

pub(crate) fn format_local_timestamp(value: &str) -> String {
    let Ok(timestamp) = OffsetDateTime::parse(value, &Rfc3339) else {
        return value.to_owned();
    };
    let offset = UtcOffset::local_offset_at(timestamp).unwrap_or_else(|_| timestamp.offset());
    format_timestamp(timestamp, offset).unwrap_or_else(|| value.to_owned())
}

fn format_timestamp(timestamp: OffsetDateTime, offset: UtcOffset) -> Option<String> {
    timestamp.to_offset(offset).format(DISPLAY_FORMAT).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_the_requested_local_timestamp_shape() {
        let timestamp = OffsetDateTime::parse("2026-08-17T16:42:00Z", &Rfc3339).unwrap();

        assert_eq!(
            format_timestamp(timestamp, UtcOffset::UTC).as_deref(),
            Some("Mon Aug 17 4:42 PM")
        );
    }

    #[test]
    fn malformed_timestamps_remain_readable() {
        assert_eq!(format_local_timestamp("recently"), "recently");
    }
}
