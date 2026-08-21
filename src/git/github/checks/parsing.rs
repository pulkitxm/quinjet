#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

/// GitHub answers the log endpoint with 404 until a job has finished writing
/// its archive, and with 410 once retention expires. Neither is a failure worth
/// showing: the run itself is still readable from its steps.
pub(super) fn log_not_published(output: &BoundedOutput) -> bool {
    let error = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    ["404", "410", "not found", "gone"]
        .into_iter()
        .any(|marker| error.contains(marker))
}

pub(super) fn rejects_unknown_flag(output: &BoundedOutput) -> bool {
    String::from_utf8_lossy(&output.stderr)
        .to_ascii_lowercase()
        .contains("unknown flag")
}

pub(super) fn pull_request_checks_args(pull_request: &PullRequest) -> Vec<OsString> {
    vec![
        OsString::from("pr"),
        OsString::from("checks"),
        OsString::from(pull_request.number.to_string()),
        OsString::from("--repo"),
        OsString::from(pull_request.base_repository.selector()),
        OsString::from("--json"),
        OsString::from("bucket,completedAt,description,link,name,startedAt,state,workflow"),
        OsString::from("--jq"),
        OsString::from(CHECK_TSV_JQ),
    ]
}

pub(super) fn parse_pull_request_checks(output: &[u8]) -> Result<Vec<PullRequestCheck>> {
    let mut checks = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [
            name,
            workflow,
            state,
            bucket,
            description,
            link,
            started_at,
            completed_at,
        ] = parse_tsv_record::<CHECK_TSV_FIELDS>(record)
            .with_context(|| format!("invalid pull-request check record {}", index + 1))?;
        let status = match bucket.to_ascii_lowercase().as_str() {
            "pass" => PullRequestCheckStatus::Passed,
            "fail" => PullRequestCheckStatus::Failed,
            "pending" => PullRequestCheckStatus::Pending,
            "skipping" => PullRequestCheckStatus::Skipped,
            "cancel" => PullRequestCheckStatus::Cancelled,
            _ => PullRequestCheckStatus::Unknown,
        };
        checks.push(PullRequestCheck {
            name,
            workflow,
            state,
            status,
            description,
            link,
            started_at,
            completed_at,
        });
    }
    checks.sort_by_key(|check| (check.workflow.to_lowercase(), check.name.to_lowercase()));
    Ok(checks)
}

pub(super) fn parse_check_steps(output: &[u8]) -> Result<Vec<CheckStep>> {
    let mut steps = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [number, name, status, conclusion, started_at, completed_at] =
            parse_tsv_record::<STEP_TSV_FIELDS>(record)
                .with_context(|| format!("invalid check step record {}", index + 1))?;
        steps.push(CheckStep {
            number: number.parse().unwrap_or(index + 1),
            status: PullRequestCheckStatus::from_conclusion(&status, &conclusion),
            name,
            conclusion,
            started_at,
            completed_at,
            lines: Vec::new(),
        });
    }
    steps.sort_by_key(|step| step.number);
    Ok(steps)
}

/// Runner logs are one timestamped line per row, carrying ANSI color and
/// `##[...]` workflow commands. Both are stripped here so the renderer only
/// deals with text plus a severity.
pub(super) fn parse_check_log(raw: &[u8]) -> (Vec<CheckLogLine>, bool) {
    let text = String::from_utf8_lossy(raw);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut lines = Vec::new();
    let mut limit_reached = false;
    for raw_line in text.lines() {
        if lines.len() >= MAX_CHECK_LOG_LINES {
            limit_reached = true;
            break;
        }
        let (timestamp, rest) = split_log_timestamp(raw_line);
        let rest = strip_ansi(rest);
        let (severity, text) = split_log_marker(&rest);
        lines.push(CheckLogLine {
            timestamp: timestamp.to_owned(),
            text,
            severity,
        });
    }
    (lines, limit_reached)
}

pub(super) fn split_log_timestamp(line: &str) -> (&str, &str) {
    let Some((candidate, rest)) = line.split_once(' ') else {
        return ("", line);
    };
    if is_log_timestamp(candidate) {
        (candidate, rest)
    } else {
        ("", line)
    }
}

pub(super) fn is_log_timestamp(value: &str) -> bool {
    value.len() >= 20
        && value.ends_with('Z')
        && value
            .as_bytes()
            .get(..4)
            .is_some_and(|year| year.iter().all(u8::is_ascii_digit))
        && value.as_bytes().get(4) == Some(&b'-')
        && value.contains('T')
}

