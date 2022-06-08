use chrono::prelude::*;
use chrono::Duration;
use chrono::Weekday;
#[cfg(feature = "serde-1")]
use serde::{Deserialize, Serialize};

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
#[cfg_attr(feature = "serde-1", derive(Serialize, Deserialize))]
pub enum Interval {
    /// The next multiple of `n` seconds since the start of the Unix epoch
    Seconds(u32),
    /// The next multiple of `n` minutes since the start of the day
    Minutes(u32),
    /// The next multiple of `n` hours since the start of the day
    Hours(u32),
    /// The next multiple of `n` days since the start of the start of the era
    Days(u32),
    /// The next multiple of `n` week since the start of the start of the era
    Weeks(u32),
    /// Every Monday
    Monday,
    /// Every Tuesday
    Tuesday,
    /// Every Wednesday
    Wednesday,
    /// Every Thursday
    Thursday,
    /// Every Friday
    Friday,
    /// Every Saturday
    Saturday,
    /// Every Sunday
    Sunday,
    /// Every weekday (Monday through Friday)
    Weekday,
}

pub trait NextTime {
    fn next<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz>;
    fn prev<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz>;
}

pub(crate) fn parse_time(s: &str) -> Result<NaiveTime, chrono::ParseError> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%I:%M:%S %p"))
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .or_else(|_| NaiveTime::parse_from_str(s, "%I:%M %p"))
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(Serialize, Deserialize))]
enum Adjustment {
    Intervals(Vec<Interval>),
    Time(NaiveTime),
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde-1", derive(Serialize, Deserialize))]
pub(crate) struct RunConfig {
    base: Interval,
    #[cfg_attr(feature = "serde-1", serde(skip_serializing_if = "Option::is_none"))]
    adjustment: Option<Adjustment>,
}

/// A RunConfig defines a schedule for a recurring event. It's composed of a base [`Interval`], and an additional adjustment.
/// The adjustment is either a single day offset (e.g. "at 3 AM") for use in conjunction with a base interval like "every three days", or "every Tuesday",
/// or it's a sequence of additional intervals, with the intended use of providing an additional offset for the scheduled task e.g.
/// "Every three hours, plus 30 minutes, plus 10 seconds".
impl RunConfig {
    pub fn from_interval(base: Interval) -> Self {
        RunConfig {
            base,
            adjustment: None,
        }
    }

    pub fn with_time(&self, t: NaiveTime) -> Self {
        RunConfig {
            adjustment: Some(Adjustment::Time(t)),
            ..*self
        }
    }

    pub fn with_subinterval(&self, ival: Interval) -> Self {
        let mut ival_queue = match self.adjustment {
            None => vec![],
            Some(Adjustment::Time(_)) => vec![],
            Some(Adjustment::Intervals(ref ivals)) => ivals.clone(),
        };
        ival_queue.push(ival);
        RunConfig {
            adjustment: Some(Adjustment::Intervals(ival_queue)),
            ..*self
        }
    }

    fn apply_adjustment<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        match self.adjustment {
            None => from.clone(),
            Some(Adjustment::Time(t)) => {
                let from_time = from.time();
                if t >= from_time {
                    from.date().and_time(t).unwrap()
                } else {
                    (from.date() + Duration::days(1)).and_time(t).unwrap()
                }
            }
            Some(Adjustment::Intervals(ref ivals)) => {
                let mut rv = from.clone();
                for ival in ivals {
                    rv = ival.next(&rv);
                }
                rv
            }
        }
    }
}

impl NextTime for RunConfig {
    fn next<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        let candidate = self.apply_adjustment(&self.base.prev(from));
        if candidate > *from {
            candidate
        } else {
            self.apply_adjustment(&self.base.next(from))
        }
    }
    fn prev<Tz: TimeZone>(&self, _from: &DateTime<Tz>) -> DateTime<Tz> {
        unimplemented!()
    }
}

static DAYS_TO_SHIFT: [u8; 14] = [7, 6, 5, 4, 3, 2, 1, 7, 6, 5, 4, 3, 2, 1];

fn day_of_week(i: Interval) -> usize {
    match i {
        Monday => 0,
        Tuesday => 1,
        Wednesday => 2,
        Thursday => 3,
        Friday => 4,
        Saturday => 5,
        Sunday => 6,
        _ => 7,
    }
}

