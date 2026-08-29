use super::review::{pair_changes, parse_dependencies, parse_vulnerabilities};
use super::*;

fn record(fields: &[&str]) -> String {
    let mut line = fields.join("\t");
    line.push('\n');
    line
}

#[test]
fn a_removal_and_an_addition_of_one_package_become_a_single_upgrade() {
    let output = format!(
        "{}{}",
        record(&[
            "removed",
            "Cargo.lock",
            "cargo",
            "serde",
            "1.0.0",
            "MIT",
            "runtime",
            "0"
        ]),
        record(&[
            "added",
            "Cargo.lock",
            "cargo",
            "serde",
            "1.1.0",
            "Apache-2.0",
            "runtime",
            "0"
        ])
    );

    let changes = pair_changes(
        parse_dependencies(output.as_bytes()).expect("the fixture is a valid dependency record"),
    );

    assert_eq!(changes.len(), 1);
    let change = changes.first().expect("one change");
    assert_eq!(change.change, DependencyChange::Changed);
    assert_eq!(change.version_label(), "1.0.0 -> 1.1.0");
    assert!(change.license_changed());
    assert_eq!(change.previous_license, "MIT");
}

#[test]
fn a_package_removed_from_one_manifest_and_added_to_another_stays_two_rows() {
    let output = format!(
        "{}{}",
        record(&[
            "removed",
            "a/Cargo.lock",
            "cargo",
            "serde",
            "1.0.0",
            "MIT",
            "runtime",
            "0"
        ]),
        record(&[
            "added",
            "b/Cargo.lock",
            "cargo",
            "serde",
            "1.0.0",
            "MIT",
            "runtime",
            "0"
        ])
    );

    let changes = pair_changes(
        parse_dependencies(output.as_bytes()).expect("the fixture is a valid dependency record"),
    );

    assert_eq!(changes.len(), 2);
}

#[test]
fn the_counts_and_the_order_come_from_the_changes_themselves() {
    let output = format!(
        "{}{}{}",
        record(&[
            "added",
            "Cargo.lock",
            "cargo",
            "zzz",
            "1.0.0",
            "MIT",
            "development",
            "0"
        ]),
        record(&[
            "added",
            "Cargo.lock",
            "cargo",
            "aaa",
            "2.0.0",
            "MIT",
            "runtime",
            "1"
        ]),
        record(&[
            "removed",
            "Cargo.lock",
            "cargo",
            "old",
            "0.1.0",
            "MIT",
            "runtime",
            "0"
        ])
    );
    let mut listing = PullRequestDependencies {
        changes: pair_changes(parse_dependencies(output.as_bytes()).expect("the fixture parses")),
        ..PullRequestDependencies::default()
    };

    listing.finish();

    assert_eq!(listing.added, 2);
    assert_eq!(listing.removed, 1);
    assert_eq!(listing.changed, 0);
    assert_eq!(
        listing.schema_version,
        PullRequestDependencies::SCHEMA_VERSION
    );
    let names: Vec<&str> = listing
        .changes
        .iter()
        .map(|change| change.name.as_str())
        .collect();
    assert_eq!(names, ["aaa", "zzz", "old"]);
    let scopes: Vec<&str> = listing
        .changes
        .iter()
        .map(|change| change.scope.word())
        .collect();
    assert_eq!(scopes, ["runtime", "dev", "runtime"]);
}

#[test]
fn an_unknown_change_word_is_an_error_rather_than_a_silent_row() {
    let output = record(&[
        "vanished",
        "Cargo.lock",
        "cargo",
        "serde",
        "1.0.0",
        "MIT",
        "runtime",
        "0",
    ]);

    let error = parse_dependencies(output.as_bytes())
        .expect_err("an unknown change word cannot be reported as a dependency change");

    assert!(format!("{error:#}").contains("vanished"), "{error:#}");
}