pub(super) fn strip_ansi(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }
        match characters.next() {
            Some('[') => {
                for next in characters.by_ref() {
                    if !matches!(next, '0'..='9' | ';' | '?' | ':') {
                        break;
                    }
                }
            }
            Some(']') => {
                for next in characters.by_ref() {
                    if next == '\u{7}' || next == '\u{1b}' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    output
}

pub(super) fn split_log_marker(value: &str) -> (CheckLogSeverity, String) {
    for (marker, severity) in [
        ("##[error]", CheckLogSeverity::Error),
        ("##[warning]", CheckLogSeverity::Warning),
        ("##[notice]", CheckLogSeverity::Notice),
        ("##[command]", CheckLogSeverity::Command),
        ("##[group]", CheckLogSeverity::Command),
        ("##[debug]", CheckLogSeverity::Normal),
        ("[command]", CheckLogSeverity::Command),
    ] {
        if let Some(rest) = value.strip_prefix(marker) {
            return (severity, rest.to_owned());
        }
    }
    if value.starts_with("##[endgroup]") || value.starts_with("##[section]") {
        return (CheckLogSeverity::Normal, String::new());
    }
    (CheckLogSeverity::Normal, value.to_owned())
}

/// Distribute timestamped lines across steps in a single forward pass, moving on
/// as soon as the next step has started. Comparing whole seconds matters:
/// runner lines carry sub-second precision while the steps API reports whole
/// seconds, and comparing those as text puts everything written during a step's
/// final second into the step before it.
///
/// Output from before the first step or after the last one is returned loose,
/// which is where provisioning and teardown failures live.
pub(super) fn assign_lines_to_steps(
    steps: &mut [CheckStep],
    lines: Vec<CheckLogLine>,
) -> Vec<CheckLogLine> {
    if steps.is_empty() {
        return lines;
    }
    let starts: Vec<Option<i64>> = steps
        .iter()
        .map(|step| timestamp_seconds(&step.started_at))
        .collect();
    let mut loose = Vec::new();
    let mut current: Option<usize> = None;
    for line in lines {
        if let Some(seconds) = timestamp_seconds(&line.timestamp) {
            while let Some(next) = current.map_or(Some(0), |index| {
                (index + 1 < steps.len()).then_some(index + 1)
            }) {
                if starts
                    .get(next)
                    .copied()
                    .flatten()
                    .is_some_and(|start| seconds >= start)
                {
                    current = Some(next);
                } else {
                    break;
                }
            }
            let past_last = current.is_some_and(|index| {
                index + 1 == steps.len()
                    && steps.get(index).is_some_and(|step| {
                        timestamp_seconds(&step.completed_at).is_some_and(|end| seconds > end)
                    })
            });
            if past_last {
                loose.push(line);
                continue;
            }
        }
        match current.and_then(|index| steps.get_mut(index)) {
            Some(step) => step.lines.push(line),
            None => loose.push(line),
        }
    }
    loose
}

/// Render an elapsed span between two RFC 3339 stamps, or nothing when either
/// is missing or the pair does not describe a forward span.
pub(super) fn elapsed_label(started_at: &str, completed_at: &str) -> String {
    elapsed_seconds(started_at, completed_at).map_or_else(String::new, format_elapsed)
}

#[expect(
    clippy::integer_division,
    reason = "splitting seconds into whole minutes and hours is the point"
)]
pub(super) fn format_elapsed(seconds: i64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3_600, (seconds % 3_600) / 60)
    }
}

/// Both stamps are RFC 3339 in UTC, so a fixed-width field comparison is enough
/// to measure an elapsed span without pulling in a date library.
pub(super) fn elapsed_seconds(started_at: &str, completed_at: &str) -> Option<i64> {
    let start = timestamp_seconds(started_at)?;
    let end = timestamp_seconds(completed_at)?;
    (end >= start).then_some(end - start)
}

pub(super) fn timestamp_seconds(value: &str) -> Option<i64> {
    let (date, rest) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;
    let time = rest.split(['Z', '+', '.']).next()?;
    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let second: i64 = time_parts.next().unwrap_or("0").parse().ok()?;
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Howard Hinnant's civil-to-days algorithm, valid across the proleptic
/// Gregorian calendar.
#[expect(
    clippy::integer_division,
    reason = "the civil-to-days algorithm is defined in truncating arithmetic"
)]
pub(super) const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
