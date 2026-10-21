use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use super::Repository;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Accounts {
    pub logins: Vec<String>,
    pub selected: Option<String>,
    pub name: String,
    pub email: String,
}

#[derive(Deserialize)]
struct User {
    login: String,
    id: u64,
    name: Option<String>,
}

impl Repository {
    fn account_command(&self) -> Command {
        let mut command = Command::new(self.github_cli.as_deref().unwrap_or_else(|| "gh".as_ref()));
        let _ = command
            .current_dir(self.root())
            .env_remove("GH_TOKEN")
            .env_remove("GITHUB_TOKEN")
            .env("GH_PROMPT_DISABLED", "1");
        command
    }

    pub(crate) fn account_config(&self, key: &str) -> Result<Option<String>> {
        let output = self.run(["config", "--get", key])?;
        if output.status.success() {
            return Ok(Some(String::from_utf8(output.stdout)?.trim().to_owned()));
        }
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        bail!("Could not read repository account configuration");
    }

    pub(crate) fn accounts(&self) -> Result<Accounts> {
        let output = self
            .account_command()
            .args([
                "auth",
                "status",
                "--hostname",
                "github.com",
                "--json",
                "hosts",
                "--jq",
                ".hosts[\"github.com\"] | map(.login)",
            ])
            .output()
            .context("Install GitHub CLI and run `gh auth login` for each account")?;
        let logins: Vec<String> = serde_json::from_slice(&output.stdout)
            .context("Could not list saved accounts; run `gh auth login` for each account")?;
        Ok(Accounts {
            logins,
            selected: self.account_config("quinjet.account")?,
            name: self.account_config("user.name")?.unwrap_or_default(),
            email: self.account_config("user.email")?.unwrap_or_default(),
        })
    }

    fn account_token(&self, login: &str) -> Result<String> {
        validate_login(login)?;
        let output = self
            .account_command()
            .args(["auth", "token", "--hostname", "github.com", "--user", login])
            .output()
            .context("Could not read the selected account from GitHub CLI")?;
        if !output.status.success() {
            bail!("No saved credential for {login}; run `gh auth login` for that account");
        }
        let token = String::from_utf8(output.stdout)
            .map_err(|_| anyhow!("Selected account credential is not valid UTF-8"))?
            .trim()
            .to_owned();
        if token.is_empty() {
            bail!("GitHub CLI returned an empty credential for {login}");
        }
        Ok(token)
    }

    pub(crate) fn pin_github_command(&self, command: &mut Command) -> Result<()> {
        if let Some(login) = self.account_config("quinjet.account")? {
            let _ = command
                .env("GH_TOKEN", self.account_token(&login)?)
                .env_remove("GITHUB_TOKEN");
        }
        Ok(())
    }

