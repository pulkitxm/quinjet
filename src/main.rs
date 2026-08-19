mod app;
mod cli;
mod convert;
mod date_time;
mod file_icons;
mod git;
mod state;
mod theme;
mod ui;
mod watch;
mod webhook;
mod webhook_parser;

use std::io::{self, IsTerminal};
use std::process::ExitCode;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, tick};
use crossterm::clipboard::CopyToClipboard;
use crossterm::cursor::Show;
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::{App, AppEffect, ToastLevel};
use crate::cli::{Launch, TerminalOptions};
use crate::git::Repository;
use crate::git::worker::GitWorker;
use crate::watch::RepoWatcher;
use crate::webhook::WebhookListener;

fn main() -> ExitCode {
    match cli::dispatch() {
        Ok(Launch::Terminal(options)) => match open_terminal(&options) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => ExitCode::from(cli::report(&error)),
        },
        Ok(Launch::Finished(code)) => ExitCode::from(code),
        Err(error) => ExitCode::from(cli::report(&error)),
    }
}

static TERMINAL_ENTERED: AtomicBool = AtomicBool::new(false);
static KEYBOARD_ENHANCED: AtomicBool = AtomicBool::new(false);
static TERMINAL_THREAD: OnceLock<thread::ThreadId> = OnceLock::new();

fn restore_terminal() {
    if !TERMINAL_ENTERED.swap(false, Ordering::SeqCst) {
        return;
    }
    drop(disable_raw_mode());
    let mut stdout = io::stdout();
    if KEYBOARD_ENHANCED.swap(false, Ordering::SeqCst) {
        drop(execute!(stdout, PopKeyboardEnhancementFlags));
    }
    drop(execute!(
        stdout,
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen,
        Show
    ));
}

fn install_panic_hook() {
    let current = thread::current().id();
    let owner = TERMINAL_THREAD.get_or_init(|| current);
    debug_assert_eq!(owner, &current, "terminal hook installed on another thread");
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if cfg!(panic = "abort")
            || TERMINAL_THREAD
                .get()
                .is_some_and(|owner| owner == &thread::current().id())
        {
            restore_terminal();
        }
        report(info);
    }));
}

fn open_terminal(options: &TerminalOptions) -> Result<()> {
    if !io::stdin().is_terminal() || !cli::stdout_is_terminal() {
        anyhow::bail!("Quinjet requires an interactive terminal");
    }

    install_panic_hook();
    let repository = Repository::discover(&options.path)?;
    state::record_recent_project(repository.root());
    let common_dir = repository.git_common_dir().ok();
    let mut worker = GitWorker::start(repository.clone());
    let mut watcher = RepoWatcher::with_extra(repository.root(), common_dir.as_deref()).ok();
    let webhooks = options
        .webhook_listen
        .as_deref()
        .map(WebhookListener::bind)
        .transpose()?;
    let mut app = App::new(repository.root(), repository.name());
    app.set_theme_selection(options.theme, options.appearance);
    let mut terminal = TerminalGuard::enter(!options.no_mouse)?;
    let render_tick = tick(Duration::from_millis(16));
    let periodic_refresh = tick(Duration::from_secs(10));
    let mut dirty = true;
    let mut running = true;

    app.configure_mouse_capture(!options.no_mouse);
    app.webhooks_listening = webhooks.is_some();
    let effects = app.initial_effects();
    running &= dispatch_effects(&mut worker, &mut watcher, &mut app, &mut terminal, effects);
    while running {
        if dirty {
            let theme = app.theme;
            let _ = terminal
                .terminal
                .draw(|frame| ui::draw(frame, &mut app, &theme))
                .context("failed to render Quinjet")?;
            dirty = false;
        }

        while let Ok(worker_event) = worker.events().try_recv() {
            let effects = app.handle_worker_event(worker_event, Instant::now());
            running &=
                dispatch_effects(&mut worker, &mut watcher, &mut app, &mut terminal, effects);
            dirty = true;
        }

        if watcher_changed(watcher.as_ref().map(RepoWatcher::changes)) {
            let mut effects = Vec::new();
            app.filesystem_changed(&mut effects);
            running &=
                dispatch_effects(&mut worker, &mut watcher, &mut app, &mut terminal, effects);
            dirty = true;
        }

        if webhook_delivered(webhooks.as_ref()) {
            let effects = app.webhook_delivered(Instant::now());
            running &=
                dispatch_effects(&mut worker, &mut watcher, &mut app, &mut terminal, effects);
            dirty = true;
        }

        if render_tick.try_recv().is_ok() {
            let (effects, changed) = app.tick(Instant::now());
            running &=
                dispatch_effects(&mut worker, &mut watcher, &mut app, &mut terminal, effects);
            dirty |= changed;
        }
        if periodic_refresh.try_recv().is_ok() {
            let mut effects = Vec::new();
            app.periodic_refresh(&mut effects);
            running &=
                dispatch_effects(&mut worker, &mut watcher, &mut app, &mut terminal, effects);
            dirty = true;
        }

        if event::poll(Duration::from_millis(8)).context("failed to poll terminal events")? {
            let effects = match event::read().context("failed to read terminal event")? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    app.handle_key(key, Instant::now())
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse, Instant::now()),
                Event::Paste(text) => {
                    app.handle_paste(&text);
                    Vec::new()
                }
                _ => Vec::new(),
            };
            running &=
                dispatch_effects(&mut worker, &mut watcher, &mut app, &mut terminal, effects);
            dirty = true;
        }
    }

    Ok(())
}

