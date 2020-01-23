use std::io;
use termion::event::Key;
use termion::raw::IntoRawMode;
use termion::input::TermRead;
use tui::backend::Backend;
use tui::backend::TermionBackend;
use tui::style::{Color, Style};
use tui::widgets::*;
use tui::Terminal;
mod tracc;
use tracc::Tracc;

pub enum Mode {
    Insert,
    Normal,
}

fn main() -> Result<(), io::Error> {
    let stdout = io::stdout().into_raw_mode()?;
    let mut inputs = io::stdin().keys();
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut tracc = Tracc::open_or_create();
    terminal.hide_cursor()?;
    terminal.clear()?;
    loop {
        refresh(&mut terminal, &tracc)?;
        // I need to find a better way to handle inputs. This is awful.
        let input = inputs.next().unwrap().expect("input ded?");
        match tracc.mode {
            Mode::Normal => match input {
                Key::Char('q') => {
                    tracc.persist();
                    break;
                },
                Key::Char('j') => tracc.selection_down(),
                Key::Char('k') => tracc.selection_up(),
                Key::Char('o') => {
                    tracc.insert();
                    tracc.set_mode(Mode::Insert, &mut terminal)?;
                }
                Key::Char('a') => tracc.set_mode(Mode::Insert, &mut terminal)?,
                Key::Char('A') => tracc.set_mode(Mode::Insert, &mut terminal)?,
                Key::Char(' ') => tracc.toggle_current(),
                // dd
                Key::Char('d') => {
                    if let Key::Char('d') = inputs.next().unwrap().unwrap() {
                        tracc.remove_current()
                    }
                }
                _ => (),
            },
            Mode::Insert => match input {
                Key::Char('\n') => tracc.set_mode(Mode::Normal, &mut terminal)?,
                Key::Esc => tracc.set_mode(Mode::Normal, &mut terminal)?,
                Key::Backspace => tracc.current_pop(),
                Key::Char(x) => tracc.append_to_current(x),
                _ => (),
            },
        };
    }
    Ok(())
}

fn refresh(terminal: &mut tui::Terminal<impl Backend>, tracc: &Tracc) -> Result<(), io::Error> {
    terminal.draw(|mut frame| {
        let size = frame.size();
        let block = Block::default().title(" t r a c c ").borders(Borders::ALL);
        SelectableList::default()
            .block(block)
            .items(&tracc.printable_todos())
            .select(tracc.selected)
            .highlight_style(Style::default().fg(Color::LightGreen))
            .highlight_symbol(">")
            .render(&mut frame, size);
    })?;
    Ok(())
}
