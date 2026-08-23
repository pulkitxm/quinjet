use std::time::Instant;

use super::{RepositoryWorkspace, RoutedEffects};

impl RepositoryWorkspace {
    pub(crate) fn drain_worker_events(&mut self, now: Instant) -> Vec<RoutedEffects> {
        let mut routed = Vec::new();
        for (id, runtime) in self.tabs.iter_mut() {
            let events = runtime.worker.events().try_iter().collect::<Vec<_>>();
            for event in events {
                let effects = runtime.app.handle_worker_event(event, now);
                routed.push(RoutedEffects { id, effects });
            }
        }
        routed
    }

    pub(crate) fn poll_watchers(&mut self) -> Vec<RoutedEffects> {
        let mut routed = Vec::new();
        for (id, runtime) in self.tabs.iter_mut() {
            let Some(receiver) = runtime
                .watcher
                .as_ref()
                .map(crate::watch::RepoWatcher::changes)
            else {
                continue;
            };
            if receiver.try_iter().next().is_none() {
                continue;
            }
            let mut effects = Vec::new();
            runtime.app.filesystem_changed(&mut effects);
            routed.push(RoutedEffects { id, effects });
        }
        routed
    }

    pub(crate) fn tick(&mut self, now: Instant) -> (Vec<RoutedEffects>, bool) {
        let active = self.active_id();
        let mut dirty = false;
        let mut routed = Vec::new();
        for (id, runtime) in self.tabs.iter_mut() {
            let (effects, changed) = runtime.app.tick(now);
            dirty |= changed && active == Some(id);
            routed.push(RoutedEffects { id, effects });
        }
        (routed, dirty)
    }

    pub(crate) fn periodic_refresh(&mut self) -> Vec<RoutedEffects> {
        let mut routed = Vec::new();
        for (id, runtime) in self.tabs.iter_mut() {
            let mut effects = Vec::new();
            runtime.app.periodic_refresh(&mut effects);
            routed.push(RoutedEffects { id, effects });
        }
        routed
    }

    pub(crate) fn webhook_delivered(&mut self, now: Instant) -> Vec<RoutedEffects> {
        self.tabs
            .iter_mut()
            .map(|(id, runtime)| RoutedEffects {
                id,
                effects: runtime.app.webhook_delivered(now),
            })
            .collect()
    }
}