use crate::Interval::*;
impl NextTime for Interval {
    fn next<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        match *self {
            Seconds(x) | Minutes(x) | Hours(x) | Days(x) | Weeks(x) if x == 0 => {
                return from.clone()
            }
            _ => (),
        }
        match *self {
            Seconds(s) => {
                let modulus = from.timestamp().checked_rem(i64::from(s)).unwrap_or(0);
                let next = s - (modulus as u32);
                from.with_nanosecond(0).unwrap() + Duration::seconds(i64::from(next))
            }
            Minutes(m) => {
                let s = from.num_seconds_from_midnight();
                let modulus = s.checked_rem(m * 60).unwrap_or(0);
                from.with_nanosecond(0).unwrap() + Duration::seconds(i64::from(m * 60 - modulus))
            }
            Hours(h) => {
                let s = from.num_seconds_from_midnight();
                let modulus = s.checked_rem(h * 3600).unwrap_or(0);
                from.with_nanosecond(0).unwrap() + Duration::seconds(i64::from(h * 3600 - modulus))
            }
            Days(d) => {
                let day_of_era = from.num_days_from_ce() as u32;
                let modulus = day_of_era.checked_rem(d).unwrap_or(0);
                (from.date() + Duration::days(i64::from(d - modulus))).and_hms(0, 0, 0)
            }
            Weeks(w) => {
                let d = from.date();
                let dow = d.weekday().num_days_from_monday();
                let start_of_week = d.clone() - Duration::days(i64::from(dow));
                let days_since_ever = d.num_days_from_ce();
                let week_num = (days_since_ever / 7) as u32;
                let modulus = week_num.checked_rem(w).unwrap_or(0);
                (start_of_week + Duration::weeks(i64::from(w - modulus))).and_hms(0, 0, 0)
            }
            Monday | Tuesday | Wednesday | Thursday | Friday | Saturday | Sunday => {
                let d = from.date();
                let dow = d.weekday().num_days_from_monday() as usize;
                let i_dow = day_of_week(*self);
                let to_shift = DAYS_TO_SHIFT[7 - i_dow + dow];
                (from.date() + Duration::days(i64::from(to_shift))).and_hms(0, 0, 0)
            }
            Weekday => {
                let d = from.date();
                let dow = d.weekday();
                let days = match dow {
                    Weekday::Fri => 3,
                    Weekday::Sat => 2,
                    _ => 1,
                };
                (from.date() + Duration::days(days)).and_hms(0, 0, 0)
            }
        }
    }

    fn prev<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        match *self {
            Seconds(x) | Minutes(x) | Hours(x) | Days(x) | Weeks(x) if x == 0 => {
                return from.clone()
            }
            _ => (),
        }
        match *self {
            Seconds(s) => {
                let modulus = from.timestamp().checked_rem(i64::from(s)).unwrap_or(0);
                let modulus = if modulus == 0 { i64::from(s) } else { modulus };
                from.with_nanosecond(0).unwrap() - Duration::seconds(modulus as i64)
            }
            Minutes(m) => {
                let s = from.num_seconds_from_midnight();
                let modulus = s.checked_rem(m * 60).unwrap_or(0);
                let modulus = if modulus == 0 { m * 60 } else { modulus };
                from.with_nanosecond(0).unwrap() - Duration::seconds(i64::from(modulus))
            }
            Hours(h) => {
                let s = from.num_seconds_from_midnight();
                let modulus = s.checked_rem(h * 3600).unwrap_or(0);
                let modulus = if modulus == 0 { h * 3600 } else { modulus };
                from.with_nanosecond(0).unwrap() - Duration::seconds(i64::from(modulus))
            }
            Days(d) => {
                let day_of_era = from.num_days_from_ce() as u32;
                let modulus = day_of_era.checked_rem(d).unwrap_or(0);
                let modulus = if modulus == 0 && from.num_seconds_from_midnight() == 0 {
                    d
                } else {
                    modulus
                };
                (from.date() - Duration::days(i64::from(modulus))).and_hms(0, 0, 0)
            }
            Weeks(w) => {
                let d = from.date();
                let dow = d.weekday().num_days_from_monday();
                let start_of_week = d.clone() - Duration::days(i64::from(dow));
                let days_since_ever = d.num_days_from_ce();
                let week_num = (days_since_ever / 7) as u32;
                let modulus = week_num.checked_rem(w).unwrap_or(0);
                let modulus = if modulus == 0 && from.num_seconds_from_midnight() == 0 {
                    w
                } else {
                    modulus
                };
                (start_of_week - Duration::weeks(i64::from(modulus))).and_hms(0, 0, 0)
            }
            Monday | Tuesday | Wednesday | Thursday | Friday | Saturday | Sunday => {
                let d = from.date();
                let dow = d.weekday().num_days_from_monday() as i32;
                let i_dow = day_of_week(*self) as i32;
                let mut to_shift = if dow >= i_dow {
                    dow - i_dow
                } else {
                    7 + dow - i_dow
                };
                if to_shift == 0 && from.num_seconds_from_midnight() == 0 {
                    to_shift = 7;
                }

                (from.date() - Duration::days(i64::from(to_shift))).and_hms(0, 0, 0)
            }
            Weekday => {
                let d = from.date();
                let dow = d.weekday();
                let days = match dow {
                    Weekday::Sat => 1,
                    Weekday::Sun => 2,
                    _ => {
                        if from.num_seconds_from_midnight() == 0 {
                            1
                        } else {
                            0
                        }
                    }
                };
                (from.date() - Duration::days(days)).and_hms(0, 0, 0)
            }
        }
    }
}

