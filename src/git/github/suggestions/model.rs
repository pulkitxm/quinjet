#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Why GitHub itself says a suggestion is no longer applicable. Problems"]
#[doc = " found in the working tree are reported by the plan instead, because"]
#[doc = " they depend on the checkout rather than on the pull request."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub(crate) enum SuggestionBlocker {
    #[doc = " A later commit changed the code the suggestion was written against,"]
    #[doc = " so its line numbers no longer mean anything."]
    Outdated,
    #[doc = " The thread has been resolved already."]
    Resolved,
    #[doc = " GitHub did not report a line range for the thread."]
    NoLineRange,
}

impl SuggestionBlocker {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::Outdated => "a later commit moved the code it was written against".to_owned(),
            Self::Resolved => "its thread is resolved".to_owned(),
            Self::NoLineRange => "GitHub reported no line range for it".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Suggestion {
    #[doc = " The review comment's node id, which is what `apply` takes."]
    pub id: String,
    pub thread_id: String,
    pub author: String,
    pub path: PathBuf,
    #[doc = " First line the suggestion replaces, one-based and inclusive."]
    pub start_line: usize,
    #[doc = " Last line the suggestion replaces, one-based and inclusive."]
    pub end_line: usize,
    #[doc = " The replacement, without its fence and without a trailing newline."]
    pub replacement: String,
    #[doc = " The comment text around the suggestion block."]
    pub comment: String,
    pub url: String,
    pub outdated: bool,
    pub resolved: bool,
    #[doc = " Absent when the suggestion can be applied to the working tree."]
    pub blocker: Option<SuggestionBlocker>,
}

impl Suggestion {
    pub(crate) const fn is_applicable(&self) -> bool {
        self.blocker.is_none()
    }

    pub(crate) fn location(&self) -> String {
        if self.end_line > self.start_line {
            return format!(
                "{}:{}-{}",
                self.path.display(),
                self.start_line,
                self.end_line
            );
        }
        format!("{}:{}", self.path.display(), self.start_line)
    }

    #[doc = " How many lines it removes and adds, which is what a listing shows"]
    #[doc = " instead of the whole replacement."]
    pub(crate) fn counts(&self) -> (usize, usize) {
        let removed = self.end_line.saturating_sub(self.start_line) + 1;
        let added = if self.replacement.is_empty() {
            0
        } else {
            self.replacement.lines().count()
        };
        (removed, added)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestSuggestions {
    pub schema_version: u8,
    pub number: u64,
    pub head_oid: String,
    pub suggestions: Vec<Suggestion>,
    pub applicable: usize,
    pub blocked: usize,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

impl PullRequestSuggestions {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    pub(crate) fn finish(&mut self) {
        self.suggestions.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.start_line.cmp(&right.start_line))
                .then_with(|| left.id.cmp(&right.id))
        });
        self.applicable = self
            .suggestions
            .iter()
            .filter(|suggestion| suggestion.is_applicable())
            .count();
        self.blocked = self.suggestions.len() - self.applicable;
        self.schema_version = Self::SCHEMA_VERSION;
    }

    pub(crate) fn select(&self, wanted: &str) -> Result<&Suggestion> {
        let matches: Vec<&Suggestion> = self
            .suggestions
            .iter()
            .filter(|suggestion| suggestion.id == wanted || suggestion.id.starts_with(wanted))
            .collect();
        match matches.as_slice() {
            [only] => Ok(only),
            [] => bail!("no suggestion on this pull request has the id `{wanted}`"),
            _ => bail!("`{wanted}` matches more than one suggestion"),
        }
    }

    pub(crate) fn applicable_suggestions(&self) -> Vec<&Suggestion> {
        self.suggestions
            .iter()
            .filter(|suggestion| suggestion.is_applicable())
            .collect()
    }
}