    pub(crate) fn switch_account(&self, login: &str) -> Result<String> {
        validate_login(login)?;
        let token = self.account_token(login)?;
        let output = self
            .account_command()
            .env("GH_TOKEN", token)
            .args(["api", "--hostname", "github.com", "user"])
            .output()
            .context("Could not verify the selected GitHub identity")?;
        if !output.status.success() {
            bail!("Could not verify {login}; check `gh auth status` and network access");
        }
        let user: User = serde_json::from_slice(&output.stdout)?;
        if !user.login.eq_ignore_ascii_case(login) {
            bail!("Saved credential does not belong to {login}; account settings were not changed");
        }
        let email = format!("{}+{}@users.noreply.github.com", user.id, user.login);
        let name = user
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| user.login.clone());
        self.save_account(&user.login, &name, &email)?;
        Ok(format!(
            "Repository account: {}. Commits: {name} <{email}>. HTTPS credentials pinned; SSH uses your SSH configuration. Commit signing disabled for this repository; configure the selected account's signing key to re-enable it.",
            user.login
        ))
    }

    pub(crate) fn verify_commit_account(&self, amend: bool) -> Result<()> {
        if self.account_config("quinjet.account")?.is_none() {
            return Ok(());
        }
        let name = self
            .account_config("quinjet.name")?
            .context("Reselect the repository account")?;
        let email = self
            .account_config("quinjet.email")?
            .context("Reselect the repository account")?;
        let expected = format!("{name} <{email}>");
        for variable in ["GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"] {
            let ident = String::from_utf8(self.checked(["var", variable])?)?;
            if !ident.starts_with(&format!("{expected} ")) {
                bail!(
                    "Commit identity differs from the selected account; clear author/committer environment overrides or reselect the repository account"
                );
            }
        }
        if amend {
            let author = String::from_utf8(self.checked(["log", "-1", "--format=%an <%ae>"])?)?;
            if author.trim() != expected {
                bail!(
                    "Cannot amend a commit authored by another identity with the selected account; create a new commit or select the original account"
                );
            }
        }
        Ok(())
    }

    fn save_account(&self, login: &str, name: &str, email: &str) -> Result<()> {
        let path = self.checked(["rev-parse", "--git-common-dir"])?;
        let directory = self.root().join(String::from_utf8(path)?.trim());
        let config = directory.join("config");
        let lock_path = directory.join("config.lock");
        let lock = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .context(
                "Repository configuration is locked by another process; retry the account switch",
            )?;
        let _guard = ConfigLock {
            path: lock_path,
            _file: lock,
        };
        let temporary = tempfile::NamedTempFile::new_in(&directory)?;
        let _ = std::fs::copy(&config, temporary.path())?;
        let helper = credential_helper(login);
        for (key, value) in [
            ("user.name", name),
            ("user.email", email),
            ("quinjet.account", login),
            ("quinjet.name", name),
            ("quinjet.email", email),
            ("commit.gpgsign", "false"),
            ("credential.https://github.com.username", login),
            ("credential.https://github.com.helper", ""),
        ] {
            let output = Command::new("git")
                .arg("config")
                .arg("--file")
                .arg(temporary.path())
                .args(["--replace-all", key, value])
                .output()?;
            if !output.status.success() {
                bail!("Could not prepare repository account settings");
            }
        }
        let output = Command::new("git")
            .arg("config")
            .arg("--file")
            .arg(temporary.path())
            .args(["--add", "credential.https://github.com.helper", &helper])
            .output()?;
        if !output.status.success() {
            bail!("Could not prepare repository credentials");
        }
        drop(
            temporary
                .persist(config)
                .context("Could not save repository account settings")?,
        );
        Ok(())
    }
}

fn validate_login(login: &str) -> Result<()> {
    if login.is_empty()
        || login.len() > 39
        || login.starts_with('-')
        || login.ends_with('-')
        || !login
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        bail!("Enter a GitHub login using letters, digits and interior hyphens");
    }
    Ok(())
}

fn credential_helper(login: &str) -> String {
    format!(
        "!f() {{ test \"$1\" = get || exit 0; password=$(GH_TOKEN= GITHUB_TOKEN= gh auth token --hostname github.com --user {login}) || exit 1; test -n \"$password\" || exit 1; printf 'username={login}\\npassword=%s\\n' \"$password\"; }}; f"
    )
}

struct ConfigLock {
    path: std::path::PathBuf,
    _file: std::fs::File,
}

impl Drop for ConfigLock {
    fn drop(&mut self) {
        drop(std::fs::remove_file(&self.path));
    }
}

#[cfg(test)]
mod tests {
    use anyhow::ensure;

    use super::*;
    use crate::git::GitOperation;
    use crate::git::tests::TestRepository;

