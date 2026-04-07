use itertools::Itertools;
use serde::ser::SerializeTuple;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::from_reader;
use std::{
    collections,
    convert::TryFrom,
    default, env, fmt, fs, io,
    path::{Path, PathBuf},
};
use time::{Date, Duration, OffsetDateTime};

#[derive(Clone)]
pub struct TimeSheet {
    pub date: Date,
    pub times: Vec<TimePoint>,
    pub selected: usize,
    pub register: Option<TimePoint>,
}

const MAIN_PAUSE_TEXT: &str = "pause";
const PAUSE_TEXTS: [&str; 4] = [MAIN_PAUSE_TEXT, "lunch", "mittag", "break"];
const END_TEXT: &str = "end";
lazy_static! {
    static ref OVERRIDE_REGEX: regex::Regex = regex::Regex::new("\\[(.*)\\]").unwrap();
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TimePoint {
    text: String,
    #[serde(
        serialize_with = "serialize_minutes",
        deserialize_with = "deserialize_minutes"
    )]
    time: i64,
}

impl TimePoint {
    pub fn new(text: &str, time: i64) -> Self {
        Self {
            text: String::from(text),
            time,
        }
    }

    pub fn time(&self) -> i64 {
        self.time
    }
}

fn serialize_minutes<S>(minutes: &i64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut tuple = serializer.serialize_tuple(4)?;
    tuple.serialize_element(&minutes.div_euclid(60))?;
    tuple.serialize_element(&minutes.rem_euclid(60))?;
    tuple.serialize_element(&0)?;
    tuple.serialize_element(&0)?;
    tuple.end()
}

fn deserialize_minutes<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    struct MinutesVisitor;

    impl<'de> de::Visitor<'de> for MinutesVisitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an integer minute offset or a time string")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
            Ok(value)
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            i64::try_from(value).map_err(|_| E::custom("time value is too large"))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            parse_minutes(value).map_err(E::custom)
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&value)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let hour: i64 = seq
                .next_element()?
                .ok_or_else(|| de::Error::invalid_length(0, &self))?;
            let minute: i64 = seq
                .next_element()?
                .ok_or_else(|| de::Error::invalid_length(1, &self))?;
            let _ = seq.next_element::<de::IgnoredAny>()?;
            let _ = seq.next_element::<de::IgnoredAny>()?;
            Ok(hour * 60 + minute)
        }
    }

    deserializer.deserialize_any(MinutesVisitor)
}

pub fn parse_minutes(value: &str) -> Result<i64, String> {
    let value = value.trim();
    if value.len() == 4 && value.chars().all(|chr| chr.is_ascii_digit()) {
        let hours = value[..2]
            .parse::<i64>()
            .map_err(|_| format!("invalid hour in time value: {value}"))?;
        let minutes = value[2..]
            .parse::<i64>()
            .map_err(|_| format!("invalid minute in time value: {value}"))?;
        return Ok(hours * 60 + minutes);
    }

    if let Some((hours, minutes)) = value.split_once(':') {
        let hours = hours
            .parse::<i64>()
            .map_err(|_| format!("invalid hour in time value: {value}"))?;
        let minutes = minutes
            .parse::<i64>()
            .map_err(|_| format!("invalid minute in time value: {value}"))?;
        return Ok(hours * 60 + minutes);
    }

    value
        .parse::<i64>()
        .map_err(|_| format!("invalid time value: {value}"))
}

fn current_minutes_since(date: Date) -> i64 {
    let now = OffsetDateTime::now_local().unwrap();
    let day_diff = (now.date() - date).whole_days();
    day_diff * 24 * 60 + now.time().hour() as i64 * 60 + now.time().minute() as i64
}

fn today() -> Date {
    OffsetDateTime::now_local().unwrap().date()
}

