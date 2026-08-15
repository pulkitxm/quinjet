#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Commit {
    pub id: String,
    pub short_id: String,
    pub parent_ids: Vec<String>,
    pub author: String,
    pub author_email: String,
    pub authored_at: String,
    pub committer: String,
    pub committer_email: String,
    pub committed_at: String,
    pub relative_date: String,
    pub subject: String,
    pub decorations: Vec<String>,
}

/// The format passed to `git log`. Unit and record separators avoid ambiguity with
/// spaces, tabs, and most text that can occur in names or commit subjects.
pub(crate) const LOG_FORMAT: &str =
    "%H%x1f%h%x1f%P%x1f%aN%x1f%aE%x1f%aI%x1f%cN%x1f%cE%x1f%cI%x1f%ar%x1f%s%x1f%D%x1e";

pub(crate) fn parse_log(output: &[u8]) -> Vec<Commit> {
    output
        .split(|byte| *byte == 0x1e)
        .filter_map(parse_record)
        .collect()
}

fn parse_record(record: &[u8]) -> Option<Commit> {
    let record = trim_ascii(record);
    if record.is_empty() {
        return None;
    }
    let fields: Vec<&[u8]> = record.split(|byte| *byte == 0x1f).collect();
    let [
        id,
        short_id,
        parent_ids,
        author,
        author_email,
        authored_at,
        committer,
        committer_email,
        committed_at,
        relative_date,
        subject,
        decorations,
        ..,
    ] = fields.as_slice()
    else {
        return None;
    };

    Some(Commit {
        id: text(id),
        short_id: text(short_id),
        parent_ids: text(parent_ids)
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect(),
        author: text(author),
        author_email: text(author_email),
        authored_at: text(authored_at),
        committer: text(committer),
        committer_email: text(committer_email),
        committed_at: text(committed_at),
        relative_date: text(relative_date),
        subject: text(subject),
        decorations: text(decorations)
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect(),
    })
}

const fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while let Some((first, rest)) = value.split_first() {
        if !first.is_ascii_whitespace() {
            break;
        }
        value = rest;
    }
    while let Some((last, rest)) = value.split_last() {
        if !last.is_ascii_whitespace() {
            break;
        }
        value = rest;
    }
    value
}

fn text(value: &[u8]) -> String {
    String::from_utf8_lossy(value).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_log_records_and_decorations() {
        let output = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\x1faaaaaaa\x1fbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb cccccccccccccccccccccccccccccccccccccccc\x1fAda Lovelace\x1fada@example.com\x1f2026-01-02T03:04:05Z\x1fLinus Torvalds\x1flinus@example.com\x1f2026-01-02T04:05:06Z\x1f2 hours ago\x1fMerge a fast thing\x1fHEAD -> main, origin/main, tag: v1\x1e\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\x1fbbbbbbb\x1f\x1fGrace Hopper\x1fgrace@example.com\x1f2026-01-01T00:00:00Z\x1fGrace Hopper\x1fgrace@example.com\x1f2026-01-01T00:00:01Z\x1fyesterday\x1fInitial commit\x1f\x1e";

        let commits = parse_log(output);

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].author, "Ada Lovelace");
        assert_eq!(commits[0].committer, "Linus Torvalds");
        assert_eq!(commits[0].committed_at, "2026-01-02T04:05:06Z");
        assert_eq!(commits[0].parent_ids.len(), 2);
        assert_eq!(
            commits[0].decorations,
            vec!["HEAD -> main", "origin/main", "tag: v1"]
        );
        assert_eq!(commits[1].subject, "Initial commit");
        assert!(commits[1].parent_ids.is_empty());
    }
}
