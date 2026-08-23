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
            machine: None,
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
