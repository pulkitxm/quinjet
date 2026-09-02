#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " What the bundle is being assembled for. The purpose decides what is"]
#[doc = " included and in what order sections give up their space, so two callers"]
#[doc = " asking different questions do not get the same undifferentiated dump."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ContextPurpose {
    #[doc = " Reading the change: the patch first, then the threads."]
    Review,
    #[doc = " Answering reviewers: the threads first, then the patch around them."]
    AddressFeedback,
    #[doc = " Making CI green: the failures and their annotations first."]
    FixCi,
}

impl ContextPurpose {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::AddressFeedback => "address-feedback",
            Self::FixCi => "fix-ci",
        }
    }

    #[doc = " The order sections are filled in. Everything earlier gets its space"]
    #[doc = " before anything later, so what a purpose cares about survives a small"]
    #[doc = " budget and what it does not is what gets dropped."]
    pub(crate) const fn section_order(self) -> [ContextSectionKind; 5] {
        match self {
            Self::Review => [
                ContextSectionKind::Instructions,
                ContextSectionKind::Patch,
                ContextSectionKind::Threads,
                ContextSectionKind::Checks,
                ContextSectionKind::Dependencies,
            ],
            Self::AddressFeedback => [
                ContextSectionKind::Instructions,
                ContextSectionKind::Threads,
                ContextSectionKind::Patch,
                ContextSectionKind::Checks,
                ContextSectionKind::Dependencies,
            ],
            Self::FixCi => [
                ContextSectionKind::Instructions,
                ContextSectionKind::Checks,
                ContextSectionKind::Patch,
                ContextSectionKind::Threads,
                ContextSectionKind::Dependencies,
            ],
        }
    }

    #[doc = " The section the purpose exists for. A bundle that had to drop this"]
    #[doc = " one did not answer the question it was asked, which is worth saying"]
    #[doc = " out loud rather than leaving a caller to notice."]
    pub(crate) const fn primary_section(self) -> ContextSectionKind {
        match self {
            Self::Review => ContextSectionKind::Patch,
            Self::AddressFeedback => ContextSectionKind::Threads,
            Self::FixCi => ContextSectionKind::Checks,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ContextSectionKind {
    Instructions,
    Patch,
    Threads,
    Checks,
    Dependencies,
}

impl ContextSectionKind {
    pub(crate) const fn heading(self) -> &'static str {
        match self {
            Self::Instructions => "repository instructions",
            Self::Patch => "patch",
            Self::Threads => "unresolved review threads",
            Self::Checks => "failing checks",
            Self::Dependencies => "dependency changes",
        }
    }

    #[doc = " Whether the section's text came from people outside the repository."]
    #[doc = " A coding tool must be able to tell repository instructions from a"]
    #[doc = " comment anybody could have written, and the answer must not depend on"]
    #[doc = " reading the prose."]
    pub(crate) const fn is_untrusted(self) -> bool {
        !matches!(self, Self::Instructions)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextSection {
    pub kind: ContextSectionKind,
    pub heading: String,
    pub body: String,
    #[doc = " True when the body is written by people who can comment on the pull"]
    #[doc = " request rather than by the repository."]
    pub untrusted: bool,
    #[doc = " Characters dropped from this section to fit the budget."]
    pub dropped_characters: usize,
    #[doc = " Items left out entirely, such as threads past the ones that fit."]
    pub dropped_items: usize,
}

impl ContextSection {
    pub(crate) const fn is_truncated(&self) -> bool {
        self.dropped_characters > 0 || self.dropped_items > 0
    }
}

#[doc = " Where the bundle came from, so a tool acting on it can prove which"]
#[doc = " commits it describes rather than inferring them from the prose."]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextProvenance {
    pub repository: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub base_ref: String,
    pub base_oid: String,
    pub head_ref: String,
    pub head_oid: String,
    #[doc = " The commit the patch is measured from, which is the merge base"]
    #[doc = " rather than the base branch tip."]
    pub merge_base_oid: String,
    pub changed_files: usize,
    pub commits: usize,
    #[doc = " When the bundle was assembled."]
    pub generated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextBudget {
    pub characters: usize,
    pub used: usize,
    pub dropped_characters: usize,
    pub dropped_items: usize,
}

impl ContextBudget {
    pub(crate) const fn truncated(&self) -> bool {
        self.dropped_characters > 0 || self.dropped_items > 0
    }

    pub(crate) const fn remaining(&self) -> usize {
        self.characters.saturating_sub(self.used)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestContext {
    pub schema_version: u8,
    pub purpose: String,
    pub provenance: ContextProvenance,
    pub sections: Vec<ContextSection>,
    pub budget: ContextBudget,
    pub warnings: Vec<String>,
}

impl PullRequestContext {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    #[doc = " Attach what the assembly could not read. Kept apart from the"]
    #[doc = " sections so a caller can tell a bundle that is complete from one"]
    #[doc = " that is missing a part it asked for."]
    #[must_use]
    pub(crate) fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings.extend(warnings);
        self
    }

    pub(crate) fn section(&self, kind: ContextSectionKind) -> Option<&ContextSection> {
        self.sections.iter().find(|section| section.kind == kind)
    }
}
