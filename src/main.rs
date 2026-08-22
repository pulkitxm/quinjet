mod app;
mod cli;
mod convert;
mod date_time;
mod file_icons;
mod git;
mod onboarding;
mod state;
mod tabs;
mod theme;
mod ui;
mod watch;
mod webhook;
mod webhook_parser;
mod workspace;

use std::collections::VecDeque;
use std::io::{self, IsTerminal};
use std::process::ExitCode;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::tick;
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

use crate::app::AppEffect;
use crate::cli::{Launch, TerminalOptions};
use crate::git::Repository;
use crate::onboarding::{Onboarding, OnboardingAction};
use crate::webhook::WebhookListener;
use crate::workspace::{RepositoryWorkspace, RoutedEffects};

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

#[expect(
    clippy::too_many_lines,
    reason = "the terminal loop routes both onboarding and repository sessions"
)]
fn open_terminal(options: &TerminalOptions) -> Result<()> {
    if !io::stdin().is_terminal() || !cli::stdout_is_terminal() {
        anyhow::bail!("Quinjet requires an interactive terminal");
    }

    install_panic_hook();
    let webhooks = options
        .webhook_listen
        .as_deref()
        .map(WebhookListener::bind)
        .transpose()?;
    let repository = Repository::discover(&options.path).ok();
    if let Some(repository) = repository.as_ref() {
        state::record_recent_project(repository.root());
    }
    let mut workspace = repository.as_ref().map(|repository| {
        let mut workspace = RepositoryWorkspace::new(
            repository,
            options.theme,
            options.appearance,
            !options.no_mouse,
            webhooks.is_some(),
        );
        workspace.sync_tabs(Instant::now());
        workspace
    });
    let mut onboarding = Onboarding::new(&options.path);
    let onboarding_theme = theme::Theme::new(options.theme, options.appearance.resolve());
    let mut terminal = TerminalGuard::enter(!options.no_mouse)?;
    let render_tick = tick(Duration::from_millis(16));
    let relative_time_tick = tick(Duration::from_secs(1));
    let periodic_refresh = tick(Duration::from_secs(10));
    let mut dirty = true;
    let mut running = true;

    if let Some(current) = workspace.as_mut() {
        running &= dispatch_launch_effects(current, &mut terminal, options.pull_request);
    }
    while running {
        if dirty {
            if let Some(current) = workspace.as_mut() {
                let Some(app) = current.active_app_mut() else {
                    break;
                };
                let theme = app.theme;
                let _ = terminal
                    .terminal
                    .draw(|frame| ui::draw(frame, app, &theme))
                    .context("failed to render Quinjet")?;
            } else {
                let _ = terminal
                    .terminal
                    .draw(|frame| onboarding.draw(frame, &onboarding_theme))
                    .context("failed to render Quinjet onboarding")?;
            }
            dirty = false;
        }

        if let Some(current) = workspace.as_mut() {
            let worker_effects = current.drain_worker_events(Instant::now());
            if !worker_effects.is_empty() {
                running &= dispatch_effects(current, &mut terminal, worker_effects);
                dirty = true;
            }

            let watcher_effects = current.poll_watchers();
            if !watcher_effects.is_empty() {
                running &= dispatch_effects(current, &mut terminal, watcher_effects);
                dirty = true;
            }

            if webhook_delivered(webhooks.as_ref()) {
                let effects = current.webhook_delivered(Instant::now());
                running &= dispatch_effects(current, &mut terminal, effects);
                dirty = true;
            }

            if render_tick.try_recv().is_ok() {
                let (effects, changed) = current.tick(Instant::now());
                running &= dispatch_effects(current, &mut terminal, effects);
                dirty |= changed;
            }
            if relative_time_tick.try_recv().is_ok() {
                dirty = true;
            }
            if periodic_refresh.try_recv().is_ok() {
                let effects = current.periodic_refresh();
                running &= dispatch_effects(current, &mut terminal, effects);
                dirty = true;
            }
        }

        if event::poll(Duration::from_millis(8)).context("failed to poll terminal events")? {
            let event = event::read().context("failed to read terminal event")?;
            if let Some(current) = workspace.as_mut() {
                let Some(id) = current.active_id() else {
                    break;
                };
                let Some(app) = current.app_mut(id) else {
                    break;
                };
                let effects = match event {
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
                current.propagate_preferences(id);
                running &=
                    dispatch_effects(current, &mut terminal, [RoutedEffects { id, effects }]);
            } else {
                let action = match event {
                    Event::Key(key) if key.kind != KeyEventKind::Release => {
                        onboarding.handle_key(key)
                    }
                    Event::Paste(text) => {
                        onboarding.handle_paste(&text);
                        OnboardingAction::None
                    }
                    _ => OnboardingAction::None,
                };
                match action {
                    OnboardingAction::None => {}
                    OnboardingAction::Quit => running = false,
                    OnboardingAction::Open(path) => match Repository::discover(&path) {
                        Ok(repository) => {
                            state::record_recent_project(repository.root());
                            let mut next = RepositoryWorkspace::new(
                                &repository,
                                options.theme,
                                options.appearance,
                                !options.no_mouse,
                                webhooks.is_some(),
                            );
                            next.sync_tabs(Instant::now());
                            running &= dispatch_launch_effects(
                                &mut next,
                                &mut terminal,
                                options.pull_request,
                            );
                            workspace = Some(next);
                        }
                        Err(error) => onboarding.show_error(error.to_string()),
                    },
                }
            }
            dirty = true;
        }
    }

    Ok(())
}

fn dispatch_launch_effects(
    workspace: &mut RepositoryWorkspace,
    terminal: &mut TerminalGuard,
    pull_request: Option<u64>,
) -> bool {
    let mut running = true;
    if let Some(effects) = workspace.initial_effects() {
        running &= dispatch_effects(workspace, terminal, [effects]);
    }
    if let Some(number) = pull_request
        && let Some(effects) = workspace.open_pull_request_on_launch(number)
    {
        running &= dispatch_effects(workspace, terminal, [effects]);
    }
    running
}

#[doc = " Deliveries only say that something changed, so several arriving together"]
#[doc = " collapse into the single refresh they would each have asked for."]
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
    workspace: &mut RepositoryWorkspace,
    terminal: &mut TerminalGuard,
    routed: impl IntoIterator<Item = RoutedEffects>,
) -> bool {
    let mut running = true;
    let mut pending = routed.into_iter().collect::<VecDeque<_>>();
    while let Some(RoutedEffects { id, effects }) = pending.pop_front() {
        for effect in effects {
            match effect {
                AppEffect::Git(command) => {
                    running &= workspace.send(id, *command);
                }
                AppEffect::Copy(text) => terminal.copy_to_clipboard(&text),
                AppEffect::SetMouseCapture(enabled) => terminal.set_mouse_capture(enabled),
                AppEffect::Open(app::OpenTarget::Browser(url)) => drop(cli::open_url(&url)),
                AppEffect::SwitchRepository(path) => {
                    if let Some(effects) = workspace.switch_repository(id, &path, Instant::now()) {
                        pending.push_back(effects);
                    }
                }
                AppEffect::OpenRepositoryTab(path) => {
                    if let Some(effects) = workspace.open_repository_tab(id, &path, Instant::now())
                    {
                        pending.push_back(effects);
                    }
                }
                AppEffect::ActivateRepositoryTab(target) => {
                    workspace.activate(target, Instant::now());
                }
                AppEffect::ReorderRepositoryTab { source, target } => {
                    workspace.reorder(source, target, Instant::now());
                }
                AppEffect::CloseRepositoryTab(target) => {
                    running &= workspace.close(target, Instant::now());
                }
                AppEffect::CloseOtherRepositoryTabs(target) => {
                    workspace.close_others(target, Instant::now());
                }
                AppEffect::CloseAllRepositoryTabs => {
                    workspace.close_all();
                    running = false;
                }
                AppEffect::Quit => running = false,
            }
        }
    }
    running
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
