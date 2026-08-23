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

#[derive(Clone, Copy, PartialEq, Eq)]
enum InitialProjectMode {
    Normal,
    PendingHost,
    ResolvePending,
}

impl RepositoryWorkspace {
    pub(crate) fn new(
        repository: &Repository,
        theme: ThemeName,
        appearance: AppearanceChoice,
        mouse: bool,
        webhooks_listening: bool,
        context: WorkspaceContext,
    ) -> Self {
        Self::new_with_mode(
            repository,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            context,
            InitialProjectMode::Normal,
        )
    }

    pub(crate) fn new_pending_host(
        repository: &Repository,
        theme: ThemeName,
        appearance: AppearanceChoice,
        mouse: bool,
        webhooks_listening: bool,
        context: WorkspaceContext,
    ) -> Self {
        Self::new_with_mode(
            repository,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            context,
            InitialProjectMode::PendingHost,
        )
    }

    pub(crate) fn new_resolving_pending(
        repository: &Repository,
        theme: ThemeName,
        appearance: AppearanceChoice,
        mouse: bool,
        webhooks_listening: bool,
        context: WorkspaceContext,
    ) -> Self {
        Self::new_with_mode(
            repository,
            theme,
            appearance,
            mouse,
            webhooks_listening,
            context,
            InitialProjectMode::ResolvePending,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the shared constructor adds one pending-tab mode to the public constructor inputs"
    )]
    fn new_with_mode(
        repository: &Repository,
        theme: ThemeName,
        appearance: AppearanceChoice,
        mouse: bool,
        webhooks_listening: bool,
        mut context: WorkspaceContext,
        mode: InitialProjectMode,
    ) -> Self {
        let title = repository.name();
        let root = repository.root().to_path_buf();
        let id = context.ssh.as_mut().map_or_else(
            || TabId::new(0),
            |context| {
                let pending = context.tabs.active_pending_for_machine(&context.current);
                let id = match (mode, pending) {
                    (InitialProjectMode::PendingHost, Some(id)) => id,
                    (InitialProjectMode::ResolvePending, Some(id)) => {
                        let _replaced = context.tabs.replace(id, title.clone(), root.clone());
                        id
                    }
                    _ => context
                        .tabs
                        .id_for_root(&context.current, &root)
                        .unwrap_or_else(|| {
                            context.tabs.append(
                                context.current.clone(),
                                title.clone(),
                                root.clone(),
                            )
                        }),
                };
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
        let pending = context
            .ssh
            .as_ref()
            .is_some_and(|context| context.tabs.is_pending(id));
        let tabs = if pending {
            RepositoryTabs::new_pending_with_id(id, "New project", root, runtime)
        } else {
            RepositoryTabs::new_with_id(id, title, root, runtime)
        };
        Self {
            tabs,
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
        let pending = current_machine.and_then(|machine| {
            ssh_context.and_then(|context| context.tabs.active_pending_for_machine(machine))
        });
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
        let restored_pending = workspace.restore_pending_runtime(pending, Instant::now());
        if let Some(id) = restored_pending
            .or(restored_active)
            .or_else(|| workspace.active_id())
        {
            let _handoff = workspace.activate(id, Instant::now());
        } else {
            workspace.sync_tabs(Instant::now());
        }
        Some(workspace)
    }

    pub(crate) fn project_session(&self) -> ProjectSession {
        let infos = self
            .tabs
            .infos()
            .into_iter()
            .filter(|tab| !self.tabs.is_pending(tab.id))
            .collect::<Vec<_>>();
        ProjectSession {
            roots: infos.iter().map(|tab| tab.root.clone()).collect(),
            active: infos.into_iter().find(|tab| tab.active).map(|tab| tab.root),
        }
    }

    pub(crate) fn ssh_context(&self) -> Option<SshContext> {
        self.ssh_context.clone()
    }

    pub(crate) fn apply_ssh_probe(&mut self, accessibility: &[(String, bool)], now: Instant) {
        let Some(context) = self.ssh_context.as_mut() else {
            return;
        };
        for machine in &mut context.machines {
            if let Some((_, accessible)) = accessibility
                .iter()
                .find(|(target, _)| target == &machine.target)
            {
                machine.accessible = *accessible;
            }
        }
        context.probing = false;
        self.sync_tabs(now);
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
        let ssh_context = self.ssh_context.clone();
        for (id, runtime) in self.tabs.iter_mut() {
            runtime.app.ssh_context.clone_from(&ssh_context);
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

    fn restore_pending_runtime(&mut self, pending: Option<TabId>, now: Instant) -> Option<TabId> {
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
