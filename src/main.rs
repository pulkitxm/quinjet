mod app;
mod cli;
mod convert;
mod date_time;
mod file_icons;
mod git;
mod integration;
mod onboarding;
mod ssh;
mod state;
mod state_sorting;
mod tabs;
mod terminal;
mod theme;
mod ui;
mod watch;
mod webhook;
mod webhook_parser;
mod workspace;

use std::collections::VecDeque;
use std::io::{self, IsTerminal};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::tick;
use crossterm::event::{self, Event, KeyEventKind};

use crate::app::{AppEffect, Modal};
use crate::cli::{Launch, TerminalOptions};
use crate::git::Repository;
use crate::onboarding::{Onboarding, OnboardingAction};
use crate::ssh::SshContext;
use crate::terminal::TerminalGuard;
use crate::webhook::WebhookListener;
use crate::workspace::{RepositoryWorkspace, RoutedEffects};

fn main() -> ExitCode {
    match cli::dispatch() {
        Ok(Launch::Terminal(options)) => terminal_exit_code(&options),
        Ok(Launch::Finished(code)) => ExitCode::from(code),
        Err(error) => ExitCode::from(cli::report(&error)),
    }
}

fn terminal_exit_code(options: &TerminalOptions) -> ExitCode {
    let inherited_context = SshContext::from_environment();
    let local_session = inherited_context.is_none();
    let context = inherited_context.or_else(|| cli::local_ssh_context(&options.path));
    match open_terminal(options, context.as_ref()) {
        Ok(TerminalOutcome::Finished) => ExitCode::SUCCESS,
        Ok(TerminalOutcome::SwitchSshMachine {
            request,
            context: updated_context,
        }) if local_session => {
            let Some(mut context) = updated_context.or(context) else {
                return ExitCode::from(cli::EXIT_FAILURE);
            };
            let Some(machine) = context.machines.get(request.index).cloned() else {
                return ExitCode::from(cli::EXIT_FAILURE);
            };
            context.current.clone_from(&machine.target);
            match cli::run_selected_terminal(
                &machine.target,
                &machine.folder,
                context,
                request.mode,
            ) {
                Ok(code) => ExitCode::from(code),
                Err(error) => ExitCode::from(cli::report(&error)),
            }
        }
        Ok(TerminalOutcome::SwitchSshMachine { request, context }) => {
            if let Some(context) = context
                && let Err(error) = ssh::emit_handoff_context(&context)
            {
                return ExitCode::from(cli::report(&error));
            }
            ssh::switch_exit_code(request)
                .map_or_else(|| ExitCode::from(cli::EXIT_FAILURE), ExitCode::from)
        }
        Err(error) => ExitCode::from(cli::report(&error)),
    }
}

enum TerminalOutcome {
    Finished,
    SwitchSshMachine {
        request: ssh::SshSwitch,
        context: Option<SshContext>,
    },
}