impl Interval {
    pub(crate) fn next_from<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        match *self {
            Seconds(x) | Minutes(x) | Hours(x) | Days(x) | Weeks(x) if x == 0 => {
                return from.clone()
            }
            _ => (),
        }

        match *self {
            Seconds(s) => from.clone() + Duration::seconds(s as i64),
            Minutes(m) => from.clone() + Duration::seconds(m as i64 * 60),
            Hours(h) => from.clone() + Duration::seconds(h as i64 * 3600),
            Days(d) => from.clone() + Duration::days(d as i64),
            Weeks(w) => from.clone() + Duration::days(w as i64 * 7),
            Monday | Tuesday | Wednesday | Thursday | Friday | Saturday | Sunday => self.next(from),
            Weekday => {
                let d = from.date();
                let dow = d.weekday();
                let days = match dow {
                    Weekday::Fri => 3,
                    Weekday::Sat => 2,
                    _ => 1,
                };
                from.clone() + Duration::days(days)
            }
        }
    }
}

/// A trait for easily expressing common intervals. Each method generates an appropriate [Interval].
/// Plural and non-plural forms behave identically, but exist to make code more grammatical.
/// ```rust
/// # use clokwerk::Interval;
/// # use clokwerk::TimeUnits;
/// assert_eq!(5.seconds(), Interval::Seconds(5));
/// assert_eq!(12.minutes(), Interval::Minutes(12));
/// assert_eq!(2.hours(), Interval::Hours(2));
/// assert_eq!(3.days(), Interval::Days(3));
/// assert_eq!(1.week(), Interval::Weeks(1));
/// ```
pub trait TimeUnits: Sized {
    fn seconds(self) -> Interval;
    fn minutes(self) -> Interval;
    fn hours(self) -> Interval;
    fn days(self) -> Interval;
    fn weeks(self) -> Interval;
    fn second(self) -> Interval {
        self.seconds()
    }
    fn minute(self) -> Interval {
        self.minutes()
    }
    fn hour(self) -> Interval {
        self.hours()
    }
    fn day(self) -> Interval {
        self.days()
    }
    fn week(self) -> Interval {
        self.weeks()
    }
}

