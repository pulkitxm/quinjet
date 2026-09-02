use super::*;

#[test]
fn pull_request_open_uses_the_selected_check_url() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let opened = fixture
        .read(&["pr", "open", "42", "--check", "Unit tests"])?
        .success()?;

    let expected = "https://github.com/acme/project/actions/runs/77/job/123";
    ensure!(opened.stderr.is_empty(), "{}", opened.stderr);
    ensure!(opened.stdout == format!("Opened {expected}\n"));
    ensure!(wait_for_capture(&fixture.open_capture)?.trim() == expected);
    ensure!(fixture.gh_calls()?.contains("argv\tpr\tchecks\t42"));
    Ok(())
}

#[test]
fn pull_request_open_inside_an_ssh_session_prints_the_url_without_an_opener() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    let mut command = fixture.command(&["pr", "open", "42", "--json"])?;
    command.env("SSH_CONNECTION", "203.0.113.7 51000 198.51.100.4 22");

    let opened = Run::from(
        command
            .output()
            .context("failed to run Quinjet inside a fake SSH session")?,
    )?
    .success()?;

    ensure!(opened.stderr.is_empty(), "{}", opened.stderr);
    ensure!(opened.json()?["message"] == "https://github.com/acme/project/pull/42");
    ensure!(!fixture.open_capture.exists());
    Ok(())
}
