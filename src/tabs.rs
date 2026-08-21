use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TabId(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TabInfo {
    pub(crate) id: TabId,
    pub(crate) title: String,
    pub(crate) root: PathBuf,
    pub(crate) active: bool,
}

#[derive(Debug)]
struct RepositoryTab<T> {
    id: TabId,
    title: String,
    root: PathBuf,
    value: T,
}

#[derive(Debug)]
pub(crate) struct RepositoryTabs<T> {
    tabs: Vec<RepositoryTab<T>>,
    active: Option<TabId>,
    next_id: u64,
}

impl<T> RepositoryTabs<T> {
    pub(crate) fn new(title: impl Into<String>, root: impl Into<PathBuf>, value: T) -> Self {
        let id = TabId(0);
        Self {
            tabs: vec![RepositoryTab {
                id,
                title: title.into(),
                root: root.into(),
                value,
            }],
            active: Some(id),
            next_id: 1,
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
        self.next_id = self.next_id.wrapping_add(1);
        self.tabs.push(RepositoryTab {
            id,
            title: title.into(),
            root: root.into(),
            value,
        });
        self.active = Some(id);
        id
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
                active: self.active == Some(tab.id),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{RepositoryTabs, TabInfo};

    #[test]
    fn new_collection_has_one_active_tab() {
        let tabs = RepositoryTabs::new("one", "/one", 10);
        let id = tabs.active_id().expect("initial tab is active");

        assert_eq!(tabs.len(), 1);
        assert!(!tabs.is_empty());
        assert_eq!(tabs.active(), Some(&10));
        assert_eq!(tabs.iter().collect::<Vec<_>>(), vec![(id, &10)]);
        assert_eq!(
            tabs.infos(),
            vec![TabInfo {
                id,
                title: String::from("one"),
                root: PathBuf::from("/one"),
                active: true,
            }]
        );
    }

    #[test]
    fn append_activates_a_stable_new_id() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let first = tabs.active_id().expect("initial tab is active");
        let second = tabs.append("two", "/two", 20);

        assert_ne!(first, second);
        assert_eq!(tabs.active_id(), Some(second));
        assert_eq!(tabs.active(), Some(&20));
        assert_eq!(tabs.id_for_root(Path::new("/one")), Some(first));
        assert_eq!(tabs.id_for_root(Path::new("/two")), Some(second));
        assert_eq!(tabs.id_for_root(Path::new("/tw")), None);
    }

    #[test]
    fn activate_rejects_an_id_that_was_closed() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let first = tabs.active_id().expect("initial tab is active");
        let second = tabs.append("two", "/two", 20);
        assert_eq!(tabs.close(first), Some(10));

        assert!(!tabs.activate(first));
        assert_eq!(tabs.active_id(), Some(second));
        assert!(tabs.activate(second));
    }

    #[test]
    fn cycling_wraps_in_both_directions() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let first = tabs.active_id().expect("initial tab is active");
        let second = tabs.append("two", "/two", 20);
        let third = tabs.append("three", "/three", 30);

        assert_eq!(tabs.cycle_next(), Some(first));
        assert_eq!(tabs.cycle_next(), Some(second));
        assert_eq!(tabs.cycle_previous(), Some(first));
        assert_eq!(tabs.cycle_previous(), Some(third));
    }

    #[test]
    fn reorder_moves_to_the_target_position_and_keeps_active_identity() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let first = tabs.active_id().expect("initial tab is active");
        let second = tabs.append("two", "/two", 20);
        let third = tabs.append("three", "/three", 30);
        assert!(tabs.activate(second));

        assert!(tabs.reorder(first, third));
        assert_eq!(
            tabs.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            vec![second, third, first]
        );
        assert_eq!(tabs.active_id(), Some(second));
        assert!(tabs.reorder(first, second));
        assert_eq!(
            tabs.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            vec![first, second, third]
        );
        assert_eq!(tabs.active_id(), Some(second));
    }

    #[test]
    fn reorder_rejects_missing_ids_without_changing_order() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let removed = tabs.append("two", "/two", 20);
        let first = tabs
            .id_for_root(Path::new("/one"))
            .expect("first tab exists");
        assert_eq!(tabs.close(removed), Some(20));

        assert!(!tabs.reorder(removed, first));
        assert!(!tabs.reorder(first, removed));
        assert_eq!(
            tabs.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            vec![first]
        );
    }

    #[test]
    fn closing_active_tab_prefers_its_right_neighbor() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let first = tabs.active_id().expect("initial tab is active");
        let second = tabs.append("two", "/two", 20);
        let third = tabs.append("three", "/three", 30);
        assert!(tabs.activate(second));

        assert_eq!(tabs.close(second), Some(20));
        assert_eq!(tabs.active_id(), Some(third));
        assert_eq!(tabs.close(third), Some(30));
        assert_eq!(tabs.active_id(), Some(first));
    }

    #[test]
    fn closing_inactive_and_final_tabs_has_deterministic_state() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let first = tabs.active_id().expect("initial tab is active");
        let second = tabs.append("two", "/two", 20);

        assert_eq!(tabs.close(first), Some(10));
        assert_eq!(tabs.active_id(), Some(second));
        assert_eq!(tabs.close(second), Some(20));
        assert!(tabs.is_empty());
        assert_eq!(tabs.active_id(), None);
        assert_eq!(tabs.active(), None);
        assert_eq!(tabs.cycle_next(), None);
        assert_eq!(tabs.cycle_previous(), None);
    }

    #[test]
    fn close_others_keeps_and_activates_the_target() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let first = tabs.active_id().expect("initial tab is active");
        let second = tabs.append("two", "/two", 20);
        let third = tabs.append("three", "/three", 30);

        assert_eq!(tabs.close_others(second), vec![10, 30]);
        assert_eq!(tabs.active_id(), Some(second));
        assert_eq!(tabs.active(), Some(&20));
        assert_eq!(
            tabs.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            vec![second]
        );
        assert_eq!(tabs.close_others(first), Vec::<i32>::new());
        assert_eq!(tabs.active_id(), Some(second));
        assert_ne!(second, third);
    }

    #[test]
    fn close_all_can_be_followed_by_append() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let first = tabs.active_id().expect("initial tab is active");
        let second = tabs.append("two", "/two", 20);

        assert_eq!(tabs.close_all(), vec![10, 20]);
        assert!(tabs.is_empty());
        assert_eq!(tabs.active_id(), None);
        let third = tabs.append("three", "/three", 30);
        assert_ne!(third, first);
        assert_ne!(third, second);
        assert_eq!(tabs.active_id(), Some(third));
    }

    #[test]
    fn mutable_access_keeps_values_independent() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let first = tabs.active_id().expect("initial tab is active");
        let second = tabs.append("two", "/two", 20);
        for (_, value) in tabs.iter_mut() {
            *value += 1;
        }
        assert!(tabs.activate(first));
        let active = tabs.active_mut().expect("first tab is active");
        *active += 100;

        assert_eq!(tabs.active(), Some(&111));
        assert_eq!(tabs.get(first), Some(&111));
        assert_eq!(tabs.get(second), Some(&21));
        assert_eq!(tabs.get_mut(first).map(|value| *value), Some(111));
        assert!(tabs.activate(second));
        assert_eq!(tabs.active(), Some(&21));
    }

    #[test]
    fn replace_preserves_tab_identity_position_and_selection() {
        let mut tabs = RepositoryTabs::new("one", "/one", 10);
        let first = tabs.active_id().expect("initial tab is active");
        let second = tabs.append("two", "/two", 20);
        assert!(tabs.activate(first));

        assert_eq!(tabs.replace(first, "next", "/next", 30), Some(10));
        assert_eq!(tabs.active_id(), Some(first));
        assert_eq!(tabs.active(), Some(&30));
        assert_eq!(tabs.id_for_root(Path::new("/one")), None);
        assert_eq!(tabs.id_for_root(Path::new("/next")), Some(first));
        assert_eq!(
            tabs.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            vec![first, second]
        );
    }
}
