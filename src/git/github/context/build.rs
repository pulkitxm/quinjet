#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Everything the bundle can draw on. Fetching happens in the caller, so"]
#[doc = " the assembly is pure and the same inputs always produce the same"]
#[doc = " bundle: a coding tool that reruns it gets the same context."]
pub(crate) struct ContextInputs<'a> {
    pub pull_request: &'a PullRequest,
    pub purpose: ContextPurpose,
    pub budget: usize,
    pub merge_base_oid: &'a str,
    pub index: &'a PullRequestDiffIndex,
    pub patch: &'a str,
    pub review: Option<&'a PullRequestReviewSnapshot>,
    pub gate: Option<&'a MergeGate>,
    pub annotations: Option<&'a PullRequestAnnotations>,
    pub dependencies: Option<&'a PullRequestDependencies>,
    pub commits: Option<&'a PullRequestCommits>,
    #[doc = " Repository instruction files, as (path, contents). These are the one"]
    #[doc = " trusted section, because they are committed to the repository."]
    pub instructions: &'a [(PathBuf, String)],
    pub generated_at: String,
    pub warnings: Vec<String>,
}

#[doc = " Assemble a bundle inside a character budget, filling sections in the"]
#[doc = " order the purpose asks for so that what the caller came for survives a"]
#[doc = " small budget and what it did not is what gets dropped."]
pub(crate) fn build_context(inputs: &ContextInputs<'_>) -> PullRequestContext {
    let budget = inputs.budget.max(MIN_BUDGET);
    let mut context = PullRequestContext {
        schema_version: PullRequestContext::SCHEMA_VERSION,
        purpose: inputs.purpose.word().to_owned(),
        provenance: provenance(inputs),
        sections: Vec::new(),
        budget: ContextBudget {
            characters: budget,
            ..ContextBudget::default()
        },
        warnings: inputs.warnings.clone(),
    };
    for kind in inputs.purpose.section_order() {
        let Some((body, dropped_items)) = section_body(inputs, kind) else {
            continue;
        };
        if body.trim().is_empty() {
            continue;
        }
        let remaining = context.budget.remaining();
        let (body, dropped_characters) = fit(body, remaining);
        if body.is_empty() {
            context.budget.dropped_items += dropped_items.max(1);
            context.budget.dropped_characters += dropped_characters;
            continue;
        }
        context.budget.used += body.chars().count();
        context.budget.dropped_characters += dropped_characters;
        context.budget.dropped_items += dropped_items;
        context.sections.push(ContextSection {
            kind,
            heading: kind.heading().to_owned(),
            body,
            untrusted: kind.is_untrusted(),
            dropped_characters,
            dropped_items,
        });
    }
    context
}

fn provenance(inputs: &ContextInputs<'_>) -> ContextProvenance {
    let pull_request = inputs.pull_request;
    ContextProvenance {
        repository: pull_request.base_repository.name_with_owner.clone(),
        number: pull_request.number,
        title: pull_request.title.clone(),
        url: pull_request.url.clone(),
        base_ref: pull_request.base_ref.clone(),
        base_oid: pull_request.base_oid.clone(),
        head_ref: pull_request.head_ref.clone(),
        head_oid: pull_request.head_oid.clone(),
        merge_base_oid: inputs.merge_base_oid.to_owned(),
        changed_files: inputs.index.files.len(),
        commits: inputs
            .commits
            .map(|commits| commits.commits.len())
            .unwrap_or_default(),
        generated_at: inputs.generated_at.clone(),
    }
}

fn section_body(inputs: &ContextInputs<'_>, kind: ContextSectionKind) -> Option<(String, usize)> {
    match kind {
        ContextSectionKind::Instructions => Some((instructions(inputs.instructions), 0)),
        ContextSectionKind::Patch => Some((inputs.patch.to_owned(), 0)),
        ContextSectionKind::Threads => inputs.review.map(threads),
        ContextSectionKind::Checks => Some(checks(inputs)),
        ContextSectionKind::Dependencies => inputs.dependencies.map(dependencies),
    }
}

