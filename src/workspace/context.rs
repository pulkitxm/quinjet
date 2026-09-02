use crate::app::App;
use crate::git::Repository;
use crate::git::worker::GitWorker;
use crate::integration::Client;
use crate::ssh::SshContext;
use crate::theme::{AppearanceChoice, ThemeSelection};
use crate::watch::RepoWatcher;

#[derive(Clone)]
pub(crate) struct WorkspaceContext {
    pub(super) ssh: Option<SshContext>,
    client: Option<Client>,
}

impl WorkspaceContext {
    pub(crate) const fn new(ssh: Option<SshContext>, client: Option<Client>) -> Self {
        Self { ssh, client }
    }
}

pub(super) struct RepositoryRuntime {
    pub(super) app: App,
    pub(super) worker: GitWorker,
    pub(super) watcher: Option<RepoWatcher>,
}

impl RepositoryRuntime {
    pub(super) fn new(
        repository: &Repository,
        theme: ThemeSelection,
        appearance: AppearanceChoice,
        mouse: bool,
        webhooks_listening: bool,
        context: WorkspaceContext,
    ) -> Self {
        let common_dir = repository.git_common_dir().ok();
        let worker = GitWorker::start(repository.clone());
        let watcher = RepoWatcher::with_extra(repository.root(), common_dir.as_deref()).ok();
        let mut app = App::new(repository.root(), repository.name());
        app.set_theme_selection(theme, appearance);
        app.configure_mouse_capture(mouse);
        app.local_browser = crate::cli::browser_is_local();
        app.webhooks_listening = webhooks_listening;
        app.ssh_context = context.ssh;
        app.set_host_client(context.client);
        Self {
            app,
            worker,
            watcher,
        }
    }
}
