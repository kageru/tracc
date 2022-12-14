#![forbid(unsafe_code)]
use std::io;
use termion::raw::IntoRawMode;
use tui::backend::TermionBackend;
use tui::Terminal;
mod layout;
mod listview;
mod timesheet;
mod todolist;
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
