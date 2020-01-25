use serde::{Deserialize, Serialize};
use serde_json::from_reader;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use time::Time;

pub struct TimeSheet {
    pub times: Vec<TimePoint>,
    pub selected: usize,
    pub register: Option<TimePoint>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TimePoint {
    text: String,
    time: Time,
}

impl TimePoint {
    pub fn new(text: &str) -> Self {
        Self {
            text: String::from(text),
            time: Time::now(),
        }
    }
}

impl fmt::Display for TimePoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}] {}", self.time.format("%H:%M"), self.text)
    }
}

impl TimeSheet {
    pub fn new() -> Self {
        Self {
            times: vec![
                TimePoint::new("A test value"),
                TimePoint::new("A second test value"),
            ],
            selected: 0,
            register: None,
        }
    }

    pub fn printable(&self) -> Vec<String> {
        self.times.iter().map(TimePoint::to_string).collect()
    }
}
/*
impl TimeSheet {
    pub fn selection_down(&mut self) {
        self.selected = (self.selected + 1).min(self.todos.len().saturating_sub(1));
    }

    pub fn selection_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn insert(&mut self, todo: Todo) {
        if self.selected == self.todos.len().saturating_sub(1) {
            self.todos.push(todo);
            self.selected = self.todos.len() - 1;
        } else {
            self.todos.insert(self.selected + 1, todo);
            self.selected += 1;
        }
    }

    pub fn remove_current(&mut self) -> Option<Todo> {
        if self.todos.is_empty() {
            return None;
        }
        let index = self.selected;
        self.selected = index.min(self.todos.len().saturating_sub(2));
        return Some(self.todos.remove(index));
    }

    pub fn toggle_current(&mut self) {
        self.todos[self.selected].done = !self.todos[self.selected].done;
    }

    fn current(&self) -> &Todo {
        &self.todos[self.selected]
    }

    pub fn normal_mode(&mut self) {
        if self.current().text.is_empty() {
            self.remove_current();
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub fn append_to_current(&mut self, chr: char) {
        self.todos[self.selected].text.push(chr);
    }

    pub fn current_pop(&mut self) {
        self.todos[self.selected].text.pop();
    }

}
*/
