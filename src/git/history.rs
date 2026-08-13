#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub parent_ids: Vec<String>,
    pub author: String,
    pub author_email: String,
    pub authored_at: String,
    pub relative_date: String,
    pub subject: String,
    pub decorations: Vec<String>,
}

/// The format passed to `git log`. Unit and record separators avoid ambiguity with
/// spaces, tabs, and most text that can occur in names or commit subjects.
pub const LOG_FORMAT: &str = "%H%x1f%h%x1f%P%x1f%aN%x1f%aE%x1f%aI%x1f%ar%x1f%s%x1f%D%x1e";

pub fn parse_log(output: &[u8]) -> Vec<Commit> {
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
    if fields.len() < 9 {
        return None;
    }

    Some(Commit {
        id: text(fields[0]),
        short_id: text(fields[1]),
        parent_ids: text(fields[2])
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect(),
        author: text(fields[3]),
        author_email: text(fields[4]),
        authored_at: text(fields[5]),
        relative_date: text(fields[6]),
        subject: text(fields[7]),
        decorations: text(fields[8])
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect(),
    })
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
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
        let output = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\x1faaaaaaa\x1fbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb cccccccccccccccccccccccccccccccccccccccc\x1fAda Lovelace\x1fada@example.com\x1f2026-01-02T03:04:05Z\x1f2 hours ago\x1fMerge a fast thing\x1fHEAD -> main, origin/main, tag: v1\x1e\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\x1fbbbbbbb\x1f\x1fGrace Hopper\x1fgrace@example.com\x1f2026-01-01T00:00:00Z\x1fyesterday\x1fInitial commit\x1f\x1e";

        let commits = parse_log(output);

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].author, "Ada Lovelace");
        assert_eq!(commits[0].parent_ids.len(), 2);
        assert_eq!(
            commits[0].decorations,
            vec!["HEAD -> main", "origin/main", "tag: v1"]
        );
        assert_eq!(commits[1].subject, "Initial commit");
        assert!(commits[1].parent_ids.is_empty());
    }
}
