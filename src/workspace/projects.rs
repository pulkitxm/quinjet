use std::path::Path;
use std::time::Instant;

use super::{RepositoryRuntime, RepositoryWorkspace, RoutedEffects, WorkspaceContext};
use crate::app::{Modal, ProjectOpenMode, ToastLevel};
use crate::git::Repository;
use crate::ssh::{SshProjectOpenMode, SshSwitch};
use crate::tabs::TabId;

impl RepositoryWorkspace {
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
                    if let Some(Modal::Projects { opening, .. }) = app.modal.as_mut() {
                        *opening = None;
                    }
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
                    if let Some(Modal::Projects { opening, .. }) = app.modal.as_mut() {
                        *opening = None;
                    }
                    app.show_toast(error.to_string(), ToastLevel::Error, now);
                }
                return None;
            }
        };
        if self.tabs.is_pending(source) {
            crate::state::record_recent_project(repository.root());
            return self.replace_repository(source, &repository, now);
        }
        if let Some(app) = self.app_mut(source) {
            app.modal = None;
        }
        if let Some(id) = self.tabs.id_for_root(repository.root()) {
            let _handoff = self.activate(id, now);
            return None;
        }
        crate::state::record_recent_project(repository.root());
        self.append_repository(source, &repository, now)
    }

    pub(crate) fn open_repository_tab_picker(
        &mut self,
        source: TabId,
        now: Instant,
    ) -> Option<RoutedEffects> {
        let source_runtime = self.tabs.get(source).or_else(|| self.tabs.active())?;
        let repository = Repository::discover(&source_runtime.app.repository_root).ok()?;
        let theme = source_runtime.app.theme_name;
        let appearance = source_runtime.app.appearance_choice;
        let mouse = source_runtime.app.mouse_capture_preference;
        let webhooks_listening = source_runtime.app.webhooks_listening;
        let host_client = source_runtime.app.host_client;
        let root = repository.root().to_path_buf();
        let id = self.ssh_context.as_mut().map_or_else(
            || TabId::new(0),
            |context| {
                context
                    .tabs
                    .append_pending(context.current.clone(), root.clone())
            },
        );
        let runtime = RepositoryRuntime::new(
            &repository,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            WorkspaceContext::new(self.ssh_context.clone(), host_client),
        );
        let id = if self.ssh_context.is_some() {
            self.tabs
                .append_pending_with_id(id, "New project", root, runtime)
        } else {
            self.tabs.append_pending("New project", root, runtime)
        };
        self.sync_tabs(now);
        let effects = self
            .app_mut(id)?
            .open_projects_on_launch(ProjectOpenMode::NewTab);
        Some(RoutedEffects { id, effects })
    }

    pub(crate) fn cancel_repository_tab_picker(
        &mut self,
        source: TabId,
        now: Instant,
    ) -> (bool, Option<SshSwitch>) {
        if !self.tabs.is_pending(source) {
            return (true, None);
        }
        self.close(source, now)
    }

    pub(crate) fn prepare_ssh_switch(&mut self, source: TabId, request: SshSwitch) {
        if request.mode != SshProjectOpenMode::New {
            return;
        }
        let Some(context) = self.ssh_context.as_mut() else {
            return;
        };
        let pending = self
            .tabs
            .is_pending(source)
            .then_some(source)
            .or_else(|| context.tabs.active_pending_for_machine(&context.current));
        let Some(pending) = pending else {
            return;
        };
        let Some(machine) = context.machines.get(request.index) else {
            return;
        };
        let _moved =
            context
                .tabs
                .move_pending(pending, machine.target.clone(), machine.folder.clone());
    }

    pub(super) fn replace_repository(
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
        let context =
            WorkspaceContext::new(self.ssh_context.clone(), source_runtime.app.host_client);
        let title = repository.name();
        let root = repository.root().to_path_buf();
        let runtime = RepositoryRuntime::new(
            repository,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            context,
        );
        drop(self.tabs.replace(source, title, root, runtime));
        if let Some(context) = self.ssh_context.as_mut() {
            let _replaced =
                context
                    .tabs
                    .replace(source, repository.name(), repository.root().to_path_buf());
            drop(context.tabs.activate(source));
        }
        self.sync_tabs(now);
        let effects = self.app_mut(source)?.initial_effects();
        Some(RoutedEffects {
            id: source,
            effects,
        })
    }

    pub(super) fn append_repository(
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
        let context = WorkspaceContext::new(self.ssh_context.clone(), source.app.host_client);
        let title = repository.name();
        let root = repository.root().to_path_buf();
        let runtime = RepositoryRuntime::new(
            repository,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            context,
        );
        let id = if let Some(context) = self.ssh_context.as_mut() {
            let id = context
                .tabs
                .id_for_root(&context.current, &root)
                .unwrap_or_else(|| {
                    context
                        .tabs
                        .append(context.current.clone(), title.clone(), root.clone())
                });
            drop(context.tabs.activate(id));
            self.tabs.append_with_id(id, title, root, runtime)
        } else {
            self.tabs.append(title, root, runtime)
        };
        self.sync_tabs(now);
        let effects = self.app_mut(id)?.initial_effects();
        Some(RoutedEffects { id, effects })
    }

    pub(super) fn follow_shared_active(&mut self, now: Instant) -> Option<SshSwitch> {
        let active = self.ssh_context.as_ref()?.tabs.active_id()?;
        if self.tabs.activate(active) {
            self.sync_tabs(now);
            None
        } else {
            self.switch_to_shared_tab(active)
        }
    }

    pub(super) fn prune_missing_shared_tabs(&mut self) {
        let roots = self
            .tabs
            .infos()
            .into_iter()
            .map(|tab| tab.root)
            .collect::<Vec<_>>();
        let Some(context) = self.ssh_context.as_mut() else {
            return;
        };
        let missing = context
            .tabs
            .entries_for_machine(&context.current)
            .filter(|tab| {
                !roots
                    .iter()
                    .any(|root| crate::git::support::same_path(root, &tab.root))
            })
            .map(|tab| tab.id)
            .collect::<Vec<_>>();
        for id in missing {
            drop(context.tabs.close(id));
        }
    }

    pub(super) fn restore_pending_runtime(
        &mut self,
        pending: Option<TabId>,
        now: Instant,
    ) -> Option<TabId> {
        let context = self.ssh_context.as_ref()?;
        let id = pending.filter(|id| context.tabs.is_pending(*id))?;
        if self.tabs.get(id).is_some() {
            return Some(id);
        }
        let root = context.tabs.get(id)?.root.clone();
        let source = self.tabs.active()?;
        let repository = Repository::discover(&source.app.repository_root).ok()?;
        let runtime = RepositoryRuntime::new(
            &repository,
            source.app.theme_name,
            source.app.appearance_choice,
            source.app.mouse_capture_preference,
            source.app.webhooks_listening,
            WorkspaceContext::new(self.ssh_context.clone(), source.app.host_client),
        );
        let id = self
            .tabs
            .append_pending_with_id(id, "New project", root, runtime);
        self.sync_tabs(now);
        Some(id)
    }

    pub(super) fn switch_to_shared_tab(&mut self, id: TabId) -> Option<SshSwitch> {
        let context = self.ssh_context.as_mut()?;
        let machine = context.tabs.get(id)?.machine.clone();
        let index = context
            .machines
            .iter()
            .position(|candidate| candidate.target == machine && candidate.accessible)?;
        drop(context.tabs.activate(id));
        Some(SshSwitch {
            index,
            mode: SshProjectOpenMode::Activate,
        })
    }
}
