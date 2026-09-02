#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " The paths worth loading a patch for: the annotated ones the pull request"]
#[doc = " actually changes. Loading the rest would be a request for nothing, since"]
#[doc = " an annotation on an untouched file can never sit in a hunk."]
pub(crate) fn annotated_paths(
    annotations: &PullRequestAnnotations,
    index: &PullRequestDiffIndex,
) -> Vec<PathBuf> {
    let changed: BTreeSet<&Path> = index.files.iter().map(|file| file.path.as_path()).collect();
    let mut wanted: BTreeSet<PathBuf> = BTreeSet::new();
    for annotation in &annotations.annotations {
        if changed.contains(annotation.path.as_path()) {
            let _ = wanted.insert(annotation.path.clone());
        }
    }
    wanted.into_iter().collect()
}

#[doc = " Decide, for each annotation, whether the pull request's patch shows the"]
#[doc = " line it points at. `visible` maps a changed path to the new-side line"]
#[doc = " numbers its patch renders; a path missing from it was never loaded."]
pub(crate) fn mark_diff_coverage(
    annotations: &mut PullRequestAnnotations,
    index: &PullRequestDiffIndex,
    visible: &HashMap<PathBuf, BTreeSet<usize>>,
) {
    let changed: BTreeSet<&Path> = index.files.iter().map(|file| file.path.as_path()).collect();
    for annotation in &mut annotations.annotations {
        annotation.placement = placement(annotation, &changed, visible);
    }
    annotations.finish();
}

fn placement(
    annotation: &CheckAnnotation,
    changed: &BTreeSet<&Path>,
    visible: &HashMap<PathBuf, BTreeSet<usize>>,
) -> AnnotationPlacement {
    if !changed.contains(annotation.path.as_path()) {
        return AnnotationPlacement::OutsideDiff;
    }
    let Some(lines) = visible.get(&annotation.path) else {
        return AnnotationPlacement::Unknown;
    };
    if annotation.start_line == 0 {
        return AnnotationPlacement::InDiff;
    }
    if (annotation.start_line..=annotation.end_line).any(|line| lines.contains(&line)) {
        AnnotationPlacement::InDiff
    } else {
        AnnotationPlacement::OutsideHunks
    }
}

#[doc = " The new-side line numbers a rendered patch actually shows, which is what"]
#[doc = " decides whether an annotation can be drawn on it."]
pub(crate) fn visible_lines(document: &crate::git::diff::DiffDocument) -> BTreeSet<usize> {
    document
        .lines
        .iter()
        .filter_map(|line| line.new_line)
        .collect()
}
