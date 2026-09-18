use std::path::PathBuf;

use clap::ValueEnum;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::Bytes;
use grep_searcher::{BinaryDetection, SearcherBuilder};
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

pub(crate) fn name_matches(query: &str, text: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    text.to_lowercase().contains(&query.to_lowercase())
}

pub(crate) fn haystack_matches(query: &str, haystack: &[u8]) -> bool {
    if query.is_empty() {
        return true;
    }
    let Some(matcher) = matcher_for(query) else {
        return false;
    };
    let mut found = false;
    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(0))
        .build();
    drop(searcher.search_slice(
        &matcher,
        haystack,
        Bytes(|_, _| {
            found = true;
            Ok(false)
        }),
    ));
    found
}

fn matcher_for(query: &str) -> Option<grep_regex::RegexMatcher> {
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
        assert!(haystack_matches("foo", b"hello foo bar"));
        assert!(haystack_matches("FOO", b"hello foo bar"));
        assert!(haystack_matches("f.o", b"fao"));
        assert!(!haystack_matches("zzz", b"hello foo"));
        assert!(haystack_matches("(", b"value (ok)"));
        assert!(haystack_matches("", b"anything"));
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
