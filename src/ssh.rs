use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub(crate) const MAX_SSH_MACHINES: usize = 16;
pub(crate) const SWITCH_EXIT_BASE: u8 = 80;
pub(crate) const SWITCH_NEW_TAB_EXIT_BASE: u8 = 96;
pub(crate) const OPEN_PROJECTS_ENV: &str = "QUINJET_OPEN_PROJECTS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SshProjectOpenMode {
    CurrentTab,
    NewTab,
}

impl SshProjectOpenMode {
    pub(crate) const fn environment_value(self) -> &'static str {
        match self {
            Self::CurrentTab => "current-tab",
            Self::NewTab => "new-tab",
        }
    }

    pub(crate) fn from_environment() -> Option<Self> {
        match std::env::var(OPEN_PROJECTS_ENV).ok()?.as_str() {
            "current-tab" => Some(Self::CurrentTab),
            "new-tab" => Some(Self::NewTab),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SshSwitch {
    pub index: usize,
    pub mode: SshProjectOpenMode,
}

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

pub(crate) fn switch_exit_code(request: SshSwitch) -> Option<u8> {
    let Ok(index) = u8::try_from(request.index) else {
        return None;
    };
    let base = match request.mode {
        SshProjectOpenMode::CurrentTab => SWITCH_EXIT_BASE,
        SshProjectOpenMode::NewTab => SWITCH_NEW_TAB_EXIT_BASE,
    };
    (usize::from(index) < MAX_SSH_MACHINES).then_some(base.saturating_add(index))
}

pub(crate) fn switch_request(code: i32) -> Option<SshSwitch> {
    let code = u8::try_from(code).ok()?;
    let (base, mode) = if code >= SWITCH_NEW_TAB_EXIT_BASE {
        (SWITCH_NEW_TAB_EXIT_BASE, SshProjectOpenMode::NewTab)
    } else {
        (SWITCH_EXIT_BASE, SshProjectOpenMode::CurrentTab)
    };
    let index = code.checked_sub(base)? as usize;
    (index < MAX_SSH_MACHINES).then_some(SshSwitch { index, mode })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_codes_cover_only_the_machine_limit() {
        let current = |index| SshSwitch {
            index,
            mode: SshProjectOpenMode::CurrentTab,
        };
        let new_tab = |index| SshSwitch {
            index,
            mode: SshProjectOpenMode::NewTab,
        };
        assert_eq!(switch_exit_code(current(0)), Some(80));
        assert_eq!(switch_exit_code(current(15)), Some(95));
        assert_eq!(switch_exit_code(new_tab(0)), Some(96));
        assert_eq!(switch_exit_code(new_tab(15)), Some(111));
        assert_eq!(switch_exit_code(current(16)), None);
        assert_eq!(switch_request(80), Some(current(0)));
        assert_eq!(switch_request(95), Some(current(15)));
        assert_eq!(switch_request(96), Some(new_tab(0)));
        assert_eq!(switch_request(111), Some(new_tab(15)));
        assert_eq!(switch_request(79), None);
        assert_eq!(switch_request(112), None);
    }
}
