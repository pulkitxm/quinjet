use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::tabs::{TabId, TabInfo};

pub(crate) const MAX_SSH_MACHINES: usize = 16;
pub(crate) const SWITCH_EXIT_BASE: u8 = 80;
pub(crate) const SWITCH_NEW_TAB_EXIT_BASE: u8 = 96;
pub(crate) const SWITCH_TAB_EXIT_BASE: u8 = 112;
pub(crate) const OPEN_PROJECTS_ENV: &str = "QUINJET_OPEN_PROJECTS";
pub(crate) const HANDOFF_CONTEXT_PREFIX: &[u8] = b"\x1b]777;quinjet-context=";
pub(crate) const HANDOFF_CONTEXT_SUFFIX: u8 = 0x07;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SshProjectOpenMode {
    Current,
    New,
    Activate,
}

impl SshProjectOpenMode {
    pub(crate) const fn environment_value(self) -> &'static str {
        match self {
            Self::Current => "current-tab",
            Self::New => "new-tab",
            Self::Activate => "activate-tab",
        }
    }

    pub(crate) fn from_environment() -> Option<Self> {
        match std::env::var(OPEN_PROJECTS_ENV).ok()?.as_str() {
            "current-tab" => Some(Self::Current),
            "new-tab" => Some(Self::New),
            "activate-tab" => Some(Self::Activate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SshSwitch {
    pub index: usize,
    pub mode: SshProjectOpenMode,
}

pub(crate) fn next_accessible_machine_index(
    machines: &[SshMachine],
    selected: usize,
) -> Option<usize> {
    machines
        .iter()
        .enumerate()
        .skip(selected.saturating_add(1))
        .chain(machines.iter().enumerate().take(selected))
        .find(|(_, machine)| machine.accessible)
        .map(|(index, _)| index)
}

pub(crate) fn previous_accessible_machine_index(
    machines: &[SshMachine],
    selected: usize,
) -> Option<usize> {
    machines
        .iter()
        .enumerate()
        .take(selected)
        .rev()
        .chain(
            machines
                .iter()
                .enumerate()
                .skip(selected.saturating_add(1))
                .rev(),
        )
        .find(|(_, machine)| machine.accessible)
        .map(|(index, _)| index)
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
pub(crate) struct SshTab {
    pub(crate) id: TabId,
    pub(crate) machine: String,
    pub(crate) title: String,
    pub(crate) root: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SshTabs {
    entries: Vec<SshTab>,
    active: Option<TabId>,
    active_by_machine: BTreeMap<String, TabId>,
    next_id: u64,
}

impl SshTabs {
    pub(crate) const fn active_id(&self) -> Option<TabId> {
        self.active
    }

    pub(crate) fn infos(&self) -> Vec<TabInfo> {
        self.entries
            .iter()
            .map(|tab| TabInfo {
                id: tab.id,
                title: tab.title.clone(),
                root: tab.root.clone(),
                machine: Some(tab.machine.clone()),
                active: self.active == Some(tab.id),
            })
            .collect()
    }

    pub(crate) fn entries_for_machine(&self, machine: &str) -> impl Iterator<Item = &SshTab> {
        self.entries
            .iter()
            .filter(move |tab| tab.machine == machine)
    }

    pub(crate) fn get(&self, id: TabId) -> Option<&SshTab> {
        self.entries.iter().find(|tab| tab.id == id)
    }

    pub(crate) fn id_for_root(&self, machine: &str, root: &Path) -> Option<TabId> {
        self.entries
            .iter()
            .find(|tab| tab.machine == machine && crate::git::support::same_path(&tab.root, root))
            .map(|tab| tab.id)
    }

    pub(crate) fn active_for_machine(&self, machine: &str) -> Option<TabId> {
        self.active_by_machine
            .get(machine)
            .copied()
            .filter(|id| self.get(*id).is_some_and(|tab| tab.machine == machine))
    }

    pub(crate) fn append(
        &mut self,
        machine: impl Into<String>,
        title: impl Into<String>,
        root: impl Into<PathBuf>,
    ) -> TabId {
        let id = TabId::new(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.entries.push(SshTab {
            id,
            machine: machine.into(),
            title: title.into(),
            root: root.into(),
        });
        drop(self.activate(id));
        id
    }

    pub(crate) fn replace(
        &mut self,
        id: TabId,
        title: impl Into<String>,
        root: impl Into<PathBuf>,
    ) -> bool {
        let Some(tab) = self.entries.iter_mut().find(|tab| tab.id == id) else {
            return false;
        };
        tab.title = title.into();
        tab.root = root.into();
        true
    }

    pub(crate) fn activate(&mut self, id: TabId) -> Option<String> {
        let machine = self.get(id)?.machine.clone();
        self.active = Some(id);
        let _previous = self.active_by_machine.insert(machine.clone(), id);
        Some(machine)
    }

    pub(crate) fn reorder(&mut self, source: TabId, target: TabId) -> bool {
        let Some(source_index) = self.entries.iter().position(|tab| tab.id == source) else {
            return false;
        };
        let Some(target_index) = self.entries.iter().position(|tab| tab.id == target) else {
            return false;
        };
        if source_index == target_index {
            return true;
        }
        let tab = self.entries.remove(source_index);
        self.entries.insert(target_index, tab);
        true
    }

    pub(crate) fn close(&mut self, id: TabId) -> Option<SshTab> {
        let index = self.entries.iter().position(|tab| tab.id == id)?;
        let removed = self.entries.remove(index);
        self.active_by_machine.retain(|_, active| *active != id);
        if self.active == Some(id) {
            self.active = self
                .entries
                .get(index)
                .or_else(|| self.entries.last())
                .map(|tab| tab.id);
        }
        if let Some(active) = self.active {
            drop(self.activate(active));
        }
        let next_for_machine = self
            .entries_for_machine(&removed.machine)
            .next()
            .map(|tab| tab.id);
        if self.active_for_machine(&removed.machine).is_none()
            && let Some(next) = next_for_machine
        {
            let _previous = self.active_by_machine.insert(removed.machine.clone(), next);
        }
        Some(removed)
    }

    pub(crate) fn close_others(&mut self, id: TabId) -> bool {
        let Some(tab) = self.get(id).cloned() else {
            return false;
        };
        self.entries.retain(|candidate| candidate.id == id);
        self.active = Some(id);
        self.active_by_machine.clear();
        let _previous = self.active_by_machine.insert(tab.machine, id);
        true
    }

    pub(crate) fn close_all(&mut self) {
        self.entries.clear();
        self.active = None;
        self.active_by_machine.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SshContext {
    pub current: String,
    pub machines: Vec<SshMachine>,
    pub tabs: SshTabs,
}

impl SshContext {
    pub(crate) fn from_environment() -> Option<Self> {
        std::env::var("QUINJET_SSH_CONTEXT")
            .ok()
            .and_then(|value| serde_json::from_str(&value).ok())
    }

    pub(crate) fn adjacent_accessible_machine_index(&self, reverse: bool) -> Option<usize> {
        let current = self
            .machines
            .iter()
            .position(|machine| machine.target == self.current)?;
        if reverse {
            previous_accessible_machine_index(&self.machines, current)
        } else {
            next_accessible_machine_index(&self.machines, current)
        }
    }
}

pub(crate) fn emit_handoff_context(context: &SshContext) -> Result<()> {
    let payload = serde_json::to_vec(context).context("failed to serialize terminal handoff")?;
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(HANDOFF_CONTEXT_PREFIX)
        .and_then(|()| stdout.write_all(&payload))
        .and_then(|()| stdout.write_all(&[HANDOFF_CONTEXT_SUFFIX]))
        .and_then(|()| stdout.flush())
        .context("failed to send terminal handoff")
}

pub(crate) fn switch_exit_code(request: SshSwitch) -> Option<u8> {
    let Ok(index) = u8::try_from(request.index) else {
        return None;
    };
    let base = match request.mode {
        SshProjectOpenMode::Current => SWITCH_EXIT_BASE,
        SshProjectOpenMode::New => SWITCH_NEW_TAB_EXIT_BASE,
        SshProjectOpenMode::Activate => SWITCH_TAB_EXIT_BASE,
    };
    (usize::from(index) < MAX_SSH_MACHINES).then_some(base.saturating_add(index))
}

pub(crate) fn switch_request(code: i32) -> Option<SshSwitch> {
    let code = u8::try_from(code).ok()?;
    let (base, mode) = if code >= SWITCH_TAB_EXIT_BASE {
        (SWITCH_TAB_EXIT_BASE, SshProjectOpenMode::Activate)
    } else if code >= SWITCH_NEW_TAB_EXIT_BASE {
        (SWITCH_NEW_TAB_EXIT_BASE, SshProjectOpenMode::New)
    } else {
        (SWITCH_EXIT_BASE, SshProjectOpenMode::Current)
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
            mode: SshProjectOpenMode::Current,
        };
        let new_tab = |index| SshSwitch {
            index,
            mode: SshProjectOpenMode::New,
        };
        let activate_tab = |index| SshSwitch {
            index,
            mode: SshProjectOpenMode::Activate,
        };
        assert_eq!(switch_exit_code(current(0)), Some(80));
        assert_eq!(switch_exit_code(current(15)), Some(95));
        assert_eq!(switch_exit_code(new_tab(0)), Some(96));
        assert_eq!(switch_exit_code(new_tab(15)), Some(111));
        assert_eq!(switch_exit_code(activate_tab(0)), Some(112));
        assert_eq!(switch_exit_code(activate_tab(15)), Some(127));
        assert_eq!(switch_exit_code(current(16)), None);
        assert_eq!(switch_request(80), Some(current(0)));
        assert_eq!(switch_request(95), Some(current(15)));
        assert_eq!(switch_request(96), Some(new_tab(0)));
        assert_eq!(switch_request(111), Some(new_tab(15)));
        assert_eq!(switch_request(112), Some(activate_tab(0)));
        assert_eq!(switch_request(127), Some(activate_tab(15)));
        assert_eq!(switch_request(79), None);
        assert_eq!(switch_request(128), None);
    }

    #[test]
    fn shared_tabs_keep_global_order_and_per_machine_selection() {
        let mut tabs = SshTabs::default();
        let local_one = tabs.append("macbook", "one", "/local/one");
        let remote = tabs.append("tof", "remote", "/remote/repo");
        let local_two = tabs.append("macbook", "two", "/local/two");

        assert_eq!(tabs.activate(local_one).as_deref(), Some("macbook"));
        assert_eq!(tabs.active_for_machine("macbook"), Some(local_one));
        assert_eq!(tabs.active_for_machine("tof"), Some(remote));
        assert!(tabs.reorder(local_two, remote));
        assert_eq!(
            tabs.infos()
                .into_iter()
                .map(|tab| (tab.id, tab.machine))
                .collect::<Vec<_>>(),
            vec![
                (local_one, Some("macbook".to_owned())),
                (local_two, Some("macbook".to_owned())),
                (remote, Some("tof".to_owned())),
            ]
        );
        assert_eq!(tabs.close(local_one).map(|tab| tab.id), Some(local_one));
        assert_eq!(tabs.active_id(), Some(local_two));
        assert_eq!(tabs.active_for_machine("macbook"), Some(local_two));
    }
}