fn instructions(files: &[(PathBuf, String)]) -> String {
    let mut body = String::new();
    for (path, contents) in files {
        body.push_str("--- ");
        body.push_str(&path.display().to_string());
        body.push_str(" ---\n");
        body.push_str(contents.trim_end());
        body.push_str("\n\n");
    }
    body
}

fn threads(review: &PullRequestReviewSnapshot) -> (String, usize) {
    let mut body = String::new();
    let mut count = 0;
    for thread in review.threads.iter().filter(|thread| !thread.is_resolved) {
        count += 1;
        body.push_str(&thread.path.display().to_string());
        if let Some(line) = thread.line.or(thread.original_line) {
            body.push(':');
            body.push_str(&line.to_string());
        }
        if thread.is_outdated {
            body.push_str("  [outdated]");
        }
        body.push_str("  id ");
        body.push_str(&thread.id);
        body.push('\n');
        for comment in &thread.comments {
            body.push_str("  @");
            body.push_str(&comment.author);
            body.push_str(": ");
            body.push_str(comment.body.trim());
            body.push('\n');
        }
        body.push('\n');
    }
    (body, count)
}

fn checks(inputs: &ContextInputs<'_>) -> (String, usize) {
    let mut body = String::new();
    let mut count = 0;
    if let Some(gate) = inputs.gate {
        for check in gate.checks.failing() {
            count += 1;
            body.push_str(&check.display_name());
            body.push_str(" failed");
            if check.required {
                body.push_str(" (required)");
            }
            body.push('\n');
        }
    }
    if let Some(annotations) = inputs.annotations {
        for annotation in &annotations.annotations {
            count += 1;
            body.push_str(&annotation_line(annotation));
        }
    }
    (body, count)
}

fn annotation_line(annotation: &CheckAnnotation) -> String {
    let mut line = format!(
        "{}  {}  {}",
        annotation.severity.word(),
        annotation.location(),
        annotation.headline()
    );
    if !annotation.message.trim().is_empty() && annotation.message.trim() != annotation.headline() {
        line.push_str("\n    ");
        line.push_str(annotation.message.trim());
    }
    line.push('\n');
    line
}

fn dependencies(listing: &PullRequestDependencies) -> (String, usize) {
    let mut body = String::new();
    let mut count = 0;
    for change in &listing.changes {
        count += 1;
        body.push_str(change.change.word());
        body.push(' ');
        body.push_str(&change.ecosystem);
        body.push(':');
        body.push_str(&change.name);
        body.push(' ');
        body.push_str(&change.version_label());
        if change.license_changed() {
            body.push_str("  license ");
            body.push_str(&change.previous_license);
            body.push_str(" -> ");
            body.push_str(&change.license);
        }
        body.push('\n');
    }
    for vulnerability in &listing.vulnerabilities {
        count += 1;
        body.push_str(vulnerability.severity.word());
        body.push(' ');
        body.push_str(&vulnerability.package);
        body.push(' ');
        body.push_str(&vulnerability.version);
        body.push_str("  ");
        body.push_str(&vulnerability.summary);
        if !vulnerability.first_patched_version.is_empty() {
            body.push_str("  fixed in ");
            body.push_str(&vulnerability.first_patched_version);
        }
        body.push('\n');
    }
    (body, count)
}

#[doc = " Cut a section to the space left, at a line boundary so a patch never"]
#[doc = " ends mid-hunk, and report exactly how much was dropped."]
fn fit(body: String, remaining: usize) -> (String, usize) {
    let length = body.chars().count();
    if length <= remaining {
        return (body, 0);
    }
    if remaining < MIN_SECTION_CHARACTERS {
        return (String::new(), length);
    }
    let mut kept = String::new();
    for line in body.split_inclusive('\n') {
        if kept.chars().count() + line.chars().count() > remaining {
            break;
        }
        kept.push_str(line);
    }
    let dropped = length - kept.chars().count();
    (kept, dropped)
}
