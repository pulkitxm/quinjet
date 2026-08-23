use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct TabId(u64);

impl TabId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TabInfo {
    pub(crate) id: TabId,
    pub(crate) title: String,
    pub(crate) root: PathBuf,
    pub(crate) machine: Option<String>,
    pub(crate) active: bool,
}

#[derive(Debug)]
struct RepositoryTab<T> {
    id: TabId,
    title: String,
    root: PathBuf,
    pending: bool,
    value: T,
}

#[derive(Debug)]
pub(crate) struct RepositoryTabs<T> {
    tabs: Vec<RepositoryTab<T>>,
    active: Option<TabId>,
    next_id: u64,
}

impl<T> RepositoryTabs<T> {
    #[cfg(test)]
    pub(crate) fn new(title: impl Into<String>, root: impl Into<PathBuf>, value: T) -> Self {
        Self::new_with_id(TabId::new(0), title, root, value)
    }

    pub(crate) fn new_with_id(
        id: TabId,
        title: impl Into<String>,
        root: impl Into<PathBuf>,
        value: T,
    ) -> Self {
        Self::new_with_pending(id, title, root, value, false)
    }

    pub(crate) fn new_pending_with_id(
        id: TabId,
        title: impl Into<String>,
        root: impl Into<PathBuf>,
        value: T,
    ) -> Self {
        Self::new_with_pending(id, title, root, value, true)
    }

    fn new_with_pending(
        id: TabId,
        title: impl Into<String>,
        root: impl Into<PathBuf>,
        value: T,
        pending: bool,
    ) -> Self {
        Self {
            tabs: vec![RepositoryTab {
                id,
                title: title.into(),
                root: root.into(),
                pending,
                value,
            }],
            active: Some(id),
            next_id: id.value().wrapping_add(1),
        }
    }

    pub(crate) const fn active_id(&self) -> Option<TabId> {
        self.active
    }

    pub(crate) fn active(&self) -> Option<&T> {
        let active = self.active?;
        self.tabs
            .iter()
            .find(|tab| tab.id == active)
            .map(|tab| &tab.value)
    }

    pub(crate) fn active_mut(&mut self) -> Option<&mut T> {
        let active = self.active?;
        self.tabs
            .iter_mut()
            .find(|tab| tab.id == active)
            .map(|tab| &mut tab.value)
    }

