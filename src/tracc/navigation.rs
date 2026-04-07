use super::edit::EditState;
use super::input::{ConfirmState, InputState};
use super::Tracc;
use crate::timesheet::TimeSheet;

pub(crate) enum PendingAction {
    BeginEdit(EditState),
    ShiftCurrent(i64),
    RemoveCurrent,
    Paste,
}

impl Tracc {
    pub(crate) fn previous_day(&mut self) -> Result<(), std::io::Error> {
        if let Some(date) = self.times.date.previous_day() {
            self.load_day(date)?;
        }
        Ok(())
    }

    pub(crate) fn next_day(&mut self) -> Result<(), std::io::Error> {
        let today = TimeSheet::current_date();
        if let Some(date) = self.times.date.next_day() {
            if date > today {
                return Ok(());
            }
            self.load_day(date)?;
        }
        Ok(())
    }

    pub(crate) fn goto_today(&mut self) -> Result<(), std::io::Error> {
        self.load_day(TimeSheet::current_date())
    }

    pub(crate) fn load_day(&mut self, date: time::Date) -> Result<(), std::io::Error> {
        self.times = TimeSheet::open(date);
        self.input_state = InputState::Normal;
        self.undo_history.clear();
        self.redo_history.clear();
        self.sheet_locked = !self.times.is_today();
        self.terminal.hide_cursor()
    }

    pub(crate) fn begin_day_load(&mut self) -> Result<InputState, std::io::Error> {
        let selected = self.times.selected;
        self.begin_edit(EditState::date(selected, self.times.date))
    }

    pub(crate) fn begin_new_item(&mut self) -> Result<InputState, std::io::Error> {
        let index = self.times.insertion_index_for_now();
        let time = self.times.current_minutes_since_start();
        let edit = EditState::new_at(index, time);

        if time > super::MAX_NEW_ITEM_MINUTES {
            Ok(InputState::Confirm(ConfirmState {
                message: "The current time for this sheet is beyond 48 hours. Continue anyway?"
                    .to_string(),
                action: PendingAction::BeginEdit(edit),
                selected: crate::confirm::ConfirmChoice::No,
            }))
        } else {
            self.guard_mutation(
                PendingAction::BeginEdit(edit),
                self.timesheet_change_message(),
            )
        }
    }

    pub(crate) fn execute_action(
        &mut self,
        action: PendingAction,
    ) -> Result<InputState, std::io::Error> {
        match action {
            PendingAction::BeginEdit(edit) => self.begin_edit(edit),
            PendingAction::ShiftCurrent(minutes) => {
                if self.times.selected_index().is_some() {
                    self.record_change_snapshot();
                    self.times.shift_current(minutes);
                    self.persist_state();
                }
                Ok(InputState::Normal)
            }
            PendingAction::RemoveCurrent => {
                if self.times.selected_index().is_some() {
                    self.record_change_snapshot();
                    self.times.remove_current();
                    self.persist_state();
                }
                Ok(InputState::Normal)
            }
            PendingAction::Paste => {
                if self.times.can_paste() {
                    self.record_change_snapshot();
                    self.times.paste();
                    self.persist_state();
                }
                Ok(InputState::Normal)
            }
        }
    }

    pub(crate) fn timesheet_change_message(&self) -> String {
        format!("Change timesheet data for {}?", self.times.date_label())
    }
}
