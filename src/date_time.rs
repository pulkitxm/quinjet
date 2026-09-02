use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

const MINUTE_SECONDS: i64 = 60;
const HOUR_SECONDS: i64 = 60 * MINUTE_SECONDS;
const DAY_SECONDS: i64 = 24 * HOUR_SECONDS;
const MONTH_SECONDS: i64 = 30 * DAY_SECONDS;
const YEAR_SECONDS: i64 = 365 * DAY_SECONDS;
const RELATIVE_TIME_GENERATION_SECONDS: i64 = 10;

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

pub(crate) fn format_relative_timestamp(value: &str) -> String {
    format_relative_timestamp_at(value, OffsetDateTime::now_utc())
}

#[doc = " The current instant as an RFC 3339 stamp, so a recorded timestamp reads"]
#[doc = " back through the same formatter as a GitHub one."]
pub(crate) fn now_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub(crate) fn relative_time_generation() -> i64 {
    OffsetDateTime::now_utc()
        .unix_timestamp()
        .div_euclid(RELATIVE_TIME_GENERATION_SECONDS)
}

fn format_relative_timestamp_at(value: &str, now: OffsetDateTime) -> String {
    let Ok(timestamp) = OffsetDateTime::parse(value, &Rfc3339) else {
        return value.to_owned();
    };
    let seconds = (now - timestamp).whole_seconds();
    let future = seconds.is_negative();
    let magnitude = seconds.saturating_abs();
    if magnitude < MINUTE_SECONDS {
        return if future {
            "in a moment".to_owned()
        } else {
            "just now".to_owned()
        };
    }
    let (amount, unit) = if magnitude < HOUR_SECONDS {
        (magnitude.div_euclid(MINUTE_SECONDS), "minute")
    } else if magnitude < DAY_SECONDS {
        (magnitude.div_euclid(HOUR_SECONDS), "hour")
    } else if magnitude < MONTH_SECONDS {
        (magnitude.div_euclid(DAY_SECONDS), "day")
    } else if magnitude < YEAR_SECONDS {
        (magnitude.div_euclid(MONTH_SECONDS), "month")
    } else {
        (magnitude.div_euclid(YEAR_SECONDS), "year")
    };
    let plural = if amount == 1 { "" } else { "s" };
    if future {
        format!("in {amount} {unit}{plural}")
    } else {
        format!("{amount} {unit}{plural} ago")
    }
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
        assert_eq!(
            format_relative_timestamp_at("recently", OffsetDateTime::UNIX_EPOCH),
            "recently"
        );
    }

    #[test]
    fn formats_relative_time_across_display_units() {
        let now = OffsetDateTime::parse("2026-08-22T18:00:00Z", &Rfc3339).unwrap();
        let cases = [
            ("2026-08-22T17:59:31Z", "just now"),
            ("2026-08-22T17:59:00Z", "1 minute ago"),
            ("2026-08-22T17:58:00Z", "2 minutes ago"),
            ("2026-08-22T16:00:00Z", "2 hours ago"),
            ("2026-08-20T18:00:00Z", "2 days ago"),
            ("2026-06-22T18:00:00Z", "2 months ago"),
            ("2024-08-22T18:00:00Z", "2 years ago"),
            ("2026-08-22T18:02:00Z", "in 2 minutes"),
        ];

        for (value, expected) in cases {
            assert_eq!(format_relative_timestamp_at(value, now), expected);
        }
    }
}
