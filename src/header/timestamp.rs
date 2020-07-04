use core::fmt;
use chrono::{LocalResult, prelude::*};

/// The type returned when parsing last modified timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimestampResult {
    /// The timestamp could not be parsed.
    None,
    /// The timestamp value lacks the information about the time zone.
    Naive(NaiveDateTime),
    /// The timestamp value in the UTC time zone.
    Utc(DateTime<Utc>)
}

impl TimestampResult {
    pub fn is_none(&self) -> bool {
        if let TimestampResult::None = self {
            return true
        }
        false
    }

    pub fn is_naive(&self) -> bool {
        if let TimestampResult::Naive(..) = self {
            return true
        }
        false
    }

    pub fn is_utc(&self) -> bool {
        if let TimestampResult::Utc(..) = self {
            return true
        }
        false
    }

    /// Returns a `Naive` date and time variant as is or `Utc` variant as naive date time in the UTC time zone.
    pub fn to_naive_utc(&self) -> Option<NaiveDateTime> {
        match self {
            TimestampResult::Naive(dt) => Some(*dt),
            TimestampResult::Utc(dt) => Some(dt.naive_utc()),
            _ => None
        }
    }

    /// Returns a `Naive` date and time variant as is or `Utc` variant as naive date time in the `Local` time zone.
    pub fn to_naive_local(&self) -> Option<NaiveDateTime> {
        match self {
            TimestampResult::Naive(dt) => Some(*dt),
            TimestampResult::Utc(dt) => Some(dt.naive_local()),
            _ => None
        }
    }

    /// Returns a date time in the UTC time zone.
    ///
    /// In this instance the `Naive` date and time variant is assumed to be in the UTC time zone.
    pub fn to_utc(&self) -> Option<DateTime<Utc>> {
        match self {
            TimestampResult::Naive(dt) => Some(DateTime::from_utc(*dt, Utc)),
            TimestampResult::Utc(dt) => Some(*dt),
            _ => None
        }
    }

    /// Returns a date time in the `Local` time zone.
    ///
    /// In this instance the `Naive` date and time variant is assumed to be in the `Local` time zone.
    pub fn to_local(&self) -> Option<DateTime<Local>> {
        match self {
            TimestampResult::Naive(dt) => Local.from_local_datetime(dt).single(),
            TimestampResult::Utc(dt) => Some(dt.with_timezone(&Local)),
            _ => None
        }
    }
}

impl fmt::Display for TimestampResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimestampResult::None => "-".fmt(f),
            TimestampResult::Naive(dt) => dt.fmt(f),
            TimestampResult::Utc(dt) => dt.fmt(f)
        }
    }
}

impl From<NaiveDateTime> for TimestampResult {
    fn from(dt: NaiveDateTime) -> Self {
        TimestampResult::Naive(dt)
    }
}

impl<T: TimeZone> From<DateTime<T>> for TimestampResult {
    fn from(dt: DateTime<T>) -> Self {
        TimestampResult::Utc(dt.with_timezone(&Utc))
    }
}

impl From<Option<NaiveDateTime>> for TimestampResult {
    fn from(dt: Option<NaiveDateTime>) -> Self {
        match dt {
            Some(dt) => TimestampResult::Naive(dt),
            None => TimestampResult::None
        }
    }
}

impl<T: TimeZone> From<LocalResult<DateTime<T>>> for TimestampResult {
    fn from(dt: LocalResult<DateTime<T>>) -> Self {
        match dt {
            LocalResult::None|LocalResult::Ambiguous(..) => TimestampResult::None,
            LocalResult::Single(dt) => dt.into(),
        }
    }
}
