#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) fn annotations(
    listing: &PullRequestAnnotations,
    group: AnnotationGrouping,
    full: bool,
) -> String {
    if listing.annotations.is_empty() {
        let mut out = Report::default();
        out.line("No annotations reported");
        for warning in &listing.warnings {
            out.line(&format!("note  {warning}"));
        }
        return out.finish();
    }
    let mut out = Report::default();
    for (heading, rows) in listing.grouped(group) {
        out.line(&format!("\n{heading}"));
        for annotation in rows {
            out.line(&annotation_row(annotation, group));
            if full {
                for line in detail_lines(annotation) {
                    out.line(&format!("      {line}"));
                }
            }
        }
    }
    out.line(&format!("\n{}", annotation_summary(&listing.counts)));
    if listing.truncated {
        out.line("[the annotation list reached Quinjet's size cap]");
    }
    for warning in &listing.warnings {
        out.line(&format!("note  {warning}"));
    }
    out.finish()
}

#[doc = " A row never repeats what its grouping heading already said."]
fn annotation_row(annotation: &CheckAnnotation, group: AnnotationGrouping) -> String {
    let severity = if group == AnnotationGrouping::Severity {
        String::new()
    } else {
        format!("{:<9}", annotation.severity.word())
    };
    let position = if group == AnnotationGrouping::File {
        format!("{:<8}", line_label(annotation))
    } else {
        format!("{:<32}", truncate(&annotation.location(), 32))
    };
    let mut row = format!(
        "  {}  {severity}{position} {}",
        annotation.severity.glyph(),
        truncate(&annotation.headline(), 60)
    );
    if !annotation.placement.is_in_diff() {
        row.push_str("  [");
        row.push_str(annotation.placement.word());
        row.push(']');
    }
    if group != AnnotationGrouping::Check {
        row.push_str("  (");
        row.push_str(&annotation.check);
        row.push(')');
    }
    row
}

fn line_label(annotation: &CheckAnnotation) -> String {
    if annotation.start_line == 0 {
        return "file".to_owned();
    }
    if annotation.end_line > annotation.start_line {
        return format!("{}-{}", annotation.start_line, annotation.end_line);
    }
    format!("{}", annotation.start_line)
}

fn detail_lines(annotation: &CheckAnnotation) -> Vec<String> {
    let mut lines: Vec<String> = annotation
        .message
        .lines()
        .map(str::to_owned)
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines.extend(
        annotation
            .raw_details
            .lines()
            .map(str::to_owned)
            .filter(|line| !line.trim().is_empty()),
    );
    lines
}

pub(crate) fn annotation_summary(counts: &AnnotationCounts) -> String {
    format!(
        "{} failure, {} warning, {} notice · {} on changed lines, {} elsewhere",
        counts.failure, counts.warning, counts.notice, counts.in_diff, counts.outside_diff
    )
}