#[test]
fn vulnerabilities_sort_by_severity_and_report_the_fixed_version() {
    let output = format!(
        "{}{}",
        record(&[
            "low-risk",
            "1.0.0",
            "low",
            "GHSA-1111",
            "A small problem",
            ""
        ]),
        record(&[
            "big-risk",
            "2.0.0",
            "critical",
            "GHSA-2222",
            "A large problem",
            "2.0.1"
        ])
    );
    let mut listing = PullRequestDependencies {
        vulnerabilities: parse_vulnerabilities(output.as_bytes())
            .expect("the fixture is a valid vulnerability record"),
        ..PullRequestDependencies::default()
    };

    listing.finish();

    let packages: Vec<&str> = listing
        .vulnerabilities
        .iter()
        .map(|vulnerability| vulnerability.package.as_str())
        .collect();
    assert_eq!(packages, ["big-risk", "low-risk"]);
    assert!(listing.has_serious_vulnerability());
    let first = listing.vulnerabilities.first().expect("one vulnerability");
    assert_eq!(first.severity, AdvisorySeverity::Critical);
    assert_eq!(first.first_patched_version, "2.0.1");
}

#[test]
fn a_severity_word_maps_to_the_level_it_names_and_never_panics() {
    assert_eq!(
        AdvisorySeverity::parse("CRITICAL"),
        AdvisorySeverity::Critical
    );
    assert_eq!(AdvisorySeverity::parse("error"), AdvisorySeverity::High);
    assert_eq!(
        AdvisorySeverity::parse("warning"),
        AdvisorySeverity::Moderate
    );
    assert_eq!(AdvisorySeverity::parse("note"), AdvisorySeverity::Low);
    assert_eq!(
        AdvisorySeverity::parse("gibberish"),
        AdvisorySeverity::Unknown
    );
    assert!(!AdvisorySeverity::Unknown.is_serious());
    assert!(AdvisorySeverity::High.is_serious());
}

#[test]
fn a_security_report_that_could_not_read_a_source_says_so_rather_than_reading_clean() {
    let mut security = PullRequestSecurity {
        warnings: vec!["code scanning was not readable: forbidden".to_owned()],
        ..PullRequestSecurity::default()
    };

    security.finish();

    assert!(!security.is_serious());
    assert_eq!(security.critical, 0);
    assert_eq!(
        security.warnings,
        ["code scanning was not readable: forbidden"]
    );
}

#[test]
fn security_counts_alerts_and_vulnerabilities_together_by_severity() {
    let mut security = PullRequestSecurity {
        alerts: vec![
            CodeScanningAlert {
                number: 2,
                rule: "rust/unused".to_owned(),
                severity: AdvisorySeverity::Low,
                description: "Unused import".to_owned(),
                path: "src/lib.rs".to_owned(),
                line: 4,
                url: String::new(),
            },
            CodeScanningAlert {
                number: 1,
                rule: "rust/injection".to_owned(),
                severity: AdvisorySeverity::Critical,
                description: "Command injection".to_owned(),
                path: "src/main.rs".to_owned(),
                line: 9,
                url: String::new(),
            },
        ],
        vulnerabilities: vec![DependencyVulnerability {
            package: "openssl".to_owned(),
            version: "0.1.0".to_owned(),
            severity: AdvisorySeverity::High,
            advisory: "GHSA-3333".to_owned(),
            summary: "Buffer overrun".to_owned(),
            first_patched_version: "0.1.1".to_owned(),
        }],
        ..PullRequestSecurity::default()
    };

    security.finish();

    assert_eq!(security.critical, 1);
    assert_eq!(security.high, 1);
    assert_eq!(security.other, 1);
    assert!(security.is_serious());
    let first = security.alerts.first().expect("one alert");
    assert_eq!(first.number, 1);
}

#[test]
fn a_comparison_wider_than_the_cap_is_cut_and_says_it_was() {
    let mut listing = PullRequestDependencies {
        changes: (0..MAX_DEPENDENCY_CHANGES + 5)
            .map(|index| DependencyDelta {
                change: DependencyChange::Added,
                ecosystem: "cargo".to_owned(),
                name: format!("package-{index:04}"),
                version: "1.0.0".to_owned(),
                previous_version: String::new(),
                manifest: "Cargo.lock".to_owned(),
                scope: DependencyScope::Runtime,
                license: "MIT".to_owned(),
                previous_license: String::new(),
                vulnerabilities: 0,
            })
            .collect(),
        ..PullRequestDependencies::default()
    };

    listing.finish();

    assert!(listing.truncated);
    assert_eq!(listing.changes.len(), MAX_DEPENDENCY_CHANGES);
    assert_eq!(listing.added, MAX_DEPENDENCY_CHANGES);
}
