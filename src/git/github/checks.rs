use std::ffi::OsString;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use super::{
    BoundedOutput, CacheLife, PullRequest, Repository, bounded_command_error, parse_tsv_record,
};

const MAX_CHECK_LOG_BYTES: usize = 8 * 1024 * 1024;
/// Check state is the one thing here that genuinely changes minute to minute,
/// so it is the one thing kept on a clock rather than on an identity.
const CHECK_LIST_CACHE_TTL: Duration = Duration::from_secs(30);
/// A ceiling on how much a single pull request will warm in the background.
const MAX_PREFETCHED_CHECK_LOGS: usize = 32;
const MAX_CHECK_LOG_LINES: usize = 200_000;
const CHECK_TSV_FIELDS: usize = 8;
const STEP_TSV_FIELDS: usize = 6;

const CHECK_TSV_JQ: &str = r#".[] | [.name, .workflow, .state, .bucket, (.description // ""), (.link // ""), (.startedAt // ""), (.completedAt // "")] | @tsv"#;
const JOB_STEPS_TSV_JQ: &str = r#".steps[]? | [((.number // 0)|tostring), (.name // ""), (.status // ""), (.conclusion // ""), (.started_at // ""), (.completed_at // "")] | @tsv"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PullRequestCheckStatus {
    Pending,
    Passed,
    Failed,
    Skipped,
    Cancelled,
    Unknown,
}

impl PullRequestCheckStatus {
    pub(crate) const fn is_running(self) -> bool {
        matches!(self, Self::Pending)
    }