#[cfg(windows)]
fn data_dir() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(target_os = "macos")]
fn data_dir() -> PathBuf {
    env::var_os("HOME")
        .map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn data_dir() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn storage_path_for(date: Date) -> PathBuf {
    let (year, month, day) = date.to_calendar_date();
    data_dir()
        .join("tracc")
        .join("timesheets")
        .join(format!("{}", year))
        .join(format!("{:02}", u8::from(month)))
        .join(format!("{:02}.json", day))
}

impl fmt::Display for TimePoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}] {}", format_minutes(self.time), self.text)
    }
}

impl default::Default for TimePoint {
    fn default() -> Self {
        TimePoint::new("", 0)
    }
}

fn read_times(path: &Path) -> Option<Vec<TimePoint>> {
    fs::File::open(path)
        .ok()
        .map(io::BufReader::new)
        .and_then(|r| from_reader(r).ok())
}

/**
 * If a time text contains "[something]",
 * only use the message inside the brackets.
 */
fn effective_text(s: String) -> String {
    let text = OVERRIDE_REGEX
        .captures(&s)
        // index 0 is the entire string
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .unwrap_or(&s);
    if PAUSE_TEXTS.contains(&text) {
        MAIN_PAUSE_TEXT
    } else {
        text
    }
    .to_string()
}

impl TimeSheet {
    pub fn open(date: Date) -> Self {
        let path = storage_path_for(date);
        let times = read_times(&path).unwrap_or_default();
        let selected = times.len().saturating_sub(1);
        Self {
            date,
            times,
            selected,
            register: None,
        }
    }

    pub fn current_date() -> Date {
        today()
    }

    pub fn date_label(&self) -> String {
        let (year, month, day) = self.date.to_calendar_date();
        format!("{:04}-{:02}-{:02}", year, u8::from(month), day)
    }

    pub fn is_today(&self) -> bool {
        self.date == today()
    }

    pub fn selected_index(&self) -> Option<usize> {
        if self.times.is_empty() {
            None
        } else {
            Some(self.selected.min(self.times.len().saturating_sub(1)))
        }
    }

    pub fn printable(&self) -> Vec<String> {
        self.times.iter().map(TimePoint::to_string).collect()
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selected_index()
            .map(|index| self.times[index].text.clone())
    }

    pub fn selected_time(&self) -> Option<i64> {
        self.selected_index().map(|index| self.times[index].time())
    }

    pub fn current_minutes_since_start(&self) -> i64 {
        current_minutes_since(self.date)
    }

    pub fn set_selected_text(&mut self, text: String) {
        if self.times.is_empty() {
            return;
        }
        self.times[self.selected].text = text;
    }

    pub fn set_selected_time(&mut self, time: i64) {
        if self.times.is_empty() {
            return;
        }
        self.times[self.selected].time = time;
        let timepoint = self.times[self.selected].clone();
        self.times.sort_by_key(|tp| tp.time);
        self.selected = self.times.iter().position(|tp| tp == &timepoint).unwrap();
    }

    pub fn insertion_index_for_now(&self) -> usize {
        let time = self.current_minutes_since_start();
        self.times.partition_point(|tp| tp.time <= time)
    }

    pub fn selection_first(&mut self) {
        if self.times.is_empty() {
            return;
        }
        self.selected = 0;
    }

    pub fn selection_up(&mut self) {
        if self.times.is_empty() {
            return;
        }
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn selection_down(&mut self) {
        if self.times.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.times.len().saturating_sub(1));
    }

    pub fn selection_last(&mut self) {
        if self.times.is_empty() {
            return;
        }
        self.selected = self.times.len().saturating_sub(1);
    }

    pub fn insert(&mut self, item: TimePoint, position: Option<usize>) {
        let pos = position.unwrap_or(self.selected);
        if pos == self.times.len().saturating_sub(1) {
            self.times.push(item);
            self.selected = self.times.len() - 1;
        } else {
            self.times.insert(pos + 1, item);
            self.selected = pos + 1;
        }
    }

