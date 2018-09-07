use chrono::prelude::*;
use chrono::Duration;
use chrono::Weekday;

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
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

pub(crate) fn parse_time(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%I:%M:%S %p"))
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .or_else(|_| NaiveTime::parse_from_str(s, "%I:%M %p"))
        .ok()
}

#[derive(Debug)]
enum Adjustment {
    Intervals(Vec<Interval>),
    Time(NaiveTime),
}

#[derive(Debug)]
pub(crate) struct RunConfig {
    base: Interval,
    adjustment: Option<Adjustment>,
}

impl RunConfig {
    pub fn from_interval(base: Interval) -> Self {
        RunConfig {
            base,
            adjustment: None,
        }
    }

    pub fn with_time(&self, s: &str) -> Self {
        RunConfig {
            adjustment: Some(Adjustment::Time(parse_time(s).unwrap())),
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
            Some(Adjustment::Time(ref t)) => {
                let from_time = from.time();
                if *t > from_time {
                    from.date().and_time(t.clone()).unwrap()
                } else {
                    (from.date() + Duration::days(1))
                        .and_time(t.clone())
                        .unwrap()
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

use Interval::*;
impl NextTime for Interval {
    fn next<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        match *self {
            Seconds(s) => {
                let modulus = from.timestamp() % (i64::from(s));
                let next = s - (modulus as u32);
                from.clone() + Duration::seconds(i64::from(next))
            }
            Minutes(m) => {
                let s = from.num_seconds_from_midnight();
                let modulus = s % (m * 60);
                from.clone() + Duration::seconds(i64::from(m * 60 - modulus))
            }
            Hours(h) => {
                let s = from.num_seconds_from_midnight();
                let modulus = s % (h * 3600);
                from.clone() + Duration::seconds(i64::from(h * 3600 - modulus))
            }
            Days(d) => {
                let day_of_era = from.num_days_from_ce() as u32;
                let modulus = day_of_era % d;
                (from.date() + Duration::days(i64::from(d - modulus))).and_hms(0, 0, 0)
            }
            Weeks(w) => {
                let d = from.date();
                let dow = d.weekday().num_days_from_monday();
                let start_of_week = d.clone() - Duration::days(i64::from(dow));
                let days_since_ever = d.num_days_from_ce();
                let week_num = (days_since_ever / 7) as u32;
                let modulus = week_num % w;
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
            Seconds(s) => {
                let modulus = from.timestamp() % i64::from(s);
                let modulus = if modulus == 0 { i64::from(s) } else { modulus };
                from.clone() - Duration::seconds(modulus as i64)
            }
            Minutes(m) => {
                let s = from.num_seconds_from_midnight();
                let modulus = s % (m * 60);
                let modulus = if modulus == 0 { (m * 60) } else { modulus };
                from.clone() - Duration::seconds(i64::from(modulus))
            }
            Hours(h) => {
                let s = from.num_seconds_from_midnight();
                let modulus = s % (h * 3600);
                let modulus = if modulus == 0 { (h * 3600) } else { modulus };
                from.clone() - Duration::seconds(i64::from(modulus))
            }
            Days(d) => {
                let day_of_era = from.num_days_from_ce() as u32;
                let modulus = day_of_era % d;
                let modulus = if modulus == 0 && from.num_seconds_from_midnight() == 0 { d } else { modulus };
                (from.date() - Duration::days(i64::from(modulus))).and_hms(0, 0, 0)
            }
            Weeks(w) => {
                let d = from.date();
                let dow = d.weekday().num_days_from_monday();
                let start_of_week = d.clone() - Duration::days(i64::from(dow));
                let days_since_ever = d.num_days_from_ce();
                let week_num = (days_since_ever / 7) as u32;
                let modulus = week_num % w;
                let modulus = if modulus == 0 && from.num_seconds_from_midnight() == 0 { w } else { modulus };
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
                    _ => if from.num_seconds_from_midnight() == 0 { 1 } else { 0 },
                };
                (from.date() - Duration::days(days)).and_hms(0, 0, 0)
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
    use chrono::prelude::*;
    use intervals::NextTime;
    use Interval::*;
    use RunConfig;
    use TimeUnits;

    #[test]
    fn basic_units() {
        assert_eq!(Seconds(5), 5.seconds());
    }

    #[test]
    fn test_next_start() {
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();

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
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();

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
        assert_eq!(
            parse_time("14:52:13"),
            Some(NaiveTime::from_hms(14, 52, 13))
        );
        assert_eq!(
            parse_time("2:52:13 pm"),
            Some(NaiveTime::from_hms(14, 52, 13))
        );
        assert_eq!(parse_time("14:52"), Some(NaiveTime::from_hms(14, 52, 0)));
        assert_eq!(parse_time("2:52 PM"), Some(NaiveTime::from_hms(14, 52, 0)));
    }

    #[test]
    fn test_run_config() {
        let rc = RunConfig::from_interval(1.day()).with_time("15:00");
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T15:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let rc = RunConfig::from_interval(Tuesday).with_time("15:00");
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T15:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let rc = RunConfig::from_interval(Tuesday).with_time("14:00");
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-11T14:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let rc = RunConfig::from_interval(Tuesday).with_subinterval(6.hours()).with_subinterval(5.minutes());
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-11T06:05:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
    }
}
