use super::edit::{EditOutcome, EditState};
use super::navigation::PendingAction;
use super::Tracc;
use crate::confirm::ConfirmChoice;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::io;

pub(crate) enum InputState {
    Normal,
    Editing(EditState),
    Confirm(ConfirmState),
    Prefix(PrefixState),
    Quit,
}

#[derive(Copy, Clone)]
pub(crate) enum PrefixState {
    G,
    Y,
}

pub(crate) struct ConfirmState {
    pub(crate) message: String,
    pub(crate) action: PendingAction,
    pub(crate) selected: ConfirmChoice,
}

impl Tracc {
    pub(crate) fn handle_input(&mut self, input: Event) -> Result<(), io::Error> {
        match input {
            Event::Key(input) => {
                let state = std::mem::replace(&mut self.input_state, InputState::Normal);
                self.input_state = match state {
                    InputState::Normal => self.handle_normal_input(input)?,
                    InputState::Editing(edit) => self.handle_edit_input(edit, input)?,
                    InputState::Confirm(confirm) => self.handle_confirm_input(confirm, input)?,
                    InputState::Prefix(prefix) => self.handle_prefix_input(prefix, input)?,
                    InputState::Quit => InputState::Quit,
                };
            }
            Event::Mouse(mouse) => self.handle_mouse_input(mouse)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_input(&mut self, mouse: MouseEvent) -> Result<(), io::Error> {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return Ok(());
        }

        match &mut self.input_state {
            InputState::Normal => {
                if let Some(index) = super::render::list_index_for_click(
                    self.list_area,
                    mouse.row,
                    self.times.times.len(),
                ) {
                    self.times.selected = index;
                }
            }
            InputState::Editing(edit) => {
                let popup_area = edit.popup_area(self.frame_area, self.list_area);
                if super::render::contains(popup_area, mouse.column, mouse.row) {
                    edit.cursor =
                        super::render::cursor_for_click(&edit.text, mouse.column, popup_area.x + 1);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_normal_input(&mut self, input: KeyEvent) -> Result<InputState, io::Error> {
        match input.code {
            KeyCode::Char('q') => Ok(InputState::Quit),
            KeyCode::Char('j') => {
                self.times.selection_down();
                Ok(InputState::Normal)
            }
            KeyCode::Char('k') => {
                self.times.selection_up();
                Ok(InputState::Normal)
            }
            KeyCode::Char('J') => {
                self.next_day()?;
                Ok(InputState::Normal)
            }
            KeyCode::Char('K') => {
                self.previous_day()?;
                Ok(InputState::Normal)
            }
            KeyCode::Char('G') => {
                self.times.selection_last();
                Ok(InputState::Normal)
            }
            KeyCode::Char('g') => Ok(InputState::Prefix(PrefixState::G)),
            KeyCode::Char('y') => Ok(InputState::Prefix(PrefixState::Y)),
            KeyCode::Char('o') => self.begin_new_item(),
            KeyCode::Char('a') => {
                let selected = self.times.selected;
                if let Some(text) = self.times.selected_text() {
                    self.guard_mutation(
                        PendingAction::BeginEdit(EditState::text(selected, text)),
                        self.timesheet_change_message(),
                    )
                } else {
                    Ok(InputState::Normal)
                }
            }
            KeyCode::Char('i') => {
                let selected = self.times.selected;
                if let Some(text) = self.times.selected_text() {
                    self.guard_mutation(
                        PendingAction::BeginEdit(EditState::text_at_start(selected, text)),
                        self.timesheet_change_message(),
                    )
                } else {
                    Ok(InputState::Normal)
                }
            }
            KeyCode::Char('A') => {
                let selected = self.times.selected;
                if let Some(time) = self.times.selected_time() {
                    self.guard_mutation(
                        PendingAction::BeginEdit(EditState::time(selected, time)),
                        self.timesheet_change_message(),
                    )
                } else {
                    Ok(InputState::Normal)
                }
            }
            KeyCode::Char('I') => {
                let selected = self.times.selected;
                if let Some(time) = self.times.selected_time() {
                    self.guard_mutation(
                        PendingAction::BeginEdit(EditState::time_at_start(selected, time)),
                        self.timesheet_change_message(),
                    )
                } else {
                    Ok(InputState::Normal)
                }
            }
            KeyCode::Char(' ') => Ok(InputState::Normal),
            KeyCode::Char('-') => self.guard_mutation(
                PendingAction::ShiftCurrent(-1),
                self.timesheet_change_message(),
            ),
            KeyCode::Char('+') => self.guard_mutation(
                PendingAction::ShiftCurrent(5),
                self.timesheet_change_message(),
            ),
            KeyCode::Char('d') => self.guard_mutation(
                PendingAction::RemoveCurrent,
                format!(
                    "Delete the current timesheet entry for {}?",
                    self.times.date_label()
                ),
            ),
            KeyCode::Char('p') => self.guard_mutation(
                PendingAction::Paste,
                format!(
                    "Paste into {} and change its timesheet data?",
                    self.times.date_label()
                ),
            ),
            KeyCode::Char('u') => {
                self.undo_previous_edit()?;
                Ok(InputState::Normal)
            }
            KeyCode::Char('r') if input.modifiers.contains(KeyModifiers::CONTROL) => {
                self.redo_previous_edit()?;
                Ok(InputState::Normal)
            }
            KeyCode::Char('r') => {
                let selected = self.times.selected;
                if self.times.selected_text().is_some() {
                    self.guard_mutation(
                        PendingAction::BeginEdit(EditState::text_empty(selected)),
                        self.timesheet_change_message(),
                    )
                } else {
                    Ok(InputState::Normal)
                }
            }
            KeyCode::Char('R') => {
                let selected = self.times.selected;
                if self.times.selected_time().is_some() {
                    self.guard_mutation(
                        PendingAction::BeginEdit(EditState::time_empty(selected)),
                        self.timesheet_change_message(),
                    )
                } else {
                    Ok(InputState::Normal)
                }
            }
            _ => Ok(InputState::Normal),
        }
    }

    fn handle_prefix_input(
        &mut self,
        prefix: PrefixState,
        input: KeyEvent,
    ) -> Result<InputState, io::Error> {
        match prefix {
            PrefixState::G => {
                match input.code {
                    KeyCode::Char('g') => self.times.selection_first(),
                    KeyCode::Char('d') => return self.begin_day_load(),
                    KeyCode::Char('t') => self.goto_today()?,
                    _ => {}
                }
                Ok(InputState::Normal)
            }
            PrefixState::Y => {
                if matches!(input.code, KeyCode::Char('y')) {
                    self.times.yank();
                }
                Ok(InputState::Normal)
            }
        }
    }

    fn handle_confirm_input(
        &mut self,
        mut confirm: ConfirmState,
        input: KeyEvent,
    ) -> Result<InputState, io::Error> {
        match input.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                confirm.selected = confirm.selected.toggle();
                Ok(InputState::Confirm(confirm))
            }
            KeyCode::Char('y') => Ok(self.accept_confirm(confirm)?),
            KeyCode::Char('n') | KeyCode::Esc => Ok(InputState::Normal),
            KeyCode::Enter | KeyCode::Char(' ') => match confirm.selected {
                ConfirmChoice::Yes => Ok(self.accept_confirm(confirm)?),
                ConfirmChoice::No => Ok(InputState::Normal),
            },
            _ => Ok(InputState::Confirm(confirm)),
        }
    }

    fn handle_edit_input(
        &mut self,
        mut edit: EditState,
        input: KeyEvent,
    ) -> Result<InputState, io::Error> {
        match edit.handle_key(input) {
            EditOutcome::Continue => Ok(InputState::Editing(edit)),
            EditOutcome::Cancel => {
                self.terminal.hide_cursor()?;
                Ok(InputState::Normal)
            }
            EditOutcome::Commit => match self.commit_edit(edit) {
                Ok(Some(edit)) => Ok(InputState::Editing(edit)),
                Ok(None) => {
                    self.terminal.hide_cursor()?;
                    Ok(InputState::Normal)
                }
                Err(err) => Err(err),
            },
        }
    }

    pub(crate) fn guard_mutation(
        &mut self,
        action: PendingAction,
        message: String,
    ) -> Result<InputState, io::Error> {
        if self.times.is_today() || !self.sheet_locked {
            self.execute_action(action)
        } else {
            Ok(InputState::Confirm(ConfirmState {
                message,
                action,
                selected: ConfirmChoice::Yes,
            }))
        }
    }

    fn accept_confirm(&mut self, confirm: ConfirmState) -> Result<InputState, io::Error> {
        self.sheet_locked = false;
        self.execute_action(confirm.action)
    }

    pub(crate) fn begin_edit(&mut self, edit: EditState) -> Result<InputState, io::Error> {
        self.terminal.show_cursor()?;
        Ok(InputState::Editing(edit))
    }
}
