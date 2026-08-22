use std::path::Path;
use std::time::Instant;

use crate::app::{App, AppEffect, ToastLevel};
use crate::git::Repository;
use crate::git::worker::{GitWorker, WorkerCommand};
use crate::integration::Client;
use crate::tabs::{RepositoryTabs, TabId};
use crate::theme::{AppearanceChoice, ThemeName};
use crate::watch::RepoWatcher;

pub(crate) struct RoutedEffects {
    pub id: TabId,
    pub effects: Vec<AppEffect>,
}

struct RepositoryRuntime {
    app: App,
    worker: GitWorker,
    watcher: Option<RepoWatcher>,
}

impl RepositoryRuntime {
    fn new(
        repository: &Repository,
        theme: ThemeName,
        appearance: AppearanceChoice,
        mouse: bool,
        webhooks_listening: bool,
        host_client: Option<Client>,
    ) -> Self {
        let common_dir = repository.git_common_dir().ok();
        let worker = GitWorker::start(repository.clone());
        let watcher = RepoWatcher::with_extra(repository.root(), common_dir.as_deref()).ok();
        let mut app = App::new(repository.root(), repository.name());
        app.set_theme_selection(theme, appearance);
        app.configure_mouse_capture(mouse);
        app.webhooks_listening = webhooks_listening;
        app.set_host_client(host_client);
        Self {
            app,
            worker,
            watcher,
        }
    }
}

pub(crate) struct RepositoryWorkspace {
    tabs: RepositoryTabs<RepositoryRuntime>,
}

impl RepositoryWorkspace {
    pub(crate) fn new(
        repository: &Repository,
        theme: ThemeName,
        appearance: AppearanceChoice,
        mouse: bool,
        webhooks_listening: bool,
        host_client: Option<Client>,
    ) -> Self {
        let title = repository.name();
        let root = repository.root().to_path_buf();
        let runtime = RepositoryRuntime::new(
            repository,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            host_client,
        );
        Self {
            tabs: RepositoryTabs::new(title, root, runtime),
        }
    }

    pub(crate) const fn active_id(&self) -> Option<TabId> {
        self.tabs.active_id()
    }

    pub(crate) fn active_app_mut(&mut self) -> Option<&mut App> {
        self.tabs.active_mut().map(|runtime| &mut runtime.app)
    }

    pub(crate) fn app_mut(&mut self, id: TabId) -> Option<&mut App> {
        self.tabs.get_mut(id).map(|runtime| &mut runtime.app)
    }

    pub(crate) fn initial_effects(&mut self) -> Option<RoutedEffects> {
        let id = self.active_id()?;
        let effects = self.app_mut(id)?.initial_effects();
        Some(RoutedEffects { id, effects })
    }

    pub(crate) fn open_pull_request_on_launch(&mut self, number: u64) -> Option<RoutedEffects> {
        let id = self.active_id()?;
        let effects = self.app_mut(id)?.open_pull_request_on_launch(number);
        Some(RoutedEffects { id, effects })
    }

    pub(crate) fn sync_tabs(&mut self, now: Instant) {
        let infos = self.tabs.infos();
        let active = self.active_id();
        for (id, runtime) in self.tabs.iter_mut() {
            runtime.app.set_tab_active(active == Some(id), now);
            if active == Some(id) {
                runtime.app.set_repository_tabs(infos.clone());
            }
        }
    }

    pub(crate) fn propagate_preferences(&mut self, source: TabId) {
        let Some(source) = self.tabs.get(source) else {
            return;
        };
        let theme = source.app.theme_name;
        let appearance = source.app.appearance_choice;
        let mouse = source.app.mouse_capture_preference;
        for (_, runtime) in self.tabs.iter_mut() {
            if runtime.app.theme_name != theme || runtime.app.appearance_choice != appearance {
                runtime.app.set_theme_selection(theme, appearance);
            }
            if runtime.app.mouse_capture_preference != mouse {
                runtime.app.configure_mouse_capture(mouse);
            }
        }
    }