impl TimeUnits for u32 {
    fn seconds(self) -> Interval {
        Seconds(self)
    }
    fn minutes(self) -> Interval {
        Minutes(self)
    }
    fn hours(self) -> Interval {
        Hours(self)
    }
    fn days(self) -> Interval {
        Days(self)
    }
    fn weeks(self) -> Interval {
        Weeks(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::intervals::{NextTime, RunConfig};
    use crate::Interval::*;
    use crate::TimeUnits;
    use chrono::prelude::*;

    #[test]
    fn basic_units() {
        assert_eq!(Seconds(5), 5.seconds());
        assert_eq!(Minutes(5), 5.minutes());
        assert_eq!(Hours(5), 5.hours());
        assert_eq!(Days(5), 5.days());
        assert_eq!(Weeks(5), 5.weeks());

        assert_eq!(Seconds(0), 0.seconds());
        assert_eq!(Minutes(0), 0.minutes());
        assert_eq!(Hours(0), 0.hours());
        assert_eq!(Days(0), 0.days());
        assert_eq!(Weeks(0), 0.weeks());
    }

    #[test]
    fn test_next_start() {
        // Set 999 ms to check that we remove any sub-second values
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13.999-00:00").unwrap();

        let next_dt = 5.seconds().next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:22:15-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = 5.seconds().next(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:22:20-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 15.minutes().next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:30:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = 15.minutes().next(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:45:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 2.hours().next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T16:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = 2.hours().next(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T18:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 2.days().next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-05T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = 2.days().next(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-07T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 2.weeks().next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-10T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = 2.weeks().next(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-24T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = Monday.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-10T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = Monday.next(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-17T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = Wednesday.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-05T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = Wednesday.next(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-12T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let friday = Friday.next(&dt);
        let next_dt = Weekday.next(&friday);
        let expected = DateTime::parse_from_rfc3339("2018-09-10T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = Weekday.next(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-11T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
    }

    #[test]
    fn test_prev() {
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13.999-00:00").unwrap();

        let prev_dt = 5.seconds().prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:22:10-00:00").unwrap();
        assert_eq!(prev_dt, expected);
        let prev_dt = 5.seconds().prev(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:22:05-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = 1.second().prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:22:12-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = 15.minutes().prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:15:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);
        let prev_dt = 15.minutes().prev(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = 1.minute().prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:22:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = 2.hours().prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);
        let prev_dt = 2.hours().prev(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T12:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = 1.hour().prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = 2.days().prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-03T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);
        let prev_dt = 2.days().prev(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-01T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = 1.day().prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = 2.weeks().prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-08-27T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);
        let prev_dt = 2.weeks().prev(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-08-13T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = 1.week().prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-03T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = Monday.prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-03T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);
        let prev_dt = Monday.prev(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-08-27T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let prev_dt = Wednesday.prev(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-08-29T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);
        let prev_dt = Wednesday.prev(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-08-22T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);

        let saturday = Saturday.prev(&dt);
        let prev_dt = Weekday.prev(&saturday);
        let expected = DateTime::parse_from_rfc3339("2018-08-31T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);
        let prev_dt = Weekday.prev(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-08-30T00:00:00-00:00").unwrap();
        assert_eq!(prev_dt, expected);
    }

    use super::parse_time;
    #[test]
    fn test_parse_time() {
        assert_eq!(parse_time("14:52:13"), Ok(NaiveTime::from_hms(14, 52, 13)));
        assert_eq!(
            parse_time("2:52:13 pm"),
            Ok(NaiveTime::from_hms(14, 52, 13))
        );
        assert_eq!(parse_time("14:52"), Ok(NaiveTime::from_hms(14, 52, 0)));
        assert_eq!(parse_time("2:52 PM"), Ok(NaiveTime::from_hms(14, 52, 0)));
    }

    #[test]
    fn test_run_config() {
        let rc = RunConfig::from_interval(1.day()).with_time(NaiveTime::from_hms(15, 0, 0));
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T15:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let rc = RunConfig::from_interval(Tuesday).with_time(NaiveTime::from_hms(15, 0, 0));
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T15:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let rc = RunConfig::from_interval(Tuesday).with_time(NaiveTime::from_hms(14, 0, 0));
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-11T14:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let rc = RunConfig::from_interval(Tuesday)
            .with_subinterval(6.hours())
            .with_subinterval(5.minutes());
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-11T06:05:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
    }

    #[test]
    fn test_division_by_zero() {
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();
        assert_eq!(0.seconds().next(&dt), dt, "next 0 seconds");
        assert_eq!(0.seconds().prev(&dt), dt, "previous 0 seconds");
        assert_eq!(0.minutes().next(&dt), dt, "next 0 minutes");
        assert_eq!(0.minutes().prev(&dt), dt, "prev 0 minutes");
        assert_eq!(0.hours().next(&dt), dt, "next 0 hours");
        assert_eq!(0.hours().prev(&dt), dt, "prev 0 hours");
        assert_eq!(0.days().next(&dt), dt, "next 0 days");
        assert_eq!(0.days().prev(&dt), dt, "prev 0 days");
        assert_eq!(0.weeks().next(&dt), dt, "next 0 weeks");
        assert_eq!(0.weeks().prev(&dt), dt, "prev 0 weeks");
    }

    #[test]
    fn test_daily_interval_plus_time_of_midnight() {
        // See https://github.com/mdsherry/clokwerk/issues/22
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();
        let rc = RunConfig::from_interval(Tuesday).with_time(NaiveTime::from_hms(0, 0, 0));
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-11T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
    }
}
#[cfg(all(test, feature = "serde-1"))]
mod serde_tests {
    use chrono::NaiveTime;

    use super::{Adjustment, Interval, RunConfig};

    // Tests to guard against breaking changes to the serialization formats
    const SERIALIZED_INTERVALS: &str = r#"[{"Seconds":5},{"Minutes":10},{"Hours":15},{"Days":20},{"Weeks":30},"Monday","Tuesday","Wednesday","Thursday","Friday","Saturday","Sunday","Weekday"]"#;
    const INTERVALS: &[Interval] = &[
        Interval::Seconds(5),
        Interval::Minutes(10),
        Interval::Hours(15),
        Interval::Days(20),
        Interval::Weeks(30),
        Interval::Monday,
        Interval::Tuesday,
        Interval::Wednesday,
        Interval::Thursday,
        Interval::Friday,
        Interval::Saturday,
        Interval::Sunday,
        Interval::Weekday,
    ];

    #[test]
    fn test_serialize_interval() {
        let serialized = serde_json::to_string(&INTERVALS).unwrap();
        assert_eq!(SERIALIZED_INTERVALS, serialized);
    }

    #[test]
    fn test_deserialize_interval() {
        let deserialized: Vec<Interval> = serde_json::from_str(SERIALIZED_INTERVALS).unwrap();
        assert_eq!(INTERVALS, deserialized)
    }

    const SERIALIZED_ADJUSTMENTS: &str =
        r#"[{"Intervals":[{"Seconds":5},"Tuesday"]},{"Time":"16:45:07"}]"#;
    fn sample_adjustments() -> Vec<Adjustment> {
        vec![
            Adjustment::Intervals(vec![Interval::Seconds(5), Interval::Tuesday]),
            Adjustment::Time(NaiveTime::from_hms(16, 45, 7)),
        ]
    }

    #[test]
    fn test_serialize_adjustments() {
        let serialized = serde_json::to_string(&sample_adjustments()).unwrap();
        assert_eq!(SERIALIZED_ADJUSTMENTS, serialized);
    }

    #[test]
    fn test_deserialize_adjustments() {
        let deserialized: Vec<Adjustment> = serde_json::from_str(SERIALIZED_ADJUSTMENTS).unwrap();
        assert_eq!(sample_adjustments(), deserialized)
    }

    const SERIALIZED_RUN_CONFIG_1: &str = r#"{"base":"Thursday","adjustment":{"Time":"01:02:03"}}"#;
    fn rc1() -> RunConfig {
        RunConfig {
            base: Interval::Thursday,
            adjustment: Some(Adjustment::Time(NaiveTime::from_hms(1, 2, 3))),
        }
    }
    #[test]
    fn test_serialize_run_config_1() {
        let serialized = serde_json::to_string(&rc1()).unwrap();
        assert_eq!(SERIALIZED_RUN_CONFIG_1, serialized);
    }

    #[test]
    fn test_deserialize_run_config_1() {
        let deserialized: RunConfig = serde_json::from_str(SERIALIZED_RUN_CONFIG_1).unwrap();
        assert_eq!(rc1(), deserialized);
    }

    const SERIALIZED_RUN_CONFIG_2: &str = r#"{"base":{"Seconds":120}}"#;
    fn rc2() -> RunConfig {
        RunConfig {
            base: Interval::Seconds(120),
            adjustment: None,
        }
    }
    #[test]
    fn test_serialize_run_config_2() {
        let serialized = serde_json::to_string(&rc2()).unwrap();
        assert_eq!(SERIALIZED_RUN_CONFIG_2, serialized);
    }

    #[test]
    fn test_deserialize_run_config_2() {
        let deserialized: RunConfig = serde_json::from_str(SERIALIZED_RUN_CONFIG_2).unwrap();
        assert_eq!(rc2(), deserialized);
    }
}