    fn from_conclusion(status: &str, conclusion: &str) -> Self {
        if !status.eq_ignore_ascii_case("completed") {
            return Self::Pending;
        }
        match conclusion.to_ascii_lowercase().as_str() {
            "success" => Self::Passed,
            "failure" | "timed_out" | "action_required" => Self::Failed,
            "skipped" | "neutral" => Self::Skipped,
            "cancelled" | "stale" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestCheck {
    pub name: String,
    pub workflow: String,
    pub state: String,
    pub status: PullRequestCheckStatus,
    pub description: String,
    pub link: String,
    pub started_at: String,
    pub completed_at: String,
}

impl PullRequestCheck {
    pub(crate) fn duration_label(&self) -> String {
        elapsed_label(&self.started_at, &self.completed_at)
    }

    /// GitHub Actions check links end in `/actions/runs/<run>/job/<job>`, which
    /// is the only place a check run exposes the job identity its logs need.
    pub(crate) fn job_id(&self) -> Option<u64> {
        let (_, job) = self.link.rsplit_once("/job/")?;
        let job = job.split(['?', '#', '/']).next()?;
        job.parse().ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CheckLogSeverity {
    Normal,
    Command,
    Notice,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckLogLine {
    pub timestamp: String,
    pub text: String,
    pub severity: CheckLogSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckStep {
    pub number: usize,
    pub name: String,
    pub status: PullRequestCheckStatus,
    pub conclusion: String,
    pub started_at: String,
    pub completed_at: String,
    pub lines: Vec<CheckLogLine>,
}

impl CheckStep {
    /// How long the step took, or how long it has been running so far when it
    /// has started but not finished.
    pub(crate) fn duration_label(&self, now: i64) -> String {
        if self.completed_at.is_empty() {
            let Some(started) = timestamp_seconds(&self.started_at) else {
                return String::new();
            };
            return if now > started {
                format!("{}…", format_elapsed(now - started))
            } else {
                String::new()
            };
        }
        elapsed_label(&self.started_at, &self.completed_at)
    }
}

/// Seconds since the Unix epoch, for measuring against a GitHub timestamp.
pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .unwrap_or_default()
}

/// A check list plus where it came from, so the view can say whether it is
/// showing a cached answer or one just read from GitHub.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestChecks {
    pub checks: Vec<PullRequestCheck>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckRunLog {
    pub steps: Vec<CheckStep>,
    /// Output produced before the first step or after the last one, which is
    /// where a runner reports provisioning and teardown failures.
    pub loose_lines: Vec<CheckLogLine>,
    pub truncated: bool,
    /// Set when there is nothing to show at all, with the reason to display.
    pub unavailable: Option<String>,
    /// The runner has not written anything yet. This is only true for the first
    /// seconds of a job: GitHub serves a growing partial log from then on, so a
    /// running job tails rather than waiting for its own completion.
    pub log_pending: bool,
}

impl CheckRunLog {
    fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            unavailable: Some(reason.into()),
            ..Self::default()
        }
    }

    /// The step a runner is currently executing, if any.
    pub(crate) fn running_step(&self) -> Option<&CheckStep> {
        self.steps
            .iter()
            .find(|step| step.status == PullRequestCheckStatus::Pending)
    }

    pub(crate) fn failed_step(&self) -> Option<&CheckStep> {
        self.steps
            .iter()
            .find(|step| step.status == PullRequestCheckStatus::Failed)
    }
}

impl Repository {
    /// `gh pr checks` exits non-zero when any run failed, so a useful response
    /// has to be recognized by its content rather than by the exit status. That
    /// is why this reads `gh` directly instead of going through the cached
    /// helper, and caches the accepted body itself.
    pub(crate) fn pull_request_checks(
        &self,
        pull_request: &PullRequest,
        refresh: bool,
    ) -> Result<PullRequestChecks> {
        let key = format!(
            "checks-v1\n{}\n{}\n{}",
            pull_request.base_repository.url.trim_end_matches('/'),
            pull_request.number,
            pull_request.head_oid
        );
        if !refresh
            && let Some(cached) = super::cache_read(&key, CacheLife::Ttl(CHECK_LIST_CACHE_TTL))
        {
            return Ok(PullRequestChecks {
                checks: parse_pull_request_checks(&cached)?,
                from_cache: true,
            });
        }
        let output = self.run_gh(pull_request_checks_args(pull_request))?;
        let accepted_status = output.status.success()
            || matches!(output.status.code(), Some(1 | 8)) && !output.stdout.is_empty();
        if output.stdout_truncated {
            bail!("pull-request checks exceeded the metadata limit");
        }
        if !accepted_status {
            let error = String::from_utf8_lossy(&output.stderr);
            if error.to_ascii_lowercase().contains("no checks") {
                return Ok(PullRequestChecks::default());
            }
            bail!(
                "{}",
                bounded_command_error("unable to load pull-request checks", &output)
            );
        }
        let checks = parse_pull_request_checks(&output.stdout)?;
        super::cache_write(&key, &output.stdout);
        Ok(PullRequestChecks {
            checks,
            from_cache: false,
        })
    }

    /// Read a check run's steps and its raw log, then attach every log line to
    /// the step whose run window contains it. Runner output is timestamped in
    /// UTC and the steps API reports the same clock, so the ranges map exactly
    /// without guessing at group headings.
    ///
    /// The log endpoint serves whatever a running job has written so far, so
    /// repeating this call while a job runs is what makes the view tail it. Only
    /// the first seconds of a job answer 404, before the blob exists at all.
    pub(crate) fn pull_request_check_log(
        &self,
        pull_request: &PullRequest,
        check: &PullRequestCheck,
    ) -> Result<CheckRunLog> {
        let Some(job) = check.job_id() else {
            return Ok(CheckRunLog::unavailable(format!(
                "{} does not publish logs through GitHub Actions",
                check.name
            )));
        };
        let repository = &pull_request.base_repository.name_with_owner;
        let life = if check.status.is_running() {
            CacheLife::Ttl(Duration::ZERO)
        } else {
            CacheLife::Immutable
        };
        let mut steps = self.check_run_steps(repository, job, life)?;
        let (raw, truncated) = self.check_run_raw_log(repository, job, life)?;
        if raw.is_empty() && steps.is_empty() {
            return Ok(CheckRunLog::unavailable(
                "GitHub has not published anything for this check yet".to_owned(),
            ));
        }
        let (lines, line_limit_reached) = parse_check_log(&raw);
        let loose_lines = assign_lines_to_steps(&mut steps, lines);
        Ok(CheckRunLog {
            steps,
            loose_lines,
            truncated: truncated || line_limit_reached,
            unavailable: None,
            log_pending: raw.is_empty(),
        })
    }

    /// Read every finished run into the cache so that selecting any of them is
    /// answered from disk. Runs still in progress are skipped: their output is
    /// not cacheable, and re-reading it here would spend requests the live tail
    /// is about to spend anyway.
    pub(crate) fn prefetch_check_run_logs(
        &self,
        pull_request: &PullRequest,
        checks: &[PullRequestCheck],
        wanted: &dyn Fn() -> bool,
    ) -> usize {
        checks
            .iter()
            .filter(|check| !check.status.is_running() && check.job_id().is_some())
            .take(MAX_PREFETCHED_CHECK_LOGS)
            .take_while(|_| wanted())
            .filter(|check| self.pull_request_check_log(pull_request, check).is_ok())
            .count()
    }

    fn check_run_steps(
        &self,
        repository: &str,
        job: u64,
        life: CacheLife,
    ) -> Result<Vec<CheckStep>> {
        let response = self.checked_cached_gh(
            &format!("check-steps-v1\n{repository}\n{job}\n{life:?}"),
            life,
            false,
            [
                OsString::from("api"),
                OsString::from(format!("repos/{repository}/actions/jobs/{job}")),
                OsString::from("--jq"),
                OsString::from(JOB_STEPS_TSV_JQ),
            ],
            "unable to read the check run steps",
        );
        match response {
            Err(_) => Ok(Vec::new()),
            Ok(response) => parse_check_steps(&response.data),
        }
    }

    fn check_run_raw_log(
        &self,
        repository: &str,
        job: u64,
        life: CacheLife,
    ) -> Result<(Vec<u8>, bool)> {
        let key = format!("check-log-v1\n{repository}\n{job}");
        if life == CacheLife::Immutable
            && let Some(cached) = super::cache_read_bounded(&key, life, MAX_CHECK_LOG_BYTES)
        {
            return Ok((cached, false));
        }
        let endpoint = format!("repos/{repository}/actions/jobs/{job}/logs");
        let output = self.run_gh_log([
            OsString::from("api"),
            OsString::from("--allow-escape-sequences"),
            OsString::from(&endpoint),
        ])?;
        let output = if output.status.success() || !rejects_unknown_flag(&output) {
            output
        } else {
            self.run_gh_log([OsString::from("api"), OsString::from(&endpoint)])?
        };
        if !output.status.success() && !output.stdout_truncated {
            if log_not_published(&output) {
                return Ok((Vec::new(), false));
            }
            bail!(
                "{}",
                bounded_command_error("unable to read the check run log", &output)
            );
        }
        if life == CacheLife::Immutable && !output.stdout_truncated && !output.stdout.is_empty() {
            super::cache_write_bounded(&key, &output.stdout, MAX_CHECK_LOG_BYTES);
        }
        Ok((output.stdout, output.stdout_truncated))
    }

    fn run_gh_log<I>(&self, args: I) -> Result<BoundedOutput>
    where
        I: IntoIterator<Item = OsString>,
    {
        self.run_gh_bounded(args, MAX_CHECK_LOG_BYTES)
    }
}

/// GitHub answers the log endpoint with 404 until a job has finished writing
/// its archive, and with 410 once retention expires. Neither is a failure worth
/// showing: the run itself is still readable from its steps.
fn log_not_published(output: &BoundedOutput) -> bool {
    let error = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    ["404", "410", "not found", "gone"]
        .into_iter()
        .any(|marker| error.contains(marker))
}

fn rejects_unknown_flag(output: &BoundedOutput) -> bool {
    String::from_utf8_lossy(&output.stderr)
        .to_ascii_lowercase()
        .contains("unknown flag")
}

fn pull_request_checks_args(pull_request: &PullRequest) -> Vec<OsString> {
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

fn parse_pull_request_checks(output: &[u8]) -> Result<Vec<PullRequestCheck>> {
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

fn parse_check_steps(output: &[u8]) -> Result<Vec<CheckStep>> {
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
fn parse_check_log(raw: &[u8]) -> (Vec<CheckLogLine>, bool) {
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

fn split_log_timestamp(line: &str) -> (&str, &str) {
    let Some((candidate, rest)) = line.split_once(' ') else {
        return ("", line);
    };
    if is_log_timestamp(candidate) {
        (candidate, rest)
    } else {
        ("", line)
    }
}

fn is_log_timestamp(value: &str) -> bool {
    value.len() >= 20
        && value.ends_with('Z')
        && value
            .as_bytes()
            .get(..4)
            .is_some_and(|year| year.iter().all(u8::is_ascii_digit))
        && value.as_bytes().get(4) == Some(&b'-')
        && value.contains('T')
}

fn strip_ansi(value: &str) -> String {
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

fn split_log_marker(value: &str) -> (CheckLogSeverity, String) {
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
fn assign_lines_to_steps(steps: &mut [CheckStep], lines: Vec<CheckLogLine>) -> Vec<CheckLogLine> {
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
fn format_elapsed(seconds: i64) -> String {
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
fn elapsed_seconds(started_at: &str, completed_at: &str) -> Option<i64> {
    let start = timestamp_seconds(started_at)?;
    let end = timestamp_seconds(completed_at)?;
    (end >= start).then_some(end - start)
}

fn timestamp_seconds(value: &str) -> Option<i64> {
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
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(link: &str) -> PullRequestCheck {
        PullRequestCheck {
            name: "Format, lint, and test (ubuntu-latest)".to_owned(),
            workflow: "CI".to_owned(),
            state: "SUCCESS".to_owned(),
            status: PullRequestCheckStatus::Passed,
            description: String::new(),
            link: link.to_owned(),
            started_at: "2026-08-14T18:55:20Z".to_owned(),
            completed_at: "2026-08-14T18:55:52Z".to_owned(),
        }
    }

    #[test]
    fn reads_the_job_identity_only_from_an_actions_check_link() {
        assert_eq!(
            check("https://github.com/acme/widget/actions/runs/123/job/456").job_id(),
            Some(456)
        );
        assert_eq!(
            check("https://github.com/acme/widget/actions/runs/123/job/456?pr=7").job_id(),
            Some(456)
        );
        assert_eq!(check("https://ci.example.test/build/9").job_id(), None);
        assert_eq!(check("").job_id(), None);
    }

    fn failing_status() -> std::process::ExitStatus {
        std::process::Command::new("false").status().unwrap()
    }

    #[test]
    fn an_unpublished_log_is_pending_rather_than_a_failure() {
        let mut output = BoundedOutput {
            status: failing_status(),
            stdout: Vec::new(),
            stderr: b"gh: HTTP 404".to_vec(),
            stdout_truncated: false,
        };
        assert!(
            log_not_published(&output),
            "a job that has not finished writing its archive is not an error"
        );

        output.stderr = b"gh: Gone (HTTP 410)".to_vec();
        assert!(
            log_not_published(&output),
            "expired retention is not either"
        );

        output.stderr = b"gh: HTTP 500 Internal Server Error".to_vec();
        assert!(!log_not_published(&output));
    }

    #[test]
    fn strips_timestamps_ansi_and_workflow_commands_from_log_lines() {
        let raw = "\u{feff}2026-08-14T18:59:57.3510133Z Current runner version: '2.336.0'\n\
2026-08-14T18:59:57.3533811Z ##[group]Runner Image Provisioner\n\
2026-08-14T18:59:57.3534599Z \u{1b}[36mHosted Compute Agent\u{1b}[0m\n\
2026-08-14T18:59:57.3539925Z ##[endgroup]\n\
2026-08-14T19:00:09.0000000Z ##[error]cargo test failed\n\
untimestamped trailing output\n";

        let (lines, limit_reached) = parse_check_log(raw.as_bytes());

        assert!(!limit_reached);
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0].text, "Current runner version: '2.336.0'");
        assert_eq!(lines[0].timestamp, "2026-08-14T18:59:57.3510133Z");
        assert_eq!(
            (lines[1].severity, lines[1].text.as_str()),
            (CheckLogSeverity::Command, "Runner Image Provisioner")
        );
        assert_eq!(
            lines[2].text, "Hosted Compute Agent",
            "color codes never reach the renderer"
        );
        assert_eq!(lines[3].text, "");
        assert_eq!(
            (lines[4].severity, lines[4].text.as_str()),
            (CheckLogSeverity::Error, "cargo test failed")
        );
        assert_eq!(lines[5].timestamp, "");
        assert_eq!(lines[5].text, "untimestamped trailing output");
    }

    #[test]
    fn attaches_each_log_line_to_the_step_that_was_running() {
        let mut steps = vec![
            CheckStep {
                number: 1,
                name: "Set up job".to_owned(),
                status: PullRequestCheckStatus::Passed,
                conclusion: "success".to_owned(),
                started_at: "2026-08-14T18:00:00Z".to_owned(),
                completed_at: "2026-08-14T18:00:10Z".to_owned(),
                lines: Vec::new(),
            },
            CheckStep {
                number: 2,
                name: "Run cargo test".to_owned(),
                status: PullRequestCheckStatus::Failed,
                conclusion: "failure".to_owned(),
                started_at: "2026-08-14T18:00:10Z".to_owned(),
                completed_at: "2026-08-14T18:02:30Z".to_owned(),
                lines: Vec::new(),
            },
        ];
        let line = |timestamp: &str, text: &str| CheckLogLine {
            timestamp: timestamp.to_owned(),
            text: text.to_owned(),
            severity: CheckLogSeverity::Normal,
        };

        let loose = assign_lines_to_steps(
            &mut steps,
            vec![
                line("2026-08-14T17:59:59Z", "provisioning"),
                line("2026-08-14T18:00:01Z", "setting up"),
                line("2026-08-14T18:00:11Z", "running tests"),
                line("", "continuation of the previous line"),
                line("2026-08-14T18:05:00Z", "teardown"),
            ],
        );

        assert_eq!(
            steps[0]
                .lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>(),
            vec!["setting up"]
        );
        assert_eq!(
            steps[1]
                .lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>(),
            vec!["running tests", "continuation of the previous line"]
        );
        assert_eq!(
            loose.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            vec!["provisioning", "teardown"]
        );
        assert_eq!(steps[1].duration_label(0), "2m 20s");
        assert_eq!(steps[0].duration_label(0), "10s");
    }

    #[test]
    fn a_step_boundary_splits_on_whole_seconds_not_on_text_order() {
        let mut steps = vec![
            CheckStep {
                number: 1,
                name: "Set up job".to_owned(),
                status: PullRequestCheckStatus::Passed,
                conclusion: "success".to_owned(),
                started_at: "2026-08-14T18:59:57Z".to_owned(),
                completed_at: "2026-08-14T18:59:58Z".to_owned(),
                lines: Vec::new(),
            },
            CheckStep {
                number: 2,
                name: "Run actions/checkout@v5".to_owned(),
                status: PullRequestCheckStatus::Passed,
                conclusion: "success".to_owned(),
                started_at: "2026-08-14T18:59:58Z".to_owned(),
                completed_at: "2026-08-14T19:00:09Z".to_owned(),
                lines: Vec::new(),
            },
        ];
        let line = |timestamp: &str, text: &str| CheckLogLine {
            timestamp: timestamp.to_owned(),
            text: text.to_owned(),
            severity: CheckLogSeverity::Normal,
        };

        let loose = assign_lines_to_steps(
            &mut steps,
            vec![
                line("2026-08-14T18:59:57.3510133Z", "Current runner version"),
                line("2026-08-14T18:59:58.4821004Z", "Run actions/checkout@v5"),
                line("2026-08-14T19:00:08.1200000Z", "Getting Git version info"),
            ],
        );

        assert!(loose.is_empty());
        assert_eq!(
            steps[0]
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Current runner version"]
        );
        assert_eq!(
            steps[1]
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Run actions/checkout@v5", "Getting Git version info"]
        );
    }

    #[test]
    fn steps_without_a_log_keep_every_line_loose() {
        let mut steps = Vec::new();
        let lines = vec![CheckLogLine {
            timestamp: "2026-08-14T18:00:00Z".to_owned(),
            text: "only output".to_owned(),
            severity: CheckLogSeverity::Normal,
        }];

        let loose = assign_lines_to_steps(&mut steps, lines);

        assert_eq!(loose.len(), 1);
    }

    #[test]
    fn parses_job_steps_and_derives_status_from_the_conclusion() {
        let output =
            b"1\tSet up job\tcompleted\tsuccess\t2026-08-14T18:00:00Z\t2026-08-14T18:00:10Z\n\
3\tRun cargo test\tcompleted\tfailure\t2026-08-14T18:00:10Z\t2026-08-14T18:02:30Z\n\
2\tCheckout\tin_progress\t\t2026-08-14T18:00:10Z\t\n";

        let steps = parse_check_steps(output).unwrap();

        assert_eq!(
            steps.iter().map(|step| step.number).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "steps render in the runner's own order"
        );
        assert_eq!(steps[0].status, PullRequestCheckStatus::Passed);
        assert_eq!(steps[1].status, PullRequestCheckStatus::Pending);
        assert_eq!(steps[2].status, PullRequestCheckStatus::Failed);
        let started = timestamp_seconds("2026-08-14T18:00:10Z").unwrap();
        assert_eq!(steps[1].duration_label(started + 95), "1m 35s…");
        assert_eq!(
            steps[1].duration_label(started),
            "",
            "a step reports nothing until at least a second has passed"
        );
    }

    #[test]
    fn measures_elapsed_time_across_month_and_year_boundaries() {
        assert_eq!(
            elapsed_label("2026-02-28T23:59:30Z", "2026-03-01T00:00:30Z"),
            "1m 0s",
            "February ends on the 28th outside a leap year"
        );
        assert_eq!(
            elapsed_label("2024-02-28T12:00:00Z", "2024-03-01T12:30:00Z"),
            "48h 30m",
            "a leap year adds the extra day between the same two dates"
        );
        assert_eq!(
            elapsed_label("2025-12-31T23:00:00Z", "2026-01-01T01:15:00Z"),
            "2h 15m"
        );
        assert_eq!(elapsed_label("bad", "worse"), "");
        assert_eq!(
            elapsed_label("2026-08-14T18:00:00Z", "2026-08-14T17:00:00Z"),
            "",
            "a completion before its start is reported as unknown, never negative"
        );
    }

    #[test]
    fn parses_live_pull_request_checks_in_stable_name_order() {
        let output = b"tests\tCI\tSUCCESS\tpass\tall good\thttps://example.test/pass\tstart\tdone\nlint\tCI\tFAILURE\tfail\tbroken\thttps://example.test/fail\tstart\tdone\nbuild\tCI\tIN_PROGRESS\tpending\t\thttps://example.test/pending\tstart\t\n";

        let checks = parse_pull_request_checks(output).unwrap();

        assert_eq!(checks.len(), 3);
        assert_eq!(checks[0].name, "build");
        assert_eq!(checks[0].status, PullRequestCheckStatus::Pending);
        assert_eq!(checks[1].name, "lint");
        assert_eq!(checks[1].status, PullRequestCheckStatus::Failed);
        assert_eq!(checks[2].name, "tests");
        assert_eq!(checks[2].status, PullRequestCheckStatus::Passed);
        assert_eq!(checks[2].description, "all good");
    }

    #[test]
    fn a_warm_up_stops_as_soon_as_the_pull_request_it_serves_is_left() {
        let settled = |name: &str| PullRequestCheck {
            name: name.to_owned(),
            workflow: "CI".to_owned(),
            state: "SUCCESS".to_owned(),
            status: PullRequestCheckStatus::Passed,
            description: String::new(),
            link: "https://github.com/o/r/actions/runs/1/job/2".to_owned(),
            started_at: String::new(),
            completed_at: String::new(),
        };
        let repository = Repository {
            root: std::path::PathBuf::from("/nonexistent-on-purpose"),
        };
        let checks = [settled("one"), settled("two"), settled("three")];

        let warmed =
            repository.prefetch_check_run_logs(&PullRequest::default(), &checks, &|| false);
        assert_eq!(warmed, 0, "a superseded warm-up asks for nothing");
    }
}
