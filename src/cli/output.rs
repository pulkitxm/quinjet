use super::*;

pub(super) struct Emitter {
    pub(super) json: bool,
    pub(super) progress: Option<ProgressBar>,
}

impl Emitter {
    pub(super) const fn new(json: bool) -> Self {
        Self {
            json,
            progress: None,
        }
    }

    pub(super) fn start_progress(&mut self, label: &'static str) -> Result<()> {
        if !progress_enabled(self.json, io::stderr().is_terminal()) {
            return Ok(());
        }
        let progress = progress_bar(label, ProgressDrawTarget::stderr())?;
        progress.enable_steady_tick(Duration::from_millis(100));
        self.progress = Some(progress);
        Ok(())
    }

    pub(super) fn set_progress(&self, label: &'static str) {
        if let Some(progress) = &self.progress {
            progress.set_message(label);
        }
    }

    pub(super) fn note(&self, text: &str) {
        if let Some(progress) = &self.progress {
            progress.println(text);
        } else {
            note(text);
        }
    }

    pub(super) fn finish_progress(&self) {
        if let Some(progress) = &self.progress {
            progress.finish_and_clear();
        }
    }

    pub(super) fn execute(&self, session: &mut Session, command: Command) -> Result<Outcome> {
        self.set_progress(command.progress_label());
        session.execute_with(
            command,
            &mut |event| self.set_progress(event.label()),
            &|| true,
        )
    }

    pub(super) fn emit<T: Serialize>(
        &self,
        value: &T,
        text: impl FnOnce() -> String,
    ) -> Result<()> {
        self.finish_progress();
        let mut stdout = io::stdout().lock();
        if self.json {
            writeln!(stdout, "{}", serde_json::to_string_pretty(value)?)?;
        } else {
            write!(stdout, "{}", text())?;
        }
        stdout.flush()?;
        Ok(())
    }

    pub(super) fn message(&self, message: &str) -> Result<()> {
        self.emit(&Message { message }, || format!("{message}\n"))
    }
}

pub(super) const fn progress_enabled(json: bool, stderr_terminal: bool) -> bool {
    !json && stderr_terminal
}

pub(super) fn progress_bar(label: &'static str, target: ProgressDrawTarget) -> Result<ProgressBar> {
    let progress = ProgressBar::with_draw_target(None, target);
    progress.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")?.tick_strings(&["-", "\\", "|", "/"]),
    );
    progress.set_message(label);
    Ok(progress)
}

#[derive(Serialize)]
pub(super) struct Message<'a> {
    pub(super) message: &'a str,
}
