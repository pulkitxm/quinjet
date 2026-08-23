use std::io::{self, IsTerminal};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use anyhow::{Context, Result};
use crossterm::clipboard::CopyToClipboard;
use crossterm::cursor::Show;
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::style::Print;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

pub(crate) const INHERITED_TERMINAL_ENV: &str = "QUINJET_INHERITED_TERMINAL";

static TERMINAL_ENTERED: AtomicBool = AtomicBool::new(false);
static HANDOFF_RAW_MODE: AtomicBool = AtomicBool::new(false);
static KEYBOARD_ENHANCED: AtomicBool = AtomicBool::new(false);
static TERMINAL_THREAD: OnceLock<thread::ThreadId> = OnceLock::new();

fn restore_terminal() {
    if HANDOFF_RAW_MODE.swap(false, Ordering::SeqCst) {
        drop(disable_raw_mode());
    }
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

pub(crate) fn restore_inherited_terminal() {
    if !io::stdout().is_terminal() {
        return;
    }
    drop(disable_raw_mode());
    let mut stdout = io::stdout();
    drop(execute!(stdout, PopKeyboardEnhancementFlags));
    drop(execute!(
        stdout,
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen,
        Show
    ));
}

pub(crate) fn install_panic_hook() {
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

pub(crate) struct TerminalGuard {
    pub(crate) terminal: Terminal<CrosstermBackend<io::Stdout>>,
    mouse: bool,
    restore: bool,
}

pub(crate) struct HandoffTerminalGuard {
    active: bool,
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
    pub(crate) fn send_host_action(&mut self, action: crate::integration::HostAction) {
        drop(execute!(
            self.terminal.backend_mut(),
            Print(action.sequence())
        ));
    }

    pub(crate) fn copy_to_clipboard(&mut self, text: &str) {
        drop(execute!(
            self.terminal.backend_mut(),
            CopyToClipboard::to_clipboard_from(text)
        ));
    }

    pub(crate) fn set_mouse_capture(&mut self, enabled: bool) {
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

    pub(crate) fn enter(mouse: bool) -> Result<Self> {
        if std::env::var_os(INHERITED_TERMINAL_ENV).is_some() {
            enable_raw_mode().context("failed to inherit terminal raw mode")?;
            TERMINAL_ENTERED.store(true, Ordering::SeqCst);
            KEYBOARD_ENHANCED.store(true, Ordering::SeqCst);
            let backend = CrosstermBackend::new(io::stdout());
            let terminal = Terminal::new(backend).context("failed to inherit terminal")?;
            return Ok(Self {
                terminal,
                mouse,
                restore: true,
            });
        }
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
        Ok(Self {
            terminal,
            mouse,
            restore: true,
        })
    }

    pub(crate) const fn preserve_for_handoff(&mut self) {
        self.restore = false;
    }
}

impl HandoffTerminalGuard {
    pub(crate) fn enter() -> Result<Self> {
        install_panic_hook();
        let active = io::stdin().is_terminal() && io::stdout().is_terminal();
        if active {
            enable_raw_mode()
                .context("failed to preserve terminal input during machine handoff")?;
            HANDOFF_RAW_MODE.store(true, Ordering::SeqCst);
        }
        Ok(Self { active })
    }
}

impl Drop for HandoffTerminalGuard {
    fn drop(&mut self) {
        if self.active && HANDOFF_RAW_MODE.swap(false, Ordering::SeqCst) {
            drop(disable_raw_mode());
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.restore {
            restore_terminal();
        }
    }
}
