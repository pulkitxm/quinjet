#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(crate) fn repository_open_target(&self) -> Option<OpenTarget> {
        self.github_repository_context()
            .map(|repository| OpenTarget::Browser(repository.url.clone()))
    }

    pub(crate) fn pull_request_repository_open_target(&self) -> Option<OpenTarget> {
        self.pull_request_repository
            .as_ref()
            .map(|repository| OpenTarget::Browser(repository.url.clone()))
            .or_else(|| self.repository_open_target())
    }

    pub(crate) fn branch_open_target(&self, branch: &str) -> Option<OpenTarget> {
        self.repository_web_url().map(|repository| {
            let mut url = repository.trim_end_matches('/').to_owned();
            url.push_str("/tree/");
            url.push_str(&encode_url_path(branch));
            OpenTarget::Browser(url)
        })
    }

    pub(crate) fn commit_open_target(&self, commit: &str) -> Option<OpenTarget> {
        self.repository_web_url().map(|repository| {
            let mut url = repository.trim_end_matches('/').to_owned();
            url.push_str("/commit/");
            url.push_str(&encode_url_path(commit));
            OpenTarget::Browser(url)
        })
    }

    pub(crate) fn pull_request_open_target(&self, number: u64) -> Option<OpenTarget> {
        if let Some(pull_request) = self
            .pull_request
            .as_ref()
            .filter(|pull_request| pull_request.number == number)
        {
            return Some(OpenTarget::Browser(pull_request.url.clone()));
        }
        self.repository_web_url().map(|repository| {
            OpenTarget::Browser(format!(
                "{}/pull/{number}",
                repository.trim_end_matches('/')
            ))
        })
    }

    pub(crate) fn account_open_target(&self, account: &str) -> Option<OpenTarget> {
        if account.is_empty()
            || account.len() > 39
            || !account
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return None;
        }
        let repository = self
            .pull_request
            .as_ref()
            .map(|pull_request| &pull_request.base_repository)
            .or(self.local_github_repository.as_ref())?;
        let root = repository_root_url(repository)?;
        Some(OpenTarget::Browser(format!(
            "{}/{}",
            root.trim_end_matches('/'),
            account
        )))
    }

    pub(crate) fn pull_request_base_branch_open_target(&self) -> Option<OpenTarget> {
        let pull_request = self.pull_request.as_ref()?;
        repository_branch_open_target(&pull_request.base_repository.url, &pull_request.base_ref)
    }

    pub(crate) fn pull_request_head_branch_open_target(&self) -> Option<OpenTarget> {
        let pull_request = self.pull_request.as_ref()?;
        let repository_name = pull_request.head_repository.as_deref()?;
        let repository_url = if repository_name
            .eq_ignore_ascii_case(&pull_request.base_repository.name_with_owner)
        {
            pull_request.base_repository.url.clone()
        } else {
            let root = repository_root_url(&pull_request.base_repository)?;
            format!("{}/{repository_name}", root.trim_end_matches('/'))
        };
        repository_branch_open_target(&repository_url, &pull_request.head_ref)
    }

    pub(super) fn repository_web_url(&self) -> Option<&str> {
        self.github_repository_context()
            .map(|repository| repository.url.as_str())
    }

    pub(super) fn github_repository_context(&self) -> Option<&GitHubRepository> {
        self.local_github_repository.as_ref().or_else(|| {
            self.pull_request
                .as_ref()
                .map(|pull_request| &pull_request.base_repository)
        })
    }

    pub(crate) fn open_link(&mut self, url: String, effects: &mut Vec<AppEffect>, now: Instant) {
        if self.local_browser {
            self.show_toast(format!("Opening {url}"), ToastLevel::Info, now);
            effects.push(AppEffect::Open(OpenTarget::Browser(url)));
            return;
        }
        effects.push(AppEffect::Copy(url.clone()));
        self.show_toast(
            format!("Copied {url}. Cmd-click or Ctrl-click the link to open it in your browser"),
            ToastLevel::Info,
            now,
        );
    }

    pub(crate) fn show_toast(&mut self, message: String, level: ToastLevel, now: Instant) {
        self.toast = Some(Toast {
            message,
            level,
            expires_at: now + TOAST_DURATION,
        });
    }
}
