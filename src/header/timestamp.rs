use core::fmt;
use chrono::{LocalResult, prelude::*};

/// The type returned when parsing last modified timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimestampResult {
    /// The timestamp could not be parsed
    None,
    /// The timestamp value lacks the information about the time zone
    Naive(NaiveDateTime),
    /// The timestamp value in the UTC time zone
    Utc(DateTime<Utc>)
}

impl TimestampResult {
    /// Return whether the timestamp could not be parsed due to invalid data
    pub fn is_none(&self) -> bool {
        if let TimestampResult::None = self {
            return true
        }
        false
    }
    /// Return whether the parsed timestamp lacks the information about the
    /// time zone
    pub fn is_naive(&self) -> bool {
        if let TimestampResult::Naive(..) = self {
            return true
        }
        false
    }
    /// Return whether the parsed timestamp has the information about the
    /// time zone
    pub fn is_utc(&self) -> bool {
        if let TimestampResult::Utc(..) = self {
            return true
        }
        false
    }

    /// Return a content of the [`Self::Naive`] date and time variant or 
    /// [`Self::Utc`] variant converted to naive date time object in the UTC
    /// time zone
    pub fn to_naive_utc(&self) -> Option<NaiveDateTime> {
        match self {
            TimestampResult::Naive(dt) => Some(*dt),
            TimestampResult::Utc(dt) => Some(dt.naive_utc()),
            _ => None
        }
    }

    /// Return a content of the [`Self::Naive`] date and time variant or 
    /// [`Self::Utc`] variant converted to naive date time object in the local
    /// time zone
    pub fn to_naive_local(&self) -> Option<NaiveDateTime> {
        match self {
            TimestampResult::Naive(dt) => Some(*dt),
            TimestampResult::Utc(dt) => Some(dt.naive_local()),
            _ => None
        }
    }

    /// Return a content of the [`Self::Utc`] date and time variant or 
    /// [`Self::Naive`] variant converted to the date time object in the UTC
    /// time zone
    pub fn to_utc(&self) -> Option<DateTime<Utc>> {
        match self {
            TimestampResult::Naive(dt) => Some(DateTime::from_naive_utc_and_offset(*dt, Utc)),
            TimestampResult::Utc(dt) => Some(*dt),
            _ => None
        }
    }

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    /// Return a content of the [`Self::Utc`] date and time variant or 
    /// [`Self::Naive`] variant converted to the date time object in the local
    /// time zone
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

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::string::ToString;
    use crate::header::{parse_msdos_datetime, parse_win_filetime};
    use super::*;

    #[test]
    fn timestamp_works() {
        assert!(parse_msdos_datetime(0).is_none());
        assert!(parse_msdos_datetime(u32::MAX).is_none());
        assert_eq!(parse_msdos_datetime(0b00000000_00100001_00000000_00000000).unwrap(),
            NaiveDate::from_ymd_opt(1980, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap());
        assert_eq!(parse_msdos_datetime(0b11111111_10011111_10111111_01111101).unwrap(),
            NaiveDate::from_ymd_opt(2107, 12, 31).unwrap().and_hms_opt(23, 59, 58).unwrap());
        assert_eq!(parse_win_filetime(0), Utc.with_ymd_and_hms(1601, 1, 1, 0, 0, 0));
        assert_eq!(parse_win_filetime(u64::MAX / 2).unwrap(), Utc.with_ymd_and_hms(30828,9,14,2,48,5)
            .unwrap().with_nanosecond(477580700).unwrap());
        assert!(matches!(parse_win_filetime(u64::MAX), LocalResult::None));
        
        let result: TimestampResult = parse_msdos_datetime(0).into();
        assert!(result.is_none());
        assert!(!result.is_naive());
        assert!(!result.is_utc());
        assert!(result.to_utc().is_none());
        assert!(result.to_naive_utc().is_none());
        assert!(result.to_naive_local().is_none());
        assert_eq!(result.to_string().as_str(), "-");
        let result: TimestampResult = parse_win_filetime(u64::MAX).into();
        assert!(result.is_none());
        assert!(!result.is_naive());
        assert!(!result.is_utc());
        assert!(result.to_utc().is_none());
        assert!(result.to_naive_utc().is_none());
        assert!(result.to_naive_local().is_none());
        assert_eq!(result.to_string().as_str(), "-");
        let result: TimestampResult = parse_msdos_datetime(0b00000000_00100001_00000000_00000000).into();
        assert!(!result.is_none());
        assert!(result.is_naive());
        assert!(!result.is_utc());
        assert!(result.to_utc().is_some());
        assert!(result.to_naive_utc().is_some());
        assert!(result.to_naive_local().is_some());
        assert_eq!(result.to_utc().unwrap(), Utc.with_ymd_and_hms(1980, 1, 1, 0, 0, 0).unwrap());
        assert_eq!(result.to_string().as_str(), "1980-01-01 00:00:00");
        let result: TimestampResult = parse_msdos_datetime(0b11111111_10011111_10111111_01111101).unwrap().into();
        assert!(!result.is_none());
        assert!(result.is_naive());
        assert!(!result.is_utc());
        assert_eq!(result.to_utc().unwrap(), Utc.with_ymd_and_hms(2107, 12, 31, 23, 59, 58).unwrap());
        assert_eq!(result.to_string().as_str(), "2107-12-31 23:59:58");
        let result: TimestampResult = parse_win_filetime(116_444_736_000_000_000).into();
        assert!(!result.is_none());
        assert!(!result.is_naive());
        assert!(result.is_utc());
        assert!(result.to_utc().is_some());
        assert!(result.to_naive_utc().is_some());
        assert!(result.to_naive_local().is_some());
        assert_eq!(result.to_utc().unwrap(), Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap());
        assert_eq!(result.to_string().as_str(), "1970-01-01 00:00:00 UTC");
    }
}