    pub(crate) fn send(&self, id: TabId, command: WorkerCommand) -> bool {
        self.tabs
            .get(id)
            .is_some_and(|runtime| runtime.worker.send(command))
    }

    pub(crate) fn activate(&mut self, id: TabId, now: Instant) {
        if self.tabs.activate(id) {
            self.sync_tabs(now);
        }
    }

    pub(crate) fn reorder(&mut self, source: TabId, target: TabId, now: Instant) {
        if self.tabs.reorder(source, target) {
            self.sync_tabs(now);
        }
    }

    pub(crate) fn close(&mut self, id: TabId, now: Instant) -> bool {
        drop(self.tabs.close(id));
        if self.tabs.is_empty() {
            return false;
        }
        self.sync_tabs(now);
        true
    }

    pub(crate) fn close_others(&mut self, id: TabId, now: Instant) {
        drop(self.tabs.close_others(id));
        self.sync_tabs(now);
    }

    pub(crate) fn close_all(&mut self) {
        drop(self.tabs.close_all());
    }

    pub(crate) fn switch_repository(
        &mut self,
        source: TabId,
        path: &Path,
        now: Instant,
    ) -> Option<RoutedEffects> {
        let repository = match Repository::discover(path) {
            Ok(repository) => repository,
            Err(error) => {
                if let Some(app) = self.app_mut(source) {
                    app.show_toast(error.to_string(), ToastLevel::Error, now);
                }
                return None;
            }
        };
        crate::state::record_recent_project(repository.root());
        self.replace_repository(source, &repository, now)
    }

    pub(crate) fn open_repository_tab(
        &mut self,
        source: TabId,
        path: &Path,
        now: Instant,
    ) -> Option<RoutedEffects> {
        let repository = match Repository::discover(path) {
            Ok(repository) => repository,
            Err(error) => {
                if let Some(app) = self.app_mut(source) {
                    app.show_toast(error.to_string(), ToastLevel::Error, now);
                }
                return None;
            }
        };
        if let Some(id) = self.tabs.id_for_root(repository.root()) {
            self.activate(id, now);
            return None;
        }
        crate::state::record_recent_project(repository.root());
        self.append_repository(source, &repository, now)
    }

    fn replace_repository(
        &mut self,
        source: TabId,
        repository: &Repository,
        now: Instant,
    ) -> Option<RoutedEffects> {
        let source_runtime = self.tabs.get(source)?;
        let theme = source_runtime.app.theme_name;
        let appearance = source_runtime.app.appearance_choice;
        let mouse = source_runtime.app.mouse_capture_preference;
        let webhooks_listening = source_runtime.app.webhooks_listening;
        let host_client = source_runtime.app.host_client;
        let title = repository.name();
        let root = repository.root().to_path_buf();
        let runtime = RepositoryRuntime::new(
            repository,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            host_client,
        );
        drop(self.tabs.replace(source, title, root, runtime));
        self.sync_tabs(now);
        let effects = self.app_mut(source)?.initial_effects();
        Some(RoutedEffects {
            id: source,
            effects,
        })
    }

    fn append_repository(
        &mut self,
        source: TabId,
        repository: &Repository,
        now: Instant,
    ) -> Option<RoutedEffects> {
        let source = self.tabs.get(source).or_else(|| self.tabs.active())?;
        let theme = source.app.theme_name;
        let appearance = source.app.appearance_choice;
        let mouse = source.app.mouse_capture_preference;
        let webhooks_listening = source.app.webhooks_listening;
        let host_client = source.app.host_client;
        let title = repository.name();
        let root = repository.root().to_path_buf();
        let runtime = RepositoryRuntime::new(
            repository,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            host_client,
        );
        let id = self.tabs.append(title, root, runtime);
        self.sync_tabs(now);
        let effects = self.app_mut(id)?.initial_effects();
        Some(RoutedEffects { id, effects })
    }

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
            let Some(receiver) = runtime.watcher.as_ref().map(RepoWatcher::changes) else {
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

#[cfg(test)]
mod tests;
