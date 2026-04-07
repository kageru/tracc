#![forbid(unsafe_code)]
use ratatui::backend::TermionBackend;
use ratatui::Terminal;
use std::io;
use termion::raw::IntoRawMode;
mod layout;
mod timesheet;
mod tracc;
use tracc::Tracc;
#[macro_use]
extern crate lazy_static;

fn main() -> Result<(), io::Error> {
    let stdout = io::stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;
    terminal.clear()?;
    let mut tracc = Tracc::new(terminal);
    tracc.run()
}
