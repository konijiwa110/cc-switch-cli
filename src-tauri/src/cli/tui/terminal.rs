use std::io::{self, Stdout};
use std::sync::Arc;

use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
#[cfg(test)]
use ratatui::backend::TestBackend;
use ratatui::prelude::Size;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::error::AppError;

type LiveTerminal = Terminal<CrosstermBackend<Stdout>>;

type PanicHook = Arc<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync + 'static>;

#[cfg(test)]
type InMemoryTerminal = Terminal<TestBackend>;

enum TerminalHandle {
    Live(LiveTerminal),
    #[cfg(test)]
    Test(InMemoryTerminal),
}

pub struct TuiTerminal {
    terminal: TerminalHandle,
    active: bool,
}

pub struct PanicRestoreHookGuard {
    previous: Option<PanicHook>,
}

impl PanicRestoreHookGuard {
    pub fn install() -> Self {
        let previous = std::panic::take_hook();
        let previous: PanicHook = previous.into();
        let previous_for_hook = previous.clone();

        std::panic::set_hook(Box::new(move |info| {
            let mut stdout = io::stdout();
            let _ = restore_stdout_best_effort(&mut stdout);
            previous_for_hook(info);
        }));

        Self {
            previous: Some(previous),
        }
    }
}

impl Drop for PanicRestoreHookGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::panic::set_hook(Box::new(move |info| previous(info)));
        }
    }
}

fn record_err(first_err: &mut Option<AppError>, e: impl ToString) {
    if first_err.is_none() {
        *first_err = Some(AppError::localized(
            "tui_terminal_error",
            format!("终端错误: {}", e.to_string()),
            format!("Terminal error: {}", e.to_string()),
        ));
    }
}

fn restore_stdout_best_effort(stdout: &mut Stdout) -> Result<(), AppError> {
    let mut first_err: Option<AppError> = None;

    if let Err(e) = disable_raw_mode() {
        record_err(&mut first_err, e);
    }

    if let Err(e) = execute!(
        stdout,
        cursor::Show,
        LeaveAlternateScreen,
        DisableMouseCapture
    ) {
        record_err(&mut first_err, e);
    }

    if let Some(err) = first_err {
        Err(err)
    } else {
        Ok(())
    }
}

fn terminal_error(e: impl ToString) -> AppError {
    AppError::localized(
        "tui_terminal_error",
        format!("终端错误: {}", e.to_string()),
        format!("Terminal error: {}", e.to_string()),
    )
}

impl TuiTerminal {
    #[cfg(test)]
    fn live_terminal_mut(&mut self) -> Option<&mut LiveTerminal> {
        match &mut self.terminal {
            TerminalHandle::Live(terminal) => Some(terminal),
            TerminalHandle::Test(_) => None,
        }
    }

    #[cfg(not(test))]
    fn live_terminal_mut(&mut self) -> Option<&mut LiveTerminal> {
        let TerminalHandle::Live(terminal) = &mut self.terminal;
        Some(terminal)
    }

    pub fn new() -> Result<Self, AppError> {
        let mut stdout = io::stdout();
        enable_raw_mode().map_err(terminal_error)?;
        if let Err(e) = execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        ) {
            let _ = restore_stdout_best_effort(&mut stdout);
            return Err(terminal_error(e));
        }

