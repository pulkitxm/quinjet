use std::path::Path;
use std::time::Instant;

use crate::app::{App, AppEffect, Modal, ProjectOpenMode, ToastLevel};
use crate::git::Repository;
use crate::git::worker::WorkerCommand;
use crate::ssh::{SshContext, SshProjectOpenMode, SshSwitch};
use crate::state::session::ProjectSession;
use crate::tabs::{RepositoryTabs, TabId};
use crate::theme::{AppearanceChoice, ThemeName};

mod context;
use context::RepositoryRuntime;
pub(crate) use context::WorkspaceContext;

pub(crate) struct RoutedEffects {
    pub id: TabId,
    pub effects: Vec<AppEffect>,
}

pub(crate) struct RepositoryWorkspace {
    tabs: RepositoryTabs<RepositoryRuntime>,
    ssh_context: Option<SshContext>,
}

impl RepositoryWorkspace {
    pub(crate) fn new(
        repository: &Repository,
        theme: ThemeName,
        appearance: AppearanceChoice,
        mouse: bool,
        webhooks_listening: bool,
        mut context: WorkspaceContext,
    ) -> Self {
        let title = repository.name();
        let root = repository.root().to_path_buf();
        let id = context.ssh.as_mut().map_or_else(
            || TabId::new(0),
            |context| {
                let id = context
                    .tabs
                    .id_for_root(&context.current, &root)
                    .unwrap_or_else(|| {
                        context
                            .tabs
                            .append(context.current.clone(), title.clone(), root.clone())
                    });
                drop(context.tabs.activate(id));
                id
            },
        );
        let runtime = RepositoryRuntime::new(
            repository,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            context.clone(),
        );
        Self {
            tabs: RepositoryTabs::new_with_id(id, title, root, runtime),
            ssh_context: context.ssh,
        }
    }

    pub(crate) fn restore(
        session: &ProjectSession,
        theme: ThemeName,
        appearance: AppearanceChoice,
        mouse: bool,
        webhooks_listening: bool,
        context: WorkspaceContext,
    ) -> Option<Self> {
        let ssh_context = context.ssh.as_ref();
        let current_machine = ssh_context.map(|context| context.current.as_str());
        let shared_roots = current_machine.map_or_else(Vec::new, |machine| {
            ssh_context.map_or_else(Vec::new, |context| {
                context
                    .tabs
                    .entries_for_machine(machine)
                    .map(|tab| tab.root.clone())
                    .collect()
            })
        });
        let roots = if shared_roots.is_empty() {
            session.roots.clone()
        } else {
            shared_roots
        };
        let desired_root = current_machine
            .and_then(|machine| {
                ssh_context.and_then(|context| {
                    context
                        .tabs
                        .active_for_machine(machine)
                        .and_then(|id| context.tabs.get(id))
                        .map(|tab| tab.root.clone())
                })
            })
            .or_else(|| session.active.clone());
        let mut repositories = roots.iter().filter_map(|root| {
            Repository::discover(root)
                .ok()
                .map(|repository| (root, repository))
        });
        let (first_root, first) = repositories.next()?;
        let mut workspace = Self::new(
            &first,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            context,
        );
        let mut restored_active = (desired_root.as_ref() == Some(first_root))
            .then(|| workspace.active_id())
            .flatten();
        for (saved_root, repository) in repositories {
            let source = workspace.active_id()?;
            let id = workspace
                .append_repository(source, &repository, Instant::now())?
                .id;
            if desired_root.as_ref() == Some(saved_root) {
                restored_active = Some(id);
            }
        }
        workspace.prune_missing_shared_tabs();
        if let Some(id) = restored_active.or_else(|| workspace.active_id()) {
            let _handoff = workspace.activate(id, Instant::now());
        } else {
            workspace.sync_tabs(Instant::now());
        }
        Some(workspace)
    }

    pub(crate) fn project_session(&self) -> ProjectSession {
        let infos = self.tabs.infos();
        ProjectSession {
            roots: infos.iter().map(|tab| tab.root.clone()).collect(),
            active: infos.into_iter().find(|tab| tab.active).map(|tab| tab.root),
        }
    }

