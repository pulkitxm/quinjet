#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AnnotationSeverity {
    Failure,
    Warning,
    Notice,
}

impl AnnotationSeverity {
    pub(crate) fn parse(level: &str) -> Self {
        match level.to_ascii_lowercase().as_str() {
            "failure" => Self::Failure,
            "warning" => Self::Warning,
            _ => Self::Notice,
        }
    }

    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Failure => "failure",
            Self::Warning => "warning",
            Self::Notice => "notice",
        }
    }

    pub(crate) const fn glyph(self) -> &'static str {
        match self {
            Self::Failure => "x",
            Self::Warning => "!",
            Self::Notice => "i",
        }
    }
}

#[doc = " Whether an annotation points at a line the pull request actually shows."]
#[doc = " An annotation on an untouched file, or on a line outside every hunk, is"]
#[doc = " still worth reading; it just cannot be drawn on the diff."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AnnotationPlacement {
    #[doc = " The path and line are both inside the pull request's patch."]
    InDiff,
    #[doc = " The pull request changes the file, but not at that line."]
    OutsideHunks,
    #[doc = " The pull request does not touch the file at all."]
    OutsideDiff,
    #[doc = " No patch was loaded for the file, so placement is unresolved."]
    Unknown,
}

impl AnnotationPlacement {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::InDiff => "in diff",
            Self::OutsideHunks => "outside hunks",
            Self::OutsideDiff => "outside diff",
            Self::Unknown => "unplaced",
        }
    }

    pub(crate) const fn is_in_diff(self) -> bool {
        matches!(self, Self::InDiff)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckAnnotation {
    #[doc = " The check run's name, which is also what `pr logs` accepts."]
    pub check: String,
    pub check_run_id: u64,
    pub check_url: String,
    pub path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub start_column: Option<usize>,
    pub end_column: Option<usize>,
    pub severity: AnnotationSeverity,
    pub title: String,
    pub message: String,
    pub raw_details: String,
    pub url: String,
    pub placement: AnnotationPlacement,
}

impl CheckAnnotation {
    #[doc = " `path:line` or `path:start-end`, which is the form an editor and a"]
    #[doc = " human both read without explanation."]
    pub(crate) fn location(&self) -> String {
        let path = self.path.display();
        if self.start_line == 0 {
            return path.to_string();
        }
        if self.end_line > self.start_line {
            return format!("{path}:{}-{}", self.start_line, self.end_line);
        }
        format!("{path}:{}", self.start_line)
    }

    #[doc = " The one line a queue row shows: the title when there is one, the first"]
    #[doc = " line of the message otherwise."]
    pub(crate) fn headline(&self) -> String {
        if !self.title.trim().is_empty() {
            return self.title.trim().to_owned();
        }
        self.message
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default()
            .to_owned()
    }

    #[doc = " The sort key: severity first, then path, then line, then check, so the"]
    #[doc = " same pull request always lists in the same order and `next` and"]
    #[doc = " `previous` mean something stable."]
    fn order(&self) -> (AnnotationSeverity, &Path, usize, usize, &str) {
        (
            self.severity,
            self.path.as_path(),
            self.start_line,
            self.end_line,
            self.check.as_str(),
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnnotationCounts {
    pub failure: usize,
    pub warning: usize,
    pub notice: usize,
    pub in_diff: usize,
    pub outside_diff: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestAnnotations {
    pub schema_version: u8,
    pub head_oid: String,
    pub annotations: Vec<CheckAnnotation>,
    pub counts: AnnotationCounts,
    #[doc = " Check runs that reported annotations Quinjet did not read, because the"]
    #[doc = " run cap or the annotation cap was reached."]
    pub truncated: bool,
    pub from_cache: bool,
    pub warnings: Vec<String>,
}

impl PullRequestAnnotations {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    pub(crate) fn finish(&mut self) {
        self.annotations
            .sort_by(|left, right| left.order().cmp(&right.order()));
        self.annotations.truncate(MAX_ANNOTATIONS);
        self.counts = AnnotationCounts::default();
        for annotation in &self.annotations {
            match annotation.severity {
                AnnotationSeverity::Failure => self.counts.failure += 1,
                AnnotationSeverity::Warning => self.counts.warning += 1,
                AnnotationSeverity::Notice => self.counts.notice += 1,
            }
            if annotation.placement.is_in_diff() {
                self.counts.in_diff += 1;
            } else {
                self.counts.outside_diff += 1;
            }
        }
        self.schema_version = Self::SCHEMA_VERSION;
    }

    #[doc = " Group rows the way a reader asked for, keeping the stable order inside"]
    #[doc = " each group."]
    pub(crate) fn grouped(&self, by: AnnotationGrouping) -> Vec<(String, Vec<&CheckAnnotation>)> {
        let mut groups: BTreeMap<String, Vec<&CheckAnnotation>> = BTreeMap::new();
        for annotation in &self.annotations {
            let key = match by {
                AnnotationGrouping::File => annotation.path.display().to_string(),
                AnnotationGrouping::Check => annotation.check.clone(),
                AnnotationGrouping::Severity => annotation.severity.word().to_owned(),
            };
            groups.entry(key).or_default().push(annotation);
        }
        groups.into_iter().collect()
    }

    pub(crate) const fn has_failures(&self) -> bool {
        self.counts.failure > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnnotationGrouping {
    File,
    Check,
    Severity,
}

#[doc = " What a caller wants out of the listing, applied before grouping so the"]
#[doc = " counts and the rows always agree."]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AnnotationFilter {
    pub severity: Option<AnnotationSeverity>,
    pub check: Option<String>,
    pub path: Option<PathBuf>,
    pub in_diff_only: bool,
}

impl AnnotationFilter {
    pub(crate) fn keeps(&self, annotation: &CheckAnnotation) -> bool {
        if self
            .severity
            .is_some_and(|severity| severity != annotation.severity)
        {
            return false;
        }
        if self.check.as_ref().is_some_and(|wanted| {
            !annotation
                .check
                .to_lowercase()
                .contains(&wanted.to_lowercase())
        }) {
            return false;
        }
        if self
            .path
            .as_ref()
            .is_some_and(|wanted| !annotation.path.starts_with(wanted))
        {
            return false;
        }
        !self.in_diff_only || annotation.placement.is_in_diff()
    }

    pub(crate) fn apply(&self, mut annotations: PullRequestAnnotations) -> PullRequestAnnotations {
        annotations
            .annotations
            .retain(|annotation| self.keeps(annotation));
        annotations.finish();
        annotations
    }
}