    #[test]
    fn account_selection_is_local_and_commits_keep_the_selected_identity() -> Result<()> {
        let personal = TestRepository::with_branch("main");
        let company = TestRepository::with_branch("main");
        let personal = personal.repository();
        let company = company.repository();
        personal.save_account("personal", "Personal", "personal@example.com")?;
        company.save_account("company", "Company", "company@example.com")?;
        for (repo, name, email) in [
            (&personal, "Personal", "personal@example.com"),
            (&company, "Company", "company@example.com"),
        ] {
            std::fs::write(repo.root().join("new.txt"), "new content")?;
            drop(repo.perform(&GitOperation::StageAll)?);
            drop(repo.perform(&GitOperation::Commit {
                message: "identity test".to_owned(),
                amend: false,
            })?);
            let author =
                String::from_utf8(repo.checked(["log", "-1", "--format=%an <%ae>|%cn <%ce>"])?)?;
            ensure!(author.trim() == format!("{name} <{email}>|{name} <{email}>"));
        }
        ensure!(personal.status()?.account.as_deref() == Some("personal"));
        ensure!(company.status()?.account.as_deref() == Some("company"));
        personal.save_account("another", "Another", "another@example.com")?;
        ensure!(personal.status()?.account.as_deref() == Some("another"));
        ensure!(company.account_config("user.email")?.as_deref() == Some("company@example.com"));
        let error = personal
            .verify_commit_account(true)
            .err()
            .context("cross-account amend should fail")?;
        ensure!(error.to_string().contains("Cannot amend"));
        drop(personal.checked(["config", "user.email", "wrong@example.com"])?);
        ensure!(personal.verify_commit_account(false).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn malformed_credentials_never_appear_in_errors() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let scratch = TestRepository::with_branch("main");
        let script = scratch.repository().root().join("fake-gh");
        std::fs::write(
            &script,
            "#!/bin/sh\nprintf 'private-credential-value\\377'\n",
        )?;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))?;
        let repo = scratch.repository_with_github_cli(script);
        let error = repo
            .account_token("personal")
            .err()
            .context("Invalid UTF-8 must fail")?;
        ensure!(error.to_string() == "Selected account credential is not valid UTF-8");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn verifies_saved_credentials_before_mutating_and_pins_api_identity() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let scratch = TestRepository::with_branch("main");
        let script = scratch.repository().root().join("fake-gh");
        std::fs::write(
            &script,
            r#"#!/bin/sh
if [ "$1 $2" = 'auth token' ]; then printf 'saved-test-token'; exit 0; fi
if [ "$1" = api ]; then
  test "$GH_TOKEN" = saved-test-token || exit 1
  printf '{"login":"personal","id":123,"name":"Personal"}'
  exit 0
fi
exit 1
"#,
        )?;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))?;
        let repo = scratch.repository_with_github_cli(script);
        let before = std::fs::read(repo.root().join(".git/config"))?;
        ensure!(repo.switch_account("company").is_err());
        ensure!(std::fs::read(repo.root().join(".git/config"))? == before);
        drop(repo.switch_account("personal")?);
        ensure!(
            repo.account_config("user.email")?.as_deref()
                == Some("123+personal@users.noreply.github.com")
        );
        let mut command = Command::new("unused");
        repo.pin_github_command(&mut command)?;
        ensure!(command.get_envs().any(|(key, value)| key == "GH_TOKEN" && value == Some("saved-test-token".as_ref())));
        ensure!(
            !std::fs::read_to_string(repo.root().join(".git/config"))?.contains("saved-test-token")
        );
        Ok(())
    }

    #[test]
    fn switch_respects_git_config_lock_and_rejects_shell_input() -> Result<()> {
        for login in ["", "-flag", "name;id", "a b", "$(id)", "trailing-"] {
            ensure!(validate_login(login).is_err(), "{login}");
        }
        let scratch = TestRepository::with_branch("main");
        let repo = scratch.repository();
        let lock = repo.root().join(".git/config.lock");
        std::fs::write(&lock, "locked")?;
        ensure!(
            repo.save_account("personal", "Personal", "personal@example.com")
                .is_err()
        );
        ensure!(std::fs::read_to_string(lock)? == "locked");
        ensure!(repo.account_config("quinjet.account")?.is_none());
        Ok(())
    }
}