    pub(crate) fn ssh_context(&self) -> Option<SshContext> {
        self.ssh_context.clone()
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

    pub(crate) fn exit_locked(&self) -> bool {
        self.tabs
            .active()
            .is_some_and(|runtime| runtime.app.exit_locked())
    }

    pub(crate) fn initial_effects(&mut self) -> Vec<RoutedEffects> {
        self.tabs
            .iter_mut()
            .map(|(id, runtime)| RoutedEffects {
                id,
                effects: runtime.app.initial_effects(),
            })
            .collect()
    }

    pub(crate) fn open_projects_on_launch(
        &mut self,
        mode: ProjectOpenMode,
    ) -> Option<RoutedEffects> {
        let id = self.active_id()?;
        let effects = self.app_mut(id)?.open_projects_on_launch(mode);
        Some(RoutedEffects { id, effects })
    }

    pub(crate) fn open_pull_request_on_launch(&mut self, number: u64) -> Option<RoutedEffects> {
        let id = self.active_id()?;
        let effects = self.app_mut(id)?.open_pull_request_on_launch(number);
        Some(RoutedEffects { id, effects })
    }

    pub(crate) fn sync_tabs(&mut self, now: Instant) {
        let infos = self
            .ssh_context
            .as_ref()
            .map_or_else(|| self.tabs.infos(), |context| context.tabs.infos());
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

    pub(crate) fn activate(&mut self, id: TabId, now: Instant) -> Option<SshSwitch> {
        if self.tabs.activate(id) {
            if let Some(context) = self.ssh_context.as_mut() {
                drop(context.tabs.activate(id));
            }
            self.sync_tabs(now);
            return None;
        }
        self.switch_to_shared_tab(id)
    }

    pub(crate) fn reorder(&mut self, source: TabId, target: TabId, now: Instant) {
        let reordered = self.ssh_context.as_mut().map_or_else(
            || self.tabs.reorder(source, target),
            |context| context.tabs.reorder(source, target),
        );
        if reordered {
            if self.tabs.get(source).is_some() && self.tabs.get(target).is_some() {
                let _reordered = self.tabs.reorder(source, target);
            }
            self.sync_tabs(now);
        }
    }

    pub(crate) fn close(&mut self, id: TabId, now: Instant) -> (bool, Option<SshSwitch>) {
        if self.exit_locked() {
            return (true, None);
        }
        drop(self.tabs.close(id));
        if let Some(context) = self.ssh_context.as_mut() {
            drop(context.tabs.close(id));
            if context.tabs.infos().is_empty() {
                return (false, None);
            }
            let handoff = self.follow_shared_active(now);
            return (handoff.is_some() || !self.tabs.is_empty(), handoff);
        }
        if self.tabs.is_empty() {
            return (false, None);
        }
        self.sync_tabs(now);
        (true, None)
    }

    pub(crate) fn close_others(&mut self, id: TabId, now: Instant) -> Option<SshSwitch> {
        if let Some(context) = self.ssh_context.as_mut() {
            if !context.tabs.close_others(id) {
                return None;
            }
            if self.tabs.get(id).is_some() {
                drop(self.tabs.close_others(id));
            } else {
                drop(self.tabs.close_all());
            }
            return self.follow_shared_active(now);
        }
        drop(self.tabs.close_others(id));
        self.sync_tabs(now);
        None
    }

    pub(crate) fn close_all(&mut self) {
        drop(self.tabs.close_all());
        if let Some(context) = self.ssh_context.as_mut() {
            context.tabs.close_all();
        }
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
        let context = WorkspaceContext::new(
            source_runtime.app.ssh_context.clone(),
            source_runtime.app.host_client,
        );
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

    fn follow_shared_active(&mut self, now: Instant) -> Option<SshSwitch> {
        let active = self.ssh_context.as_ref()?.tabs.active_id()?;
        if self.tabs.activate(active) {
            self.sync_tabs(now);
            None
        } else {
            self.switch_to_shared_tab(active)
        }
    }

    fn prune_missing_shared_tabs(&mut self) {
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

    fn switch_to_shared_tab(&mut self, id: TabId) -> Option<SshSwitch> {
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

mod runtime;

#[cfg(test)]
mod tests;