        let terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => terminal,
            Err(e) => {
                let mut stdout = io::stdout();
                let _ = restore_stdout_best_effort(&mut stdout);
                return Err(terminal_error(e));
            }
        };

        Ok(Self {
            terminal: TerminalHandle::Live(terminal),
            active: true,
        })
    }

    #[cfg(test)]
    pub(crate) fn new_for_test() -> Result<Self, AppError> {
        const TEST_TERMINAL_WIDTH: u16 = 120;
        const TEST_TERMINAL_HEIGHT: u16 = 40;

        let terminal = Terminal::new(TestBackend::new(TEST_TERMINAL_WIDTH, TEST_TERMINAL_HEIGHT))
            .map_err(terminal_error)?;

        Ok(Self {
            terminal: TerminalHandle::Test(terminal),
            active: false,
        })
    }

    pub fn draw<F>(&mut self, f: F) -> Result<(), AppError>
    where
        F: FnOnce(&mut ratatui::Frame<'_>),
    {
        match &mut self.terminal {
            TerminalHandle::Live(terminal) => terminal.draw(f).map(|_| ()).map_err(terminal_error),
            #[cfg(test)]
            TerminalHandle::Test(terminal) => terminal.draw(f).map(|_| ()).map_err(terminal_error),
        }
    }

    pub fn size(&self) -> Result<Size, AppError> {
        match &self.terminal {
            TerminalHandle::Live(terminal) => terminal.size().map_err(terminal_error),
            #[cfg(test)]
            TerminalHandle::Test(terminal) => terminal.size().map_err(terminal_error),
        }
    }

    pub fn with_terminal_restored<T>(
        &mut self,
        f: impl FnOnce() -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        self.restore_best_effort()?;

        struct ReactivateOnDrop<'a> {
            terminal: &'a mut TuiTerminal,
            reactivated: bool,
        }

        impl Drop for ReactivateOnDrop<'_> {
            fn drop(&mut self) {
                if self.reactivated {
                    return;
                }
                let _ = self.terminal.activate_best_effort();
            }
        }

        let mut guard = ReactivateOnDrop {
            terminal: self,
            reactivated: false,
        };

        let result = f();
        let activate_result = guard.terminal.activate_best_effort();
        if activate_result.is_ok() {
            guard.reactivated = true;
        }
        activate_result?;

        result
    }

    pub fn restore_best_effort(&mut self) -> Result<(), AppError> {
        if !self.active {
            return Ok(());
        }

        let Some(terminal) = self.live_terminal_mut() else {
            self.active = false;
            return Ok(());
        };

        let mut first_err: Option<AppError> = None;

        if let Err(e) = disable_raw_mode() {
            record_err(&mut first_err, e);
        }

        if let Err(e) = execute!(
            terminal.backend_mut(),
            cursor::Show,
            LeaveAlternateScreen,
            DisableMouseCapture
        ) {
            record_err(&mut first_err, e);
        }
        let _ = terminal.show_cursor();

        if let Some(err) = first_err {
            Err(err)
        } else {
            self.active = false;
            Ok(())
        }
    }

    pub fn activate_best_effort(&mut self) -> Result<(), AppError> {
        if self.active {
            return Ok(());
        }

        let Some(terminal) = self.live_terminal_mut() else {
            return Ok(());
        };

        let mut first_err: Option<AppError> = None;

        if let Err(e) = enable_raw_mode() {
            record_err(&mut first_err, e);
        }

        if let Err(e) = execute!(
            terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        ) {
            record_err(&mut first_err, e);
        }

        if let Err(e) = terminal.clear() {
            record_err(&mut first_err, e);
        }

        if let Some(err) = first_err {
            Err(err)
        } else {
            self.active = true;
            Ok(())
        }
    }
}

impl Drop for TuiTerminal {
    fn drop(&mut self) {
        let _ = self.restore_best_effort();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};
    use std::thread;

    use super::TuiTerminal;

    #[test]
    fn new_for_test_supports_parallel_construction_without_touching_real_tty() {
        let barrier = Arc::new(Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    for _ in 0..32 {
                        let mut terminal = TuiTerminal::new_for_test().expect("create terminal");
                        let size = terminal.size().expect("read terminal size");
                        assert!(size.width > 0);
                        assert!(size.height > 0);
                        terminal.draw(|_| {}).expect("draw frame");
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread should complete without panic");
        }
    }

    #[test]
    fn test_terminal_restore_and_reactivate_are_safe_noops() {
        let mut terminal = TuiTerminal::new_for_test().expect("create terminal");

        terminal
            .restore_best_effort()
            .expect("restore test terminal");
        terminal
            .activate_best_effort()
            .expect("reactivate test terminal");
        terminal.draw(|_| {}).expect("draw after lifecycle noops");
    }

    #[test]
    fn with_terminal_restored_keeps_test_terminal_usable() {
        let mut terminal = TuiTerminal::new_for_test().expect("create terminal");

        let value = terminal
            .with_terminal_restored(|| Ok::<_, crate::error::AppError>(42))
            .expect("run callback with terminal restored");

        assert_eq!(value, 42);
        terminal
            .draw(|_| {})
            .expect("draw after with_terminal_restored");
    }
}