    pub(crate) fn get(&self, id: TabId) -> Option<&T> {
        self.tabs
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| &tab.value)
    }

    pub(crate) fn get_mut(&mut self, id: TabId) -> Option<&mut T> {
        self.tabs
            .iter_mut()
            .find(|tab| tab.id == id)
            .map(|tab| &mut tab.value)
    }

    #[cfg(test)]
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = (TabId, &T)> + ExactSizeIterator {
        self.tabs.iter().map(|tab| (tab.id, &tab.value))
    }

    pub(crate) fn iter_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = (TabId, &mut T)> + ExactSizeIterator {
        self.tabs.iter_mut().map(|tab| (tab.id, &mut tab.value))
    }

    #[cfg(test)]
    pub(crate) const fn len(&self) -> usize {
        self.tabs.len()
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub(crate) fn append(
        &mut self,
        title: impl Into<String>,
        root: impl Into<PathBuf>,
        value: T,
    ) -> TabId {
        let id = TabId(self.next_id);
        self.append_with_id(id, title, root, value)
    }

    pub(crate) fn append_with_id(
        &mut self,
        id: TabId,
        title: impl Into<String>,
        root: impl Into<PathBuf>,
        value: T,
    ) -> TabId {
        self.next_id = self.next_id.max(id.value().wrapping_add(1));
        self.tabs.push(RepositoryTab {
            id,
            title: title.into(),
            root: root.into(),
            pending: false,
            value,
        });
        self.active = Some(id);
        id
    }

    pub(crate) fn append_pending_with_id(
        &mut self,
        id: TabId,
        title: impl Into<String>,
        root: impl Into<PathBuf>,
        value: T,
    ) -> TabId {
        self.next_id = self.next_id.max(id.value().wrapping_add(1));
        self.tabs.push(RepositoryTab {
            id,
            title: title.into(),
            root: root.into(),
            pending: true,
            value,
        });
        self.active = Some(id);
        id
    }

    pub(crate) fn append_pending(
        &mut self,
        title: impl Into<String>,
        root: impl Into<PathBuf>,
        value: T,
    ) -> TabId {
        let id = TabId(self.next_id);
        self.append_pending_with_id(id, title, root, value)
    }

    pub(crate) fn is_pending(&self, id: TabId) -> bool {
        self.tabs
            .iter()
            .find(|tab| tab.id == id)
            .is_some_and(|tab| tab.pending)
    }

    pub(crate) fn id_for_root(&self, root: &Path) -> Option<TabId> {
        self.tabs
            .iter()
            .find(|tab| tab.root == root)
            .map(|tab| tab.id)
    }

    pub(crate) fn activate(&mut self, id: TabId) -> bool {
        if self.tabs.iter().any(|tab| tab.id == id) {
            self.active = Some(id);
            true
        } else {
            false
        }
    }

    pub(crate) fn replace(
        &mut self,
        id: TabId,
        title: impl Into<String>,
        root: impl Into<PathBuf>,
        value: T,
    ) -> Option<T> {
        let tab = self.tabs.iter_mut().find(|tab| tab.id == id)?;
        tab.title = title.into();
        tab.root = root.into();
        tab.pending = false;
        Some(std::mem::replace(&mut tab.value, value))
    }

    #[cfg(test)]
    pub(crate) fn cycle_next(&mut self) -> Option<TabId> {
        let next = self.active.map_or_else(
            || self.tabs.first(),
            |active| {
                self.tabs
                    .iter()
                    .skip_while(|tab| tab.id != active)
                    .nth(1)
                    .or_else(|| self.tabs.first())
            },
        )?;
        self.active = Some(next.id);
        Some(next.id)
    }

    #[cfg(test)]
    pub(crate) fn cycle_previous(&mut self) -> Option<TabId> {
        let previous = self.active.map_or_else(
            || self.tabs.last(),
            |active| {
                self.tabs
                    .iter()
                    .position(|tab| tab.id == active)
                    .and_then(|index| index.checked_sub(1))
                    .and_then(|index| self.tabs.get(index))
                    .or_else(|| self.tabs.last())
            },
        )?;
        self.active = Some(previous.id);
        Some(previous.id)
    }

    pub(crate) fn reorder(&mut self, source: TabId, target: TabId) -> bool {
        let Some(source_index) = self.tabs.iter().position(|tab| tab.id == source) else {
            return false;
        };
        let Some(target_index) = self.tabs.iter().position(|tab| tab.id == target) else {
            return false;
        };
        if source_index == target_index {
            return true;
        }
        let tab = self.tabs.remove(source_index);
        self.tabs.insert(target_index, tab);
        true
    }

    pub(crate) fn close(&mut self, id: TabId) -> Option<T> {
        let index = self.tabs.iter().position(|tab| tab.id == id)?;
        let was_active = self.active == Some(id);
        let removed = self.tabs.remove(index);
        if was_active {
            self.active = self
                .tabs
                .get(index)
                .or_else(|| self.tabs.last())
                .map(|tab| tab.id);
        }
        Some(removed.value)
    }

    pub(crate) fn close_others(&mut self, id: TabId) -> Vec<T> {
        if !self.tabs.iter().any(|tab| tab.id == id) {
            return Vec::new();
        }
        let mut retained = Vec::with_capacity(1);
        let mut removed = Vec::with_capacity(self.tabs.len().saturating_sub(1));
        for tab in self.tabs.drain(..) {
            if tab.id == id {
                retained.push(tab);
            } else {
                removed.push(tab.value);
            }
        }
        self.tabs = retained;
        self.active = Some(id);
        removed
    }

    pub(crate) fn close_all(&mut self) -> Vec<T> {
        self.active = None;
        std::mem::take(&mut self.tabs)
            .into_iter()
            .map(|tab| tab.value)
            .collect()
    }

    pub(crate) fn infos(&self) -> Vec<TabInfo> {
        self.tabs
            .iter()
            .map(|tab| TabInfo {
                id: tab.id,
                title: tab.title.clone(),
                root: tab.root.clone(),
                machine: None,
                active: self.active == Some(tab.id),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