#[expect(
    clippy::too_many_lines,
    reason = "the terminal loop routes both onboarding and repository sessions"
)]
fn open_terminal(
    options: &TerminalOptions,
    ssh_context: Option<&SshContext>,
) -> Result<TerminalOutcome> {
    if !io::stdin().is_terminal() || !cli::stdout_is_terminal() {
        anyhow::bail!("Quinjet requires an interactive terminal");
    }

    terminal::install_panic_hook();
    let webhooks = options
        .webhook_listen
        .as_deref()
        .map(WebhookListener::bind)
        .transpose()?;
    let handoff_mode = ssh::SshProjectOpenMode::from_environment();
    let repository = (!matches!(
        handoff_mode,
        Some(ssh::SshProjectOpenMode::Current | ssh::SshProjectOpenMode::Activate)
    ))
    .then(|| Repository::discover(&options.path).ok())
    .flatten();
    if let Some(repository) = repository.as_ref() {
        state::record_recent_project(repository.root());
    }
    let restored_workspace = handoff_mode.and_then(|_| {
        let session = state::session::load_project_session().unwrap_or_default();
        RepositoryWorkspace::restore(
            &session,
            options.theme,
            options.appearance,
            !options.no_mouse,
            webhooks.is_some(),
            ssh_context,
            options.client,
        )
    });
    let mut workspace = restored_workspace.or_else(|| {
        repository.as_ref().map(|repository| {
            let mut workspace = RepositoryWorkspace::new(
                repository,
                options.theme,
                options.appearance,
                !options.no_mouse,
                webhooks.is_some(),
                ssh_context.cloned(),
                options.client,
            );
            workspace.sync_tabs(Instant::now());
            workspace
        })
    });
    let onboarding_mode = if handoff_mode == Some(ssh::SshProjectOpenMode::New) {
        app::ProjectOpenMode::NewTab
    } else {
        app::ProjectOpenMode::Initial
    };
    let mut onboarding = Onboarding::new(&options.path, ssh_context.cloned(), onboarding_mode);
    let onboarding_theme = theme::Theme::new(options.theme, options.appearance.resolve());
    let mut terminal = TerminalGuard::enter(!options.no_mouse)?;
    let render_tick = tick(Duration::from_millis(16));
    let relative_time_tick = tick(Duration::from_secs(1));
    let periodic_refresh = tick(Duration::from_secs(10));
    let mut dirty = true;
    let mut running = true;
    let mut switch_ssh_machine = None;

    if let Some(current) = workspace.as_mut() {
        running &= dispatch_launch_effects(
            current,
            &mut terminal,
            options.pull_request,
            &mut switch_ssh_machine,
        );
        let project_mode = match handoff_mode {
            Some(ssh::SshProjectOpenMode::Current) => Some(app::ProjectOpenMode::CurrentTab),
            Some(ssh::SshProjectOpenMode::New) => Some(app::ProjectOpenMode::NewTab),
            Some(ssh::SshProjectOpenMode::Activate) | None => None,
        };
        if let Some(mode) = project_mode
            && let Some(effects) = current.open_projects_on_launch(mode)
        {
            running &= dispatch_effects(current, &mut terminal, [effects], &mut switch_ssh_machine);
        }
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
                running &= dispatch_effects(
                    current,
                    &mut terminal,
                    worker_effects,
                    &mut switch_ssh_machine,
                );
                dirty = true;
            }

            let watcher_effects = current.poll_watchers();
            if !watcher_effects.is_empty() {
                running &= dispatch_effects(
                    current,
                    &mut terminal,
                    watcher_effects,
                    &mut switch_ssh_machine,
                );
                dirty = true;
            }

            if webhook_delivered(webhooks.as_ref()) {
                let effects = current.webhook_delivered(Instant::now());
                running &=
                    dispatch_effects(current, &mut terminal, effects, &mut switch_ssh_machine);
                dirty = true;
            }

            if render_tick.try_recv().is_ok() {
                let (effects, changed) = current.tick(Instant::now());
                running &=
                    dispatch_effects(current, &mut terminal, effects, &mut switch_ssh_machine);
                dirty |= changed;
            }
            if periodic_refresh.try_recv().is_ok() {
                let effects = current.periodic_refresh();
                running &=
                    dispatch_effects(current, &mut terminal, effects, &mut switch_ssh_machine);
                dirty = true;
            }
        }
        if relative_time_tick.try_recv().is_ok() {
            dirty = true;
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
                running &= dispatch_effects(
                    current,
                    &mut terminal,
                    [RoutedEffects { id, effects }],
                    &mut switch_ssh_machine,
                );
            } else {
                let action = match event {
                    Event::Key(key) if key.kind != KeyEventKind::Release => {
                        onboarding.handle_key(key)
                    }
                    Event::Paste(text) => {
                        onboarding.handle_paste(&text);
                        OnboardingAction::None
                    }
                    Event::Mouse(mouse) => onboarding.handle_mouse(mouse),
                    _ => OnboardingAction::None,
                };
                match action {
                    OnboardingAction::None => {}
                    OnboardingAction::Quit => running = false,
                    OnboardingAction::SwitchSshMachine(index) => {
                        switch_ssh_machine = Some(ssh::SshSwitch {
                            index,
                            mode: ssh::SshProjectOpenMode::Current,
                        });
                        running = false;
                    }
                    OnboardingAction::Open(path) => match Repository::discover(&path) {
                        Ok(repository) => {
                            state::record_recent_project(repository.root());
                            let mut next = RepositoryWorkspace::new(
                                &repository,
                                options.theme,
                                options.appearance,
                                !options.no_mouse,
                                webhooks.is_some(),
                                ssh_context.cloned(),
                                options.client,
                            );
                            next.sync_tabs(Instant::now());
                            running &= dispatch_launch_effects(
                                &mut next,
                                &mut terminal,
                                options.pull_request,
                                &mut switch_ssh_machine,
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

    if switch_ssh_machine.is_some()
        && let Some(current) = workspace.as_ref()
    {
        state::session::record_project_session(current.project_session());
    }
    let outcome = switch_ssh_machine.map_or(TerminalOutcome::Finished, |request| {
        TerminalOutcome::SwitchSshMachine {
            request,
            context: workspace
                .as_ref()
                .and_then(RepositoryWorkspace::ssh_context)
                .or_else(|| ssh_context.cloned()),
        }
    });
    if matches!(outcome, TerminalOutcome::SwitchSshMachine { .. }) {
        terminal.preserve_for_handoff();
    }
    Ok(outcome)
}

fn dispatch_launch_effects(
    workspace: &mut RepositoryWorkspace,
    terminal: &mut TerminalGuard,
    pull_request: Option<u64>,
    switch_ssh_machine: &mut Option<ssh::SshSwitch>,
) -> bool {
    let mut running = true;
    let effects = workspace.initial_effects();
    running &= dispatch_effects(workspace, terminal, effects, switch_ssh_machine);
    if let Some(number) = pull_request
        && let Some(effects) = workspace.open_pull_request_on_launch(number)
    {
        running &= dispatch_effects(workspace, terminal, [effects], switch_ssh_machine);
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
    switch_ssh_machine: &mut Option<ssh::SshSwitch>,
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
                AppEffect::Host(action) => terminal.send_host_action(action),
                AppEffect::Open(app::OpenTarget::Browser(url)) => drop(cli::open_url(&url)),
                AppEffect::SwitchRepository(path) => {
                    render_project_opening(workspace, terminal, id);
                    if let Some(effects) = workspace.switch_repository(id, &path, Instant::now()) {
                        pending.push_back(effects);
                    }
                }
                AppEffect::OpenRepositoryTab(path) => {
                    render_project_opening(workspace, terminal, id);
                    if let Some(effects) = workspace.open_repository_tab(id, &path, Instant::now())
                    {
                        pending.push_back(effects);
                    }
                }
                AppEffect::SwitchSshMachine(request) => {
                    *switch_ssh_machine = Some(request);
                    running = false;
                }
                AppEffect::ActivateRepositoryTab(target) => {
                    if let Some(request) = workspace.activate(target, Instant::now()) {
                        *switch_ssh_machine = Some(request);
                        running = false;
                    }
                }
                AppEffect::ReorderRepositoryTab { source, target } => {
                    workspace.reorder(source, target, Instant::now());
                }
                AppEffect::CloseRepositoryTab(target) => {
                    let (keep_running, handoff) = workspace.close(target, Instant::now());
                    running &= keep_running;
                    if let Some(request) = handoff {
                        *switch_ssh_machine = Some(request);
                        running = false;
                    }
                }
                AppEffect::CloseOtherRepositoryTabs(target) => {
                    if let Some(request) = workspace.close_others(target, Instant::now()) {
                        *switch_ssh_machine = Some(request);
                        running = false;
                    }
                }
                AppEffect::CloseAllRepositoryTabs => {
                    if !workspace.exit_locked() {
                        workspace.close_all();
                        running = false;
                    }
                }
                AppEffect::Quit => {
                    if !workspace.exit_locked() {
                        running = false;
                    }
                }
            }
        }
    }
    running
}

fn render_project_opening(
    workspace: &mut RepositoryWorkspace,
    terminal: &mut TerminalGuard,
    id: tabs::TabId,
) {
    if workspace.active_id() != Some(id) {
        return;
    }
    let Some(app) = workspace.app_mut(id) else {
        return;
    };
    if !matches!(
        app.modal,
        Some(Modal::Projects {
            opening: Some(_),
            ..
        })
    ) {
        return;
    }
    let theme = app.theme;
    drop(terminal.terminal.draw(|frame| ui::draw(frame, app, &theme)));
}
