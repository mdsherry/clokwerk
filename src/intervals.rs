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
    Weekday
}

pub trait NextTime {
    fn next<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz>;
    fn next_start<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz>;
}

pub(crate) fn parse_time(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%I:%M:%S %p"))
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .or_else(|_| NaiveTime::parse_from_str(s, "%I:%M %p")).ok()
}

#[derive(Debug)]
enum Adjustment {
    Intervals(Vec<Interval>),
    Time(NaiveTime)
}

#[derive(Debug)]
pub(crate) struct RunConfig {
    base: Interval,
    adjustment: Option<Adjustment>
}

impl RunConfig {
    pub fn from_interval(base: Interval) -> Self {
        RunConfig { base, adjustment: None }
    }

    pub fn with_time(&self, s: &str) -> Self {
        RunConfig { adjustment: Some(Adjustment::Time(parse_time(s).unwrap())), ..*self }
    }

    pub fn with_subinterval(&self, ival: Interval) -> Self {
        let mut ival_queue = match self.adjustment {
            None => vec![],
            Some(Adjustment::Time(_)) => vec![],
            Some(Adjustment::Intervals(ref ivals)) => ivals.clone()
        };
        ival_queue.push(ival);
        RunConfig { adjustment: Some(Adjustment::Intervals(ival_queue)), ..*self }
    }
}

impl NextTime for RunConfig {
    fn next<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        // XXX: This doesn't take adjustment into account
        // XXX: This doesn't do the right thing for `.every(Monday).at("13:00")` if it's
        // still before 1 PM on a Monday
        self.base.next_start(from)

    }
    fn next_start<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        self.next(from)
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
        _ => 7
    }
}

use Interval::*;
impl NextTime for Interval {
    fn next<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        match *self {
            Seconds(s) => from.clone() + Duration::seconds(s as i64),
            Minutes(m) => from.clone() + Duration::minutes(m as i64),
            Hours(h) => from.clone() + Duration::hours(h as i64),
            Days(d) => from.clone() + Duration::days(d as i64),
            Weeks(w) => from.clone() + Duration::weeks(w as i64),
            Monday | Tuesday | Wednesday | Thursday | Friday | Saturday | Sunday => {
                let d = from.date();
                let dow = d.weekday().num_days_from_monday() as usize;
                let i_dow = day_of_week(*self);
                let to_shift = DAYS_TO_SHIFT[7 - i_dow + dow];
                (from.date() + Duration::days(to_shift as i64)).and_hms(0, 0, 0)
            },
            Weekday => {
                let d = from.date();
                let dow = d.weekday();
                let days = match dow {
                    Weekday::Fri => 3,
                    Weekday::Sat => 2,
                    _ => 1
                };
                (from.date() + Duration::days(days)).and_hms(0, 0, 0)
            }
        }
    }

    fn next_start<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        match *self {
            Seconds(s) => {
                let modulus = from.timestamp() % (s as i64);
                let next = s - (modulus as u32);
                from.clone() + Duration::seconds(next as i64)
            }
            Minutes(m) => {
                let s = from.num_seconds_from_midnight();
                let modulus = s % (m * 60);
                from.clone() + Duration::seconds((m * 60 - modulus) as i64)
            }
            Hours(h) => {
                let s = from.num_seconds_from_midnight();
                let modulus = s % (h * 3600);
                from.clone() + Duration::seconds((h * 3600 - modulus) as i64)
            }
            Days(d) => {
                let day_of_era = from.num_days_from_ce() as u32;
                let modulus = day_of_era % d;
                (from.date() + Duration::days((d - modulus) as i64)).and_hms(0, 0, 0)
            }
            Weeks(w) => {
                let d = from.date();
                let dow = d.weekday().num_days_from_monday();
                let start_of_week = d.clone() - Duration::days(dow as i64);
                let days_since_ever = d.num_days_from_ce();
                let week_num = (days_since_ever / 7) as u32;
                let modulus = week_num % w;
                (start_of_week + Duration::weeks((w - modulus) as i64)).and_hms(0, 0, 0)
            },
            _ => self.next(from)
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
    fn second(self) -> Interval { self.seconds() }
    fn minute(self) -> Interval { self.minutes() }
    fn hour(self) -> Interval { self.hours() }
    fn day(self) -> Interval { self.days() }
    fn week(self) -> Interval { self.weeks() }
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
    use Interval::*;
    use intervals::NextTime;
    use TimeUnits;
    use RunConfig;

    #[test]
    fn basic_units() {
        assert_eq!(Seconds(5), 5.seconds());
    }

    #[test]
    fn test_next() {
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();
        let next_dt = 5.seconds().next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:22:18-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 13.minutes().next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:35:13-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 2.hours().next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T16:22:13-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 2.days().next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-06T14:22:13-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 2.weeks().next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-18T14:22:13-00:00").unwrap();
        assert_eq!(next_dt, expected);
    }

    #[test]
    fn test_next_start() {
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();

        let next_dt = 5.seconds().next_start(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:22:15-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = 5.seconds().next_start(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:22:20-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 15.minutes().next_start(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:30:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = 15.minutes().next_start(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T14:45:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 2.hours().next_start(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T16:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = 2.hours().next_start(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T18:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 2.days().next_start(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-05T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = 2.days().next_start(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-07T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = 2.weeks().next_start(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-10T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = 2.weeks().next_start(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-24T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = Monday.next_start(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-10T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = Monday.next_start(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-17T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let next_dt = Wednesday.next_start(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-05T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = Wednesday.next_start(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-12T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let friday = Friday.next_start(&dt);
        let next_dt = Weekday.next_start(&friday);
        let expected = DateTime::parse_from_rfc3339("2018-09-10T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
        let next_dt = Weekday.next_start(&expected);
        let expected = DateTime::parse_from_rfc3339("2018-09-11T00:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
    }

    use super::parse_time;
    #[test]
    fn test_parse_time() {
        assert_eq!(parse_time("14:52:13"), Some(NaiveTime::from_hms(14, 52, 13)));
        assert_eq!(parse_time("2:52:13 pm"), Some(NaiveTime::from_hms(14, 52, 13)));
        assert_eq!(parse_time("14:52"), Some(NaiveTime::from_hms(14, 52, 0)));
        assert_eq!(parse_time("2:52 PM"), Some(NaiveTime::from_hms(14, 52, 0)));
    }

    // This doesn't work yet
    // #[test]
    fn test_run_config() {
        let rc = RunConfig::from_interval(Tuesday).with_time("15:00");
        let dt = DateTime::parse_from_rfc3339("2018-09-04T14:22:13-00:00").unwrap();
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-04T15:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);

        let rc = RunConfig::from_interval(Tuesday).with_time("14:00");
        let next_dt = rc.next(&dt);
        let expected = DateTime::parse_from_rfc3339("2018-09-11T14:00:00-00:00").unwrap();
        assert_eq!(next_dt, expected);
    }
}
