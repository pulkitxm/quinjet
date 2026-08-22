use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub(crate) const MAX_SSH_MACHINES: usize = 16;
pub(crate) const SWITCH_EXIT_BASE: u8 = 80;
pub(crate) const OPEN_PROJECTS_ENV: &str = "QUINJET_OPEN_PROJECTS";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SshMachine {
    pub target: String,
    pub folder: PathBuf,
    pub accessible: bool,
    pub uses: u64,
    pub local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SshContext {
    pub current: String,
    pub machines: Vec<SshMachine>,
}

impl SshContext {
    pub(crate) fn from_environment() -> Option<Self> {
        std::env::var("QUINJET_SSH_CONTEXT")
            .ok()
            .and_then(|value| serde_json::from_str(&value).ok())
    }
}

pub(crate) fn switch_exit_code(index: usize) -> Option<u8> {
    let Ok(index) = u8::try_from(index) else {
        return None;
    };
    (usize::from(index) < MAX_SSH_MACHINES).then_some(SWITCH_EXIT_BASE.saturating_add(index))
}

pub(crate) fn switch_index(code: i32) -> Option<usize> {
    let code = u8::try_from(code).ok()?;
    let index = code.checked_sub(SWITCH_EXIT_BASE)? as usize;
    (index < MAX_SSH_MACHINES).then_some(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_codes_cover_only_the_machine_limit() {
        assert_eq!(switch_exit_code(0), Some(80));
        assert_eq!(switch_exit_code(15), Some(95));
        assert_eq!(switch_exit_code(16), None);
        assert_eq!(switch_index(80), Some(0));
        assert_eq!(switch_index(95), Some(15));
        assert_eq!(switch_index(79), None);
        assert_eq!(switch_index(96), None);
    }
}
