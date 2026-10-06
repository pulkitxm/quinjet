use std::path::PathBuf;

use clap::ValueEnum;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::sinks::Bytes;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SearchMode {
    #[default]
    Name,
    Contents,
    Both,
}

impl SearchMode {
    pub(crate) const ALL: [Self; 3] = [Self::Name, Self::Contents, Self::Both];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Contents => "Contents",
            Self::Both => "Both",
        }
    }

    pub(crate) const fn cycle(self) -> Self {
        match self {
            Self::Name => Self::Contents,
            Self::Contents => Self::Both,
            Self::Both => Self::Name,
        }
    }

    pub(crate) const fn previous(self) -> Self {
        match self {
            Self::Name => Self::Both,
            Self::Contents => Self::Name,
            Self::Both => Self::Contents,
        }
    }

    pub(crate) const fn includes_name(self) -> bool {
        matches!(self, Self::Name | Self::Both)
    }

    pub(crate) const fn includes_contents(self) -> bool {
        matches!(self, Self::Contents | Self::Both)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SearchSource {
    Worktree,
    Index,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchFile {
    pub path: PathBuf,
    pub source: SearchSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SearchTarget {
    Changes {
        files: Vec<SearchFile>,
    },
    History {
        revision: String,
        skip: usize,
        limit: usize,
        selected: Option<String>,
    },
    PullRequest {
        workspace: u64,
        paths: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchRequest {
    pub query: String,
    pub mode: SearchMode,
    pub target: SearchTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchHits {
    pub mode: SearchMode,
    pub query: String,
    pub paths: Vec<String>,
    pub commits: Vec<String>,
}

impl SearchHits {
    pub(crate) fn empty(mode: SearchMode, query: impl Into<String>) -> Self {
        Self {
            mode,
            query: query.into(),
            paths: Vec::new(),
            commits: Vec::new(),
        }
    }

    pub(crate) fn with_paths(mut self, mut paths: Vec<String>) -> Self {
        paths.sort();
        paths.dedup();
        self.paths = paths;
        self
    }

    pub(crate) fn with_commits(mut self, mut commits: Vec<String>) -> Self {
        commits.sort();
        commits.dedup();
        self.commits = commits;
        self
    }
}

#[derive(Debug)]
pub(crate) struct PreparedSearch {
    normalized_query: String,
    contents: Option<PreparedContents>,
}

#[derive(Debug)]
struct PreparedContents {
    matcher: RegexMatcher,
    searcher: Searcher,
}

impl PreparedSearch {
    pub(crate) fn new(query: &str, mode: SearchMode) -> Self {
        let contents = if mode.includes_contents() && !query.is_empty() {
            matcher_for(query).map(|matcher| PreparedContents {
                matcher,
                searcher: SearcherBuilder::new()
                    .binary_detection(BinaryDetection::quit(0))
                    .build(),
            })
        } else {
            None
        };
        Self {
            normalized_query: query.to_lowercase(),
            contents,
        }
    }

    pub(crate) const fn normalized_query(&self) -> &str {
        self.normalized_query.as_str()
    }

    pub(crate) fn name_matches(&self, text: &str) -> bool {
        self.normalized_query.is_empty() || text.to_lowercase().contains(&self.normalized_query)
    }

    pub(crate) fn haystack_matches(&mut self, haystack: &[u8]) -> bool {
        if self.normalized_query.is_empty() {
            return true;
        }
        let Some(contents) = self.contents.as_mut() else {
            return false;
        };
        let mut found = false;
        drop(contents.searcher.search_slice(
            &contents.matcher,
            haystack,
            Bytes(|_, _| {
                found = true;
                Ok(false)
            }),
        ));
        found
    }
}

pub(crate) fn name_matches(query: &str, text: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    text.to_lowercase().contains(&query.to_lowercase())
}

fn matcher_for(query: &str) -> Option<RegexMatcher> {
    RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(query)
        .ok()
        .or_else(|| {
            RegexMatcherBuilder::new()
                .case_insensitive(true)
                .fixed_strings(true)
                .build(query)
                .ok()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_mode_stays_a_case_insensitive_substring() {
        assert!(name_matches("read", "README.md"));
        assert!(name_matches("READ", "src/readme.txt"));
        assert!(!name_matches("read", "src/main.rs"));
        assert!(name_matches("", "anything"));
    }

    #[test]
    fn contents_uses_ripgrep_regex_and_falls_back_to_literals() {
        let haystack_matches = |query: &str, haystack: &[u8]| {
            PreparedSearch::new(query, SearchMode::Contents).haystack_matches(haystack)
        };
        assert!(haystack_matches("foo", b"hello foo bar"));
        assert!(haystack_matches("FOO", b"hello foo bar"));
        assert!(haystack_matches("f.o", b"fao"));
        assert!(!haystack_matches("zzz", b"hello foo"));
        assert!(haystack_matches("(", b"value (ok)"));
        assert!(haystack_matches("", b"anything"));
        assert!(haystack_matches("école", "ÉCOLE".as_bytes()));
        assert!(haystack_matches("^foo$", b"before\nFOO\nafter\n"));
        assert!(!haystack_matches("^foo$", b"before-FOO-after\n"));
    }

    #[test]
    fn prepared_search_resets_between_documents() {
        let mut early_match_utf16 = vec![0xff, 0xfe];
        early_match_utf16.extend("needle\n".encode_utf16().flat_map(u16::to_le_bytes));
        early_match_utf16.resize(512 * 1024, b' ');
        let documents: &[(&[u8], bool)] = &[
            (b"", false),
            (b"no match\n", false),
            (b"before\nNEEDLE\nafter\n", true),
            (b"", false),
            (b"\xef\xbb\xbfNEEDLE\n", true),
            (b"\xff\xfen\0e\0e\0d\0l\0e\0\n\0", true),
            (b"needle\0needle\n", false),
            (b"no match\n", false),
            (b"\xfe\xff\0N\0E\0E\0D\0L\0E\0\n", true),
            (b"\0needle\n", false),
            (&early_match_utf16, true),
            (b"\xff\xfen\0o\0\n\0", false),
            (b"no match\n", false),
            (b"needle\n", true),
            (b"", false),
        ];
        for query in ["needle", "^needle$"] {
            let mut search = PreparedSearch::new(query, SearchMode::Contents);
            for _ in 0..2 {
                for (index, &(document, expected)) in documents.iter().enumerate() {
                    let fresh =
                        PreparedSearch::new(query, SearchMode::Contents).haystack_matches(document);
                    let reused = search.haystack_matches(document);
                    assert_eq!(reused, fresh, "{query:?}: document {index}");
                    assert_eq!(reused, expected, "{query:?}: document {index}");
                }
            }
        }
    }

    #[test]
    fn name_only_and_empty_queries_skip_contents_preparation() {
        let search = PreparedSearch::new("(READ", SearchMode::Name);
        assert!(search.contents.is_none(), "{search:?}");
        assert!(search.name_matches("src/(read).txt"));
        assert!(!search.name_matches("README.md"));
        for mode in SearchMode::ALL {
            let mut search = PreparedSearch::new("", mode);
            assert!(search.contents.is_none(), "{search:?}");
            for document in [b"".as_slice(), b"\0binary", b"\xff\xfe"] {
                assert!(search.haystack_matches(document), "{mode:?}: {document:?}");
            }
        }
    }

    #[test]
    fn search_mode_cycles_through_the_three_labels() {
        assert_eq!(SearchMode::Name.cycle(), SearchMode::Contents);
        assert_eq!(SearchMode::Contents.cycle(), SearchMode::Both);
        assert_eq!(SearchMode::Both.cycle(), SearchMode::Name);
        assert_eq!(SearchMode::Name.previous(), SearchMode::Both);
        assert_eq!(SearchMode::Contents.previous(), SearchMode::Name);
        assert_eq!(SearchMode::Both.previous(), SearchMode::Contents);
        assert_eq!(
            SearchMode::ALL.map(SearchMode::label),
            ["Name", "Contents", "Both"]
        );
    }
}
