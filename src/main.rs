use std::io;
use termion::event::Key;
use termion::raw::IntoRawMode;
use tui::backend::Backend;
use tui::backend::TermionBackend;
use tui::style::{Color, Style};
use tui::widgets::*;
use tui::Terminal;
mod events;
use events::{Event, Events};
mod tracc;
use tracc::Tracc;

pub enum Mode {
    Insert,
    Normal,
}

fn main() -> Result<(), io::Error> {
    let stdout = io::stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut tracc = Tracc::new();
    terminal.hide_cursor()?;
    terminal.clear()?;
    let events = Events::new();
    loop {
        refresh(&mut terminal, &tracc)?;
        // I need to find a better way to handle inputs. This is awful.
        match events.next().expect("input ded?") {
            Event::Input(input) => match tracc.mode {
                Mode::Normal => match input {
                    Key::Char('q') => break,
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
                        if let Event::Input(Key::Char('d')) = events.next().unwrap() {
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
            },
        }
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
