#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) fn dependencies(listing: &PullRequestDependencies) -> String {
    let mut out = Report::default();
    for change in &listing.changes {
        out.line(&dependency_row(change));
    }
    if out.empty() {
        out.line("No dependency changes reported");
    } else {
        out.line(&format!(
            "\n{} added, {} removed, {} changed, {} license change(s)",
            listing.added, listing.removed, listing.changed, listing.license_changes
        ));
    }
    if !listing.vulnerabilities.is_empty() {
        out.blank();
        for vulnerability in &listing.vulnerabilities {
            out.line(&vulnerability_row(vulnerability));
        }
        if listing.has_serious_vulnerability() {
            out.line("a dependency this pull request introduces has a known serious advisory");
        }
    }
    if listing.truncated {
        out.line("[the dependency comparison reached Quinjet's size cap]");
    }
    for warning in &listing.warnings {
        out.line(&format!("note  {warning}"));
    }
    out.finish()
}

fn dependency_row(change: &DependencyDelta) -> String {
    let mut row = format!(
        "{:<8} {:<9} {:<34} {}",
        change.change.word(),
        change.scope.word(),
        truncate(&format!("{}:{}", change.ecosystem, change.name), 34),
        change.version_label()
    );
    if change.license_changed() {
        row.push_str("  license ");
        row.push_str(&change.previous_license);
        row.push_str(" -> ");
        row.push_str(&change.license);
    }
    row
}

fn vulnerability_row(vulnerability: &DependencyVulnerability) -> String {
    let mut row = format!(
        "{:<9} {:<34} {}",
        vulnerability.severity.word(),
        truncate(
            &format!("{} {}", vulnerability.package, vulnerability.version),
            34
        ),
        truncate(&vulnerability.summary, 58)
    );
    if !vulnerability.first_patched_version.is_empty() {
        row.push_str("  fixed in ");
        row.push_str(&vulnerability.first_patched_version);
    }
    row
}

pub(crate) fn security(findings: &PullRequestSecurity) -> String {
    let mut out = Report::default();
    for alert in &findings.alerts {
        out.line(&format!(
            "{:<9} {:<34} {}",
            alert.severity.word(),
            truncate(&format!("{}:{}", alert.path, alert.line), 34),
            truncate(&alert_summary(alert), 58)
        ));
    }
    for vulnerability in &findings.vulnerabilities {
        out.line(&vulnerability_row(vulnerability));
    }
    if out.empty() {
        out.line("No security findings reported");
    } else {
        out.line(&format!(
            "\n{} critical, {} high, {} other",
            findings.critical, findings.high, findings.other
        ));
    }
    if findings.truncated {
        out.line("[the alert list reached Quinjet's size cap]");
    }
    for warning in &findings.warnings {
        out.line(&format!("note  {warning}"));
    }
    out.finish()
}

fn alert_summary(alert: &CodeScanningAlert) -> String {
    if alert.description.trim().is_empty() {
        return alert.rule.clone();
    }
    alert.description.trim().to_owned()
}

#[doc = " The text face of a bundle. Every section carries a banner saying"]
#[doc = " whether its body is repository content or text written by whoever can"]
#[doc = " comment on the pull request, because a reader deciding what to trust"]
#[doc = " must not have to infer that from the prose itself."]
pub(crate) fn context(bundle: &PullRequestContext) -> String {
    let mut out = Report::default();
    out.line(&format!(
        "context for {}#{} ({})",
        bundle.provenance.repository, bundle.provenance.number, bundle.purpose
    ));
    out.line(&format!(
        "head {}  base {}  merge-base {}",
        short_oid(&bundle.provenance.head_oid),
        short_oid(&bundle.provenance.base_oid),
        short_oid(&bundle.provenance.merge_base_oid)
    ));
    out.line(&format!(
        "{} file(s), {} commit(s), assembled {}",
        bundle.provenance.changed_files, bundle.provenance.commits, bundle.provenance.generated_at
    ));
    for section in &bundle.sections {
        out.blank();
        out.line(&section_banner(section));
        out.line(section.body.trim_end());
        if section.is_truncated() {
            out.line(&format!(
                "[{} character(s) and {} item(s) left out of this section]",
                section.dropped_characters, section.dropped_items
            ));
        }
    }
    out.blank();
    out.line(&format!(
        "{} of {} character(s) used{}",
        bundle.budget.used,
        bundle.budget.characters,
        if bundle.budget.truncated() {
            ", truncated"
        } else {
            ""
        }
    ));
    for warning in &bundle.warnings {
        out.line(&format!("note  {warning}"));
    }
    out.finish()
}

fn section_banner(section: &ContextSection) -> String {
    let trust = if section.untrusted {
        "untrusted, written by pull-request participants"
    } else {
        "trusted, committed to the repository"
    };
    format!("=== {} ({trust}) ===", section.heading)
}
