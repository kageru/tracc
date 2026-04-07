use super::Tracc;
use crate::timesheet;
use std::{fs, io::Write};

impl Tracc {
    pub(crate) fn persist_state(&self) {
        fn write(path: &std::path::Path, content: &str) {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).ok();
            }
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)
                .ok()
                .unwrap_or_else(|| {
                    panic!("Can’t save state to JSON. Dumping raw data:\n{}", content)
                })
                .write_all(content.as_bytes())
                .unwrap();
        }
        let times_ser = serde_json::to_string(&self.times.times).unwrap();
        write(&timesheet::storage_path_for(self.times.date), &times_ser);
    }

    pub(crate) fn record_change_snapshot(&mut self) {
        self.redo_history.clear();
        self.push_history();
    }

    fn push_history(&mut self) {
        if self.undo_history.len() == super::MAX_UNDO_SNAPSHOTS {
            self.undo_history.pop_front();
        }
        self.undo_history.push_back(self.times.clone());
    }

    pub(crate) fn undo_previous_edit(&mut self) -> Result<(), std::io::Error> {
        let Some(previous) = self.undo_history.pop_back() else {
            return Ok(());
        };

        let current = self.times.clone();
        self.redo_history.push_back(current);

        self.times = previous;
        self.input_state = super::input::InputState::Normal;
        self.sheet_locked = !self.times.is_today();
        self.persist_state();
        self.terminal.hide_cursor()
    }

    pub(crate) fn redo_previous_edit(&mut self) -> Result<(), std::io::Error> {
        let Some(next) = self.redo_history.pop_back() else {
            return Ok(());
        };

        let current = self.times.clone();
        self.undo_history.push_back(current);

        self.times = next;
        self.input_state = super::input::InputState::Normal;
        self.sheet_locked = !self.times.is_today();
        self.persist_state();
        self.terminal.hide_cursor()
    }
}
