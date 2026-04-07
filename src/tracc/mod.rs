mod edit;
mod history;
mod input;
mod navigation;
mod render;

use self::input::InputState;
use crate::timesheet::TimeSheet;
use crossterm::event;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use std::{collections::VecDeque, io};

pub(crate) type Terminal = ratatui::Terminal<CrosstermBackend<io::Stdout>>;

pub struct Tracc {
    times: TimeSheet,
    terminal: Terminal,
    input_state: InputState,
    frame_area: Rect,
    list_area: Rect,
    sheet_locked: bool,
    undo_history: VecDeque<TimeSheet>,
    redo_history: VecDeque<TimeSheet>,
}

const MAX_NEW_ITEM_MINUTES: i64 = 48 * 60;
const MAX_UNDO_SNAPSHOTS: usize = 20;

impl Tracc {
    pub fn new(terminal: Terminal) -> Self {
        let date = TimeSheet::current_date();
        let times = TimeSheet::open(date);
        Self {
            sheet_locked: !times.is_today(),
            times,
            terminal,
            input_state: InputState::Normal,
            frame_area: Rect::default(),
            list_area: Rect::default(),
            undo_history: VecDeque::new(),
            redo_history: VecDeque::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), io::Error> {
        loop {
            self.refresh()?;
            let input = event::read()?;
            self.handle_input(input)?;
            if matches!(self.input_state, InputState::Quit) {
                break;
            }
        }
        self.terminal.clear()?;
        Ok(())
    }
}