fn watcher_changed(receiver: Option<&Receiver<()>>) -> bool {
    let Some(receiver) = receiver else {
        return false;
    };
    let mut changed = false;
    while receiver.try_recv().is_ok() {
        changed = true;
    }
    changed
}

/// Deliveries only say that something changed, so several arriving together
/// collapse into the single refresh they would each have asked for.
fn webhook_delivered(listener: Option<&WebhookListener>) -> bool {
    let Some(listener) = listener else {
        return false;
    };
    let mut delivered = false;
    while listener.deliveries().try_recv().is_ok() {
        delivered = true;
    }
    delivered
}

fn dispatch_effects(
    worker: &mut GitWorker,
    watcher: &mut Option<RepoWatcher>,
    app: &mut App,
    terminal: &mut TerminalGuard,
    effects: Vec<AppEffect>,
) -> bool {
    let mut running = true;
    for effect in effects {
        match effect {
            AppEffect::Git(command) => {
                running &= worker.send(*command);
            }
            AppEffect::Copy(text) => terminal.copy_to_clipboard(&text),
            AppEffect::SetMouseCapture(enabled) => terminal.set_mouse_capture(enabled),
            AppEffect::Open(app::OpenTarget::Browser(url)) => drop(cli::open_url(&url)),
            AppEffect::OpenRepository(path) => {
                running &= bind_repository(worker, watcher, app, terminal, &path);
            }
            AppEffect::Quit => running = false,
        }
    }
    running
}

fn bind_repository(
    worker: &mut GitWorker,
    watcher: &mut Option<RepoWatcher>,
    app: &mut App,
    terminal: &mut TerminalGuard,
    path: &std::path::Path,
) -> bool {
    let repository = match Repository::discover(path) {
        Ok(repository) => repository,
        Err(error) => {
            app.show_toast(error.to_string(), ToastLevel::Error, Instant::now());
            return true;
        }
    };
    if repository.root() == app.repository_root {
        return true;
    }
    state::record_recent_project(repository.root());
    let theme_name = app.theme_name;
    let appearance_choice = app.appearance_choice;
    let mouse = app.mouse_capture_preference;
    let webhooks_listening = app.webhooks_listening;
    let common_dir = repository.git_common_dir().ok();
    *worker = GitWorker::start(repository.clone());
    *watcher = RepoWatcher::with_extra(repository.root(), common_dir.as_deref()).ok();
    *app = App::new(repository.root(), repository.name());
    app.set_theme_selection(theme_name, appearance_choice);
    app.configure_mouse_capture(mouse);
    app.webhooks_listening = webhooks_listening;
    let effects = app.initial_effects();
    dispatch_effects(worker, watcher, app, terminal, effects)
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    mouse: bool,
}

struct TerminalRollback {
    armed: bool,
}

impl Drop for TerminalRollback {
    fn drop(&mut self) {
        if self.armed {
            restore_terminal();
        }
    }
}

impl TerminalGuard {
    fn copy_to_clipboard(&mut self, text: &str) {
        drop(execute!(
            self.terminal.backend_mut(),
            CopyToClipboard::to_clipboard_from(text)
        ));
    }

    fn set_mouse_capture(&mut self, enabled: bool) {
        if self.mouse == enabled {
            return;
        }
        let backend = self.terminal.backend_mut();
        let applied = if enabled {
            execute!(backend, EnableMouseCapture)
        } else {
            execute!(backend, DisableMouseCapture)
        };
        if applied.is_ok() {
            self.mouse = enabled;
        }
    }

    fn enter(mouse: bool) -> Result<Self> {
        enable_raw_mode().context("failed to enable terminal raw mode")?;
        TERMINAL_ENTERED.store(true, Ordering::SeqCst);
        let mut rollback = TerminalRollback { armed: true };
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)
            .context("failed to enter alternate screen")?;
        if mouse {
            execute!(stdout, EnableMouseCapture).context("failed to enable mouse capture")?;
        }
        if crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false)
            && execute!(
                stdout,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                )
            )
            .is_ok()
        {
            KEYBOARD_ENHANCED.store(true, Ordering::SeqCst);
        }
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("failed to initialize terminal")?;
        terminal.clear().context("failed to clear terminal")?;
        rollback.armed = false;
        Ok(Self { terminal, mouse })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}