    pub fn insert_at(&mut self, item: TimePoint, index: usize) {
        let index = index.min(self.times.len());
        if index == self.times.len() {
            self.times.push(item);
            self.selected = self.times.len() - 1;
        } else {
            self.times.insert(index, item);
            self.selected = index;
        }
    }

    pub fn remove_current(&mut self) {
        if self.times.is_empty() {
            return;
        }
        let index = self.selected;
        self.selected = index.min(self.times.len().saturating_sub(2));
        self.register = self.times.remove(index).into();
    }

    pub fn paste(&mut self) {
        if let Some(item) = self.register.clone() {
            self.insert(item, None);
        }
    }

    pub fn can_paste(&self) -> bool {
        self.register.is_some()
    }

    pub fn yank(&mut self) {
        if self.times.is_empty() {
            return;
        }
        let index = self.selected;
        self.register = self.times[index].clone().into();
    }

    /**
     * Adjust the current time by `minutes` and round the result to a multiple of `minutes`.
     * This is so I can adjust in steps of 5 but still get nice, even numbers in the output.
     */
    pub fn shift_current(&mut self, minutes: i64) {
        if self.times.is_empty() {
            return;
        }
        let time = &mut self.times[self.selected].time;
        *time += minutes;
        *time -= time.rem_euclid(5);
        let timepoint = self.times[self.selected].clone();
        self.times.sort_by_key(|tp| tp.time);
        self.selected = self.times.iter().position(|tp| tp == &timepoint).unwrap();
    }

    pub fn has_time_overflow(&self) -> bool {
        self.times
            .last()
            .map(|tp| tp.time > 24 * 60)
            .unwrap_or(false)
    }

    fn grouped_times(&self) -> collections::BTreeMap<String, Duration> {
        let last_time = self.times.last();
        let current_time = self.current_minutes_since_start();
        self.times
            .iter()
            .chain(TimeSheet::maybe_end_time(last_time, current_time).iter())
            .tuple_windows()
            .map(|(prev, next)| (prev.text.clone(), Duration::minutes(next.time - prev.time)))
            // Fold into a map to group by description.
            // I use a BTreeMap because I need a stable output order for the iterator
            // (otherwise the summary list will jump around on every input).
            .fold(collections::BTreeMap::new(), |mut map, (text, duration)| {
                *map.entry(effective_text(text)).or_insert(Duration::ZERO) += duration;
                map
            })
    }

    fn maybe_end_time(last_time: Option<&TimePoint>, current_time: i64) -> Option<TimePoint> {
        match last_time {
            Some(tp) if PAUSE_TEXTS.contains(&&tp.text[..]) => None,
            Some(tp) if tp.text == END_TEXT => None,
            Some(tp) if tp.time > current_time => None,
            _ => Some(TimePoint::new(END_TEXT, current_time)),
        }
    }

    pub fn time_by_tasks(&self) -> String {
        self.grouped_times()
            .into_iter()
            .filter(|(text, _)| text != MAIN_PAUSE_TEXT)
            .map(|(text, duration)| format!("{}: {}", text, format_duration(&duration)))
            .join("\n")
    }

    pub fn sum_as_str(&self) -> String {
        let total = self
            .grouped_times()
            .into_iter()
            .filter(|(text, _)| text != MAIN_PAUSE_TEXT)
            .fold(Duration::ZERO, |total, (_, d)| total + d);
        format_duration(&total)
    }

    pub fn pause_time(&self) -> String {
        let times = self.grouped_times();
        let duration = times
            .get(MAIN_PAUSE_TEXT)
            .copied()
            .unwrap_or(Duration::ZERO);
        format!("{}: {}", MAIN_PAUSE_TEXT, format_duration(&duration))
    }
}

fn format_duration(d: &Duration) -> String {
    format!("{}:{:02}", d.whole_hours(), d.whole_minutes() % 60)
}

fn format_minutes(minutes: i64) -> String {
    let hours = minutes.div_euclid(60);
    let minutes = minutes.rem_euclid(60);
    format!("{:02}:{:02}", hours, minutes)
}
