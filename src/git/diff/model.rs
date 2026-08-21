#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DiffLineKind {
    FileHeader,
    FileFooter,
    HunkHeader,
    Context,
    Added,
    Removed,
    Meta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HighlightSpan {
    pub text: String,
    pub foreground: Option<SyntaxColor>,
    pub bold: bool,
    pub italic: bool,
}

impl HighlightSpan {
    pub(super) fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            foreground: None,
            bold: false,
            italic: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub spans: Vec<HighlightSpan>,
}

impl DiffLine {
    pub(crate) fn text(&self) -> String {
        if self.kind == DiffLineKind::FileHeader {
            self.spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            self.spans.iter().map(|span| span.text.as_str()).collect()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommitDetails {
    pub id: String,
    pub subject: String,
    pub author: String,
    pub author_email: String,
    pub authored_at: String,
    pub committer: String,
    pub committer_email: String,
    pub committed_at: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffLineCounts {
    pub additions: usize,
    pub deletions: usize,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffFileIndexEntry {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: String,
    /// Exact per-file totals read from `git diff --numstat` while the index is
    /// built. Known counts let a file header render its real `+n -n` before the
    /// patch for that file has been produced.
    pub counts: Option<DiffLineCounts>,
}

impl DiffFileIndexEntry {
    pub(crate) const fn new(path: PathBuf, old_path: Option<PathBuf>, status: String) -> Self {
        Self {
            path,
            old_path,
            status,
            counts: None,
        }
    }

    fn label(&self) -> String {
        let mut label = self.path.display().to_string();
        if let Some(old_path) = self.old_path.as_ref().filter(|old| *old != &self.path) {
            label.push_str("  · renamed from ");
            label.push_str(&old_path.display().to_string());
        } else if !self.status.is_empty() {
            label.push_str("  · ");
            label.push_str(&self.status);
        }
        if self.counts.is_some_and(|counts| counts.binary) {
            label.push_str("  · binary");
        }
        label
    }

    fn count_spans(&self) -> (String, String) {
        self.counts.map_or_else(
            || ("+··".to_owned(), "-··".to_owned()),
            |counts| {
                (
                    format!("+{}", counts.additions),
                    format!("-{}", counts.deletions),
                )
            },
        )
    }
}

/// Parse `git diff --numstat -z` output into per-path totals. Renames emit an
/// empty path field followed by the pre-image and post-image records, so the
/// scanner has to consume those two extra records instead of assuming one.
pub(crate) fn parse_numstat(output: &[u8]) -> HashMap<PathBuf, DiffLineCounts> {
    let records: Vec<&[u8]> = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect();
    let mut counts = HashMap::new();
    let mut cursor = 0;
    while cursor < records.len() {
        let Some(record) = records.get(cursor).copied() else {
            break;
        };
        cursor += 1;
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let (Some(additions), Some(deletions), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let binary = additions == b"-" || deletions == b"-";
        let entry = DiffLineCounts {
            additions: parse_count(additions),
            deletions: parse_count(deletions),
            binary,
        };
        if path.is_empty() {
            let Some(new_path) = records.get(cursor + 1) else {
                break;
            };
            cursor += 2;
            let _ = counts.insert(record_path(new_path), entry);
        } else {
            let _ = counts.insert(record_path(path), entry);
        }
    }
    counts
}

fn parse_count(field: &[u8]) -> usize {
    std::str::from_utf8(field)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default()
}

fn record_path(record: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(record).into_owned())
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffIndex {
    pub title: String,
    pub files: Vec<DiffFileIndexEntry>,
    pub truncated: bool,
    pub commit_details: Option<CommitDetails>,
}

impl DiffIndex {
    pub(crate) fn line_counts(&self) -> DiffLineCounts {
        self.files.iter().filter_map(|file| file.counts).fold(
            DiffLineCounts::default(),
            |total, counts| DiffLineCounts {
                additions: total.additions.saturating_add(counts.additions),
                deletions: total.deletions.saturating_add(counts.deletions),
                binary: total.binary || counts.binary,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn document(&self, loaded: &HashMap<PathBuf, DiffDocument>) -> DiffDocument {
        self.document_with_visibility(loaded, |_| true)
    }

    pub(crate) fn document_with_visibility(
        &self,
        loaded: &HashMap<PathBuf, DiffDocument>,
        mut visible: impl FnMut(&Path) -> bool,
    ) -> DiffDocument {
        if self.files.is_empty() {
            let mut document = DiffDocument::empty(&self.title, "No file changes to display");
            document.commit_details.clone_from(&self.commit_details);
            return document;
        }

        let mut lines = Vec::with_capacity(self.files.len().saturating_mul(3));
        let mut truncated = self.truncated;
        for file in &self.files {
            let loaded_document = loaded.get(&file.path);
            let show_body = visible(&file.path);
            truncated |= loaded_document.is_some_and(|document| document.truncated);
            let loaded_header = loaded_document.and_then(|document| {
                document
                    .lines
                    .iter()
                    .find(|line| line.kind == DiffLineKind::FileHeader)
            });
            if show_body && let Some(document) = loaded_document.filter(|_| loaded_header.is_some())
            {
                let mut file_lines = document.lines.clone();
                if let Some(label) = file_lines
                    .iter_mut()
                    .find(|line| line.kind == DiffLineKind::FileHeader)
                    .and_then(|header| header.spans.first_mut())
                {
                    label.text = file.label();
                }
                lines.extend(file_lines);
                continue;
            }

            let mut header = index_file_header(file);
            if let Some(loaded_header) = loaded_header {
                for span_index in 1..=2 {
                    if let (Some(target), Some(source)) = (
                        header.spans.get_mut(span_index),
                        loaded_header.spans.get(span_index),
                    ) {
                        target.text.clone_from(&source.text);
                    }
                }
            }
            lines.push(header);
            if show_body {
                if let Some(document) = loaded_document {
                    lines.extend(document.lines.clone());
                } else {
                    lines.push(meta_line(DiffLineKind::Meta, "Loading diff…"));
                }
            } else {
                lines.push(meta_line(
                    DiffLineKind::Meta,
                    if loaded_document.is_some() {
                        "Diff loaded · expand this file to display it"
                    } else {
                        "Expand this file to load its diff"
                    },
                ));
            }
            lines.push(meta_line(DiffLineKind::FileFooter, ""));
        }

        DiffDocument {
            title: self.title.clone(),
            lines,
            truncated,
            commit_details: self.commit_details.clone(),
            pull_request_details: None,
        }
    }
}

fn index_file_header(file: &DiffFileIndexEntry) -> DiffLine {
    let (additions, deletions) = file.count_spans();
    DiffLine {
        kind: DiffLineKind::FileHeader,
        old_line: None,
        new_line: None,
        spans: vec![
            HighlightSpan::plain(file.label()),
            HighlightSpan::plain(additions),
            HighlightSpan::plain(deletions),
        ],
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestDetails {
    pub number: u64,
    pub title: String,
    pub description: String,
    pub author: String,
    pub state: String,
    pub is_draft: bool,
    pub updated_at: String,
    pub url: String,
    pub base_repository: String,
    pub base_ref: String,
    pub base_remotes: Vec<String>,
    pub head_repository: Option<String>,
    pub head_ref: String,
    pub head_remotes: Vec<String>,
    pub is_cross_repository: bool,
    pub changed_files: usize,
    pub additions: usize,
    pub deletions: usize,
    pub selected_file: Option<String>,
    pub selected_file_additions: usize,
    pub selected_file_deletions: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffDocument {
    pub title: String,
    pub lines: Vec<DiffLine>,
    pub truncated: bool,
    pub commit_details: Option<CommitDetails>,
    pub pull_request_details: Option<PullRequestDetails>,
}

impl DiffDocument {
    pub(crate) fn empty(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            lines: vec![DiffLine {
                kind: DiffLineKind::Meta,
                old_line: None,
                new_line: None,
                spans: vec![HighlightSpan::plain(message)],
            }],
            truncated: false,
            commit_details: None,
            pull_request_details: None,
        }
    }

    pub(crate) fn file_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::FileHeader)
            .count()
    }

    pub(crate) fn addition_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::Added)
            .count()
    }

    pub(crate) fn deletion_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::Removed)
            .count()
    }
}
