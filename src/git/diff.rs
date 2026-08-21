use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use unicode_width::UnicodeWidthChar;

use crate::theme::SyntaxColor;

const TAB_WIDTH: usize = 4;
const MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES: usize = 512 * 1024;
const MAX_SYNTAX_HIGHLIGHT_LINE_BYTES: usize = 32 * 1024;

mod model;
mod parser;
mod syntax;

pub(crate) use model::*;
pub(crate) use parser::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use syntax::*;

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests;
