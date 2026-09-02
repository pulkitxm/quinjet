#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DependencyChange {
    Added,
    Removed,
    #[doc = " The same package appears on both sides at different versions, which"]
    #[doc = " GitHub reports as a removal and an addition until Quinjet pairs them."]
    Changed,
}

impl DependencyChange {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Changed => "changed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DependencyScope {
    Runtime,
    Development,
    Unknown,
}

impl DependencyScope {
    pub(super) fn parse(scope: &str) -> Self {
        match scope.to_ascii_lowercase().as_str() {
            "runtime" => Self::Runtime,
            "development" => Self::Development,
            _ => Self::Unknown,
        }
    }

    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Development => "dev",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DependencyDelta {
    pub change: DependencyChange,
    pub ecosystem: String,
    pub name: String,
    #[doc = " The version this pull request introduces, empty for a removal."]
    pub version: String,
    #[doc = " The version it replaces, set only for a change."]
    pub previous_version: String,
    pub manifest: String,
    pub scope: DependencyScope,
    pub license: String,
    #[doc = " The license the previous version carried, set only when it differs."]
    pub previous_license: String,
    pub vulnerabilities: usize,
}

impl DependencyDelta {
    #[doc = " `1.2.3` for an addition, `1.2.3 -> 1.3.0` for a change."]
    pub(crate) fn version_label(&self) -> String {
        if self.previous_version.is_empty() || self.previous_version == self.version {
            return self.version.clone();
        }
        format!("{} -> {}", self.previous_version, self.version)
    }

    pub(crate) fn license_changed(&self) -> bool {
        !self.previous_license.is_empty() && self.previous_license != self.license
    }

    fn order(&self) -> (DependencyChange, String, String) {
        (self.change, self.ecosystem.clone(), self.name.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AdvisorySeverity {
    Critical,
    High,
    Moderate,
    Low,
    Unknown,
}

impl AdvisorySeverity {
    pub(super) fn parse(severity: &str) -> Self {
        match severity.to_ascii_lowercase().as_str() {
            "critical" => Self::Critical,
            "high" | "error" => Self::High,
            "moderate" | "medium" | "warning" => Self::Moderate,
            "low" | "note" => Self::Low,
            _ => Self::Unknown,
        }
    }

    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Moderate => "moderate",
            Self::Low => "low",
            Self::Unknown => "unknown",
        }
    }

    #[doc = " Whether a finding at this level is worth failing a script over."]
    pub(crate) const fn is_serious(self) -> bool {
        matches!(self, Self::Critical | Self::High)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DependencyVulnerability {
    pub package: String,
    pub version: String,
    pub severity: AdvisorySeverity,
    pub advisory: String,
    pub summary: String,
    #[doc = " The first version that fixes it, empty when there is no fix yet."]
    pub first_patched_version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestDependencies {
    pub schema_version: u8,
    pub base_oid: String,
    pub head_oid: String,
    pub changes: Vec<DependencyDelta>,
    pub vulnerabilities: Vec<DependencyVulnerability>,
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub license_changes: usize,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

impl PullRequestDependencies {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    pub(super) fn finish(&mut self) {
        self.changes.sort_by_key(DependencyDelta::order);
        self.vulnerabilities.sort_by(|left, right| {
            left.severity
                .cmp(&right.severity)
                .then_with(|| left.package.cmp(&right.package))
        });
        if self.changes.len() > MAX_DEPENDENCY_CHANGES {
            self.truncated = true;
            self.changes.truncate(MAX_DEPENDENCY_CHANGES);
        }
        self.added = self.count(DependencyChange::Added);
        self.removed = self.count(DependencyChange::Removed);
        self.changed = self.count(DependencyChange::Changed);
        self.license_changes = self
            .changes
            .iter()
            .filter(|change| change.license_changed())
            .count();
        self.schema_version = Self::SCHEMA_VERSION;
    }

    fn count(&self, wanted: DependencyChange) -> usize {
        self.changes
            .iter()
            .filter(|change| change.change == wanted)
            .count()
    }

    pub(crate) fn has_serious_vulnerability(&self) -> bool {
        self.vulnerabilities
            .iter()
            .any(|vulnerability| vulnerability.severity.is_serious())
    }
}

#[doc = " A code-scanning alert on the head commit."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodeScanningAlert {
    pub number: u64,
    pub rule: String,
    pub severity: AdvisorySeverity,
    pub description: String,
    pub path: String,
    pub line: usize,
    pub url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestSecurity {
    pub schema_version: u8,
    pub head_oid: String,
    pub alerts: Vec<CodeScanningAlert>,
    pub vulnerabilities: Vec<DependencyVulnerability>,
    pub critical: usize,
    pub high: usize,
    pub other: usize,
    pub truncated: bool,
    #[doc = " What Quinjet could not read. A repository with code scanning disabled"]
    #[doc = " or a token without the scope both land here rather than looking clean."]
    pub warnings: Vec<String>,
}

impl PullRequestSecurity {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    pub(super) fn finish(&mut self) {
        self.alerts.sort_by(|left, right| {
            left.severity
                .cmp(&right.severity)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.line.cmp(&right.line))
        });
        if self.alerts.len() > MAX_ALERTS {
            self.truncated = true;
            self.alerts.truncate(MAX_ALERTS);
        }
        self.vulnerabilities.sort_by(|left, right| {
            left.severity
                .cmp(&right.severity)
                .then_with(|| left.package.cmp(&right.package))
        });
        let severities = self.alerts.iter().map(|alert| alert.severity).chain(
            self.vulnerabilities
                .iter()
                .map(|vulnerability| vulnerability.severity),
        );
        self.critical = 0;
        self.high = 0;
        self.other = 0;
        for severity in severities {
            match severity {
                AdvisorySeverity::Critical => self.critical += 1,
                AdvisorySeverity::High => self.high += 1,
                _ => self.other += 1,
            }
        }
        self.schema_version = Self::SCHEMA_VERSION;
    }

    pub(crate) const fn is_serious(&self) -> bool {
        self.critical > 0 || self.high > 0
    }
}
