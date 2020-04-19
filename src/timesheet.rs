use super::tracc::ListView;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::from_reader;
use std::default;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use time::OffsetDateTime;

pub struct TimeSheet {
    pub times: Vec<TimePoint>,
    pub selected: usize,
    pub register: Option<TimePoint>,
    pub editing_time: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TimePoint {
    text: String,
    time: OffsetDateTime,
}

impl TimePoint {
    pub fn new(text: &str) -> Self {
        Self {
            text: String::from(text),
            time: OffsetDateTime::now_local(),
        }
    }
}

impl fmt::Display for TimePoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{}] {}",
            self.time
                .to_offset(time::UtcOffset::current_local_offset())
                .format("%H:%M"),
            self.text
        )
    }
}

impl default::Default for TimePoint {
    fn default() -> Self {
        TimePoint::new("")
    }
}

fn read_times(path: &str) -> Option<Vec<TimePoint>> {
    File::open(path)
        .ok()
        .map(|f| BufReader::new(f))
        .and_then(|r| from_reader(r).ok())
}

impl TimeSheet {
    pub fn open_or_create(path: &str) -> Self {
        Self {
            times: read_times(path).unwrap_or(vec![TimePoint::new("Did something")]),
            selected: 0,
            register: None,
            editing_time: false,
        }
    }

    pub fn printable(&self) -> Vec<String> {
        self.times.iter().map(TimePoint::to_string).collect()
    }

    fn current(&self) -> &TimePoint {
        &self.times[self.selected]
    }

    pub fn time_by_tasks(&self) -> String {
        self.times
            .iter()
            .tuple_windows()
            .map(|(prev, next)| (prev.text.clone(), next.time - prev.time))
            .fold(
                std::collections::BTreeMap::new(),
                |mut map, (text, duration)| {
                    *map.entry(text).or_insert(time::Duration::zero()) += duration;
                    map
                },
            )
            .into_iter()
            .map(|(text, duration)| format!("{}: {}", text, format_duration(&duration)))
            .join(" | ")
    }

    pub fn sum_as_str(&self) -> String {
        let total = self
            .times
            .iter()
            .map(|tp| tp.time)
            .tuple_windows()
            .fold(time::Duration::zero(), |total, (last, next)| {
                total + (next - last)
            });
        format_duration(&total)
    }
}

fn format_duration(d: &time::Duration) -> String {
    format!("{}:{:02}", d.whole_hours(), d.whole_minutes().max(1) % 60)
}

impl ListView<TimePoint> for TimeSheet {
    fn selection_pointer(&mut self) -> &mut usize {
        &mut self.selected
    }

    fn list(&mut self) -> &mut Vec<TimePoint> {
        &mut self.times
    }

    fn register(&mut self) -> &mut Option<TimePoint> {
        &mut self.register
    }

    fn normal_mode(&mut self) {
        if self.current().text.is_empty() {
            self.remove_current();
            self.selected = self.selected.saturating_sub(1);
        }
        self.times.sort_by_key(|t| t.time);
    }

    fn toggle_current(&mut self) {
        self.editing_time = !self.editing_time;
    }

    fn append_to_current(&mut self, chr: char) {
        self.times[self.selected].text.push(chr);
    }

    fn backspace(&mut self) {
        self.times[self.selected].text.pop();
    }
}
