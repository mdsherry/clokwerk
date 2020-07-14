use crate::{
    intervals::ClokwerkTime,
    timeprovider::{ChronoTimeProvider, TimeProvider},
};
use chrono::prelude::*;
use intervals::NextTime;
use std::fmt::{self, Debug};
use std::{convert::TryInto, marker::PhantomData};
use Interval;
use RunConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunCount {
    Never,
    Times(usize),
    Forever,
}

#[derive(Debug, Clone)]
struct RepeatConfig {
    repeats: usize,
    repeat_interval: Interval,
    repeats_left: usize,
}

/// A job to run on the scheduler.
/// Create these by calling [`Scheduler::every()`](::Scheduler::every).
pub struct Job<Tz = Local, Tp = ChronoTimeProvider>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    frequency: Vec<RunConfig>,
    next_run: Option<DateTime<Tz>>,
    last_run: Option<DateTime<Tz>>,
    job: Option<Box<dyn FnMut() + Send>>,
    run_count: RunCount,
    repeat_config: Option<RepeatConfig>,
    tz: Tz,
    _tp: PhantomData<Tp>,
}

impl<Tz, Tp> fmt::Debug for Job<Tz, Tp>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Job")
            .field("frequency", &self.frequency)
            .field("next_run", &self.next_run)
            .field("last_run", &self.last_run)
            .field("run_count", &self.run_count)
            .field("repeat_config", &self.repeat_config)
            .finish()
    }
}

impl<Tz, Tp> Job<Tz, Tp>
where
    Tz: chrono::TimeZone + Sync + Send,
    Tp: TimeProvider,
{
    pub(crate) fn new(ival: Interval, tz: Tz) -> Self {
        Job {
            frequency: vec![RunConfig::from_interval(ival)],
            next_run: None,
            last_run: None,
            job: None,
            run_count: RunCount::Forever,
            repeat_config: None,
            tz,
            _tp: PhantomData,
        }
    }

    fn last_frequency(&mut self) -> &mut RunConfig {
        let last_idx = self.frequency.len() - 1;
        &mut self.frequency[last_idx]
    }

    /// Specify the time of day when a task should run, e.g.
    /// ```rust
    /// # extern crate clokwerk;
    /// # extern crate chrono;
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # use chrono::NaiveTime;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(1.day()).at("14:32").run(|| println!("Tea time!"));
    /// scheduler.every(Wednesday).at("6:32:21 PM").run(|| println!("Writing examples is hard"));
    /// scheduler.every(Weekday).at(NaiveTime::from_hms(23, 42, 16)).run(|| println!("Also works with NaiveTime"));
    /// ```
    /// Times can be specified using strings, with or without seconds, and in either 24-hour or 12-hour time.
    /// They can also be any other type that implements `TryInto<ClokwerkTime>`, which includes [`chrono::NaiveTime`].
    /// This method will panic if TryInto fails, e.g. because the time string could not be parsed.
    /// If the value comes from an untrusted source, e.g. user input, [`Job::try_at`] will return a result instead.
    ///
    /// This method is mutually exclusive with [`Job::plus()`].
    pub fn at<T>(&mut self, time: T) -> &mut Self
    where
        T: TryInto<ClokwerkTime>,
        T::Error: Debug,
    {
        self.try_at(time)
            .expect("Could not convert value into a time")
    }

    /// Identical to [`Job::at`] except that it returns a Result instead of panicking if the conversion failed.
    /// ```rust
    /// # extern crate clokwerk;
    /// # extern crate chrono;
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(1.day()).try_at("14:32")?.run(|| println!("Tea time!"));
    /// # Ok::<(), chrono::ParseError>(())
    /// ```
    /// Times can be specified with or without seconds, and in either 24-hour or 12-hour time.
    /// Mutually exclusive with [`Job::plus()`].
    pub fn try_at<T>(&mut self, time: T) -> Result<&mut Self, T::Error>
    where
        T: TryInto<ClokwerkTime>,
    {
        {
            let frequency = self.last_frequency();
            *frequency = frequency.with_time(time.try_into()?);
        }
        Ok(self)
    }

    /// Add additional precision time to when a task should run, e.g.
    /// ```rust
    /// # extern crate clokwerk;
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(1.day())
    ///     .plus(6.hours())
    ///     .plus(13.minutes())
    ///   .run(|| println!("Time to wake up!"));
    /// ```
    /// Mutually exclusive with [`Job::at()`].
    pub fn plus(&mut self, ival: Interval) -> &mut Self {
        {
            let frequency = self.last_frequency();
            *frequency = frequency.with_subinterval(ival);
        }
        self
    }

    /// Add an additional scheduling to the task. All schedules will be considered when determining
    /// when the task should next run.
    pub fn and_every(&mut self, ival: Interval) -> &mut Self {
        self.frequency.push(RunConfig::from_interval(ival));
        self
    }

    /// Execute the job only once. Equivalent to `_.count(1)`.
    pub fn once(&mut self) -> &mut Self {
        self.run_count = RunCount::Times(1);
        self
    }

    /// Execute the job forever. This is the default behaviour.
    pub fn forever(&mut self) -> &mut Self {
        self.run_count = RunCount::Forever;
        self
    }

    /// Execute the job only `count` times.
    pub fn count(&mut self, count: usize) -> &mut Self {
        self.run_count = RunCount::Times(count);
        self
    }

    fn next_run_time(&self, now: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        match self.run_count {
            RunCount::Never => None,
            _ => self.frequency.iter().map(|freq| freq.next(now)).min(),
        }
    }

    /// After running once, run again with the specified interval.
    ///
    /// ```rust
    /// # extern crate clokwerk;
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # fn hit_snooze() {}
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(Weekday)
    ///   .at("7:40")
    ///   .repeating_every(10.minutes())
    ///   .times(5)
    ///   .run(|| hit_snooze());
    /// ```
    /// will hit snooze five times every morning, at 7:40, 7:50, 8:00, 8:10 and 8:20.
    ///
    /// Unlike [`Job::at`] and [`Job::plus`],
    /// this affects all intervals associated with the job, not just the most recent one.
    /// ```rust
    /// # extern crate clokwerk;
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # fn hit_snooze() {}
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(Weekday)
    ///   .at("7:40")
    ///   .and_every(Saturday)
    ///   .at("9:15")
    ///   .and_every(Sunday)
    ///   .at("9:15")
    ///   .repeating_every(10.minutes())
    ///   .times(5)
    ///   .run(|| hit_snooze());
    /// ```
    /// hits snooze five times every day, not just Sundays.
    ///
    /// If a job is still repeating, it will ignore otherwise scheduled runs.
    /// ```rust
    /// # extern crate clokwerk;
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # fn hit_snooze() {}
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(1.hour())
    ///   .repeating_every(45.minutes())
    ///   .times(3)
    ///   .run(|| println!("Hello"));
    /// ```
    /// If this is scheduled to run at 6 AM, it will print `Hello` at 6:00, 6:45, and 7:30, and then again at 8:00, 8:45, 9:30, etc.
    pub fn repeating_every(&mut self, interval: Interval) -> Repeating<Tz, Tp> {
        Repeating {
            job: self,
            interval,
        }
    }

    /// Specify a task to run, and schedule its next run
    pub fn run<F>(&mut self, f: F) -> &mut Self
    where
        F: 'static + FnMut() + Send,
    {
        self.job = Some(Box::new(f));
        if let None = self.next_run {
            let now = Tp::now(&self.tz);
            self.next_run = self.next_run_time(&now);
            match &mut self.repeat_config {
                Some(RepeatConfig {
                    repeats,
                    repeats_left,
                    ..
                }) => {
                    *repeats_left = *repeats;
                }
                None => (),
            }
        }
        self
    }

    /// Test whether a job is scheduled to run again. This is usually only called by
    /// [Scheduler::run_pending()](::Scheduler::run_pending).
    pub fn is_pending(&self, now: &DateTime<Tz>) -> bool {
        match &self.next_run {
            Some(dt) => *dt <= *now,
            None => false,
        }
    }

    /// Run a task and re-schedule it. This is usually only called by
    /// [Scheduler::run_pending()](::Scheduler::run_pending).
    pub fn execute(&mut self, now: &DateTime<Tz>) {
        // Don't do anything if we're run out of runs
        if self.run_count == RunCount::Never {
            return;
        }
        if let Some(ref mut f) = self.job {
            f();
        }

        // We compute this up front since we can't borrow self immutably while doing this next bit
        let next_run_time = self.next_run_time(now);
        match &mut self.repeat_config {
            Some(RepeatConfig {
                repeats,
                repeats_left,
                repeat_interval,
            }) => {
                if *repeats_left > 0 {
                    *repeats_left -= 1;
                    // Normal scheduling is aligned with the day: if you ask for something every hour, it will
                    // run at the start of the next hour, not one hour after being scheduled.
                    // For repeats, though, we want them aligned the first run of the repeats.
                    // This means we want to align with with next_run (which should be not far in the past),
                    // or if it's somehow unavailable, the current time.
                    // It's possible that we're really far behind. If so, find the next repeat interval that's
                    // still in the future (relative to when we start this run.)
                    let mut next = self.next_run.as_ref().unwrap_or(now).clone();
                    loop {
                        next = repeat_interval.next_from(&next);
                        if next > *now {
                            break;
                        }
                    }
                    self.next_run = Some(next);
                } else {
                    self.next_run = next_run_time;
                    *repeats_left = *repeats;
                }
            }
            None => self.next_run = next_run_time,
        }

        self.last_run = Some(now.clone());
        self.run_count = match self.run_count {
            RunCount::Never => RunCount::Never,
            RunCount::Times(n) if n > 1 => RunCount::Times(n - 1),
            RunCount::Times(_) => RunCount::Never,
            RunCount::Forever => RunCount::Forever,
        };
    }
}

pub struct Repeating<'a, Tz: chrono::TimeZone, Tp: TimeProvider> {
    job: &'a mut Job<Tz, Tp>,
    interval: Interval,
}

impl<'a, Tz, Tp> Repeating<'a, Tz, Tp>
where
    Tz: chrono::TimeZone + Sync + Send,
    Tp: TimeProvider,
{
    /// Indicate the number of additoinal times the job should be run every time it's scheduled.
    /// Passing a value of 0 here is the same as not specifying a repeat at all.
    pub fn times(self, n: usize) -> &'a mut Job<Tz, Tp> {
        if n >= 1 {
            self.job.repeat_config = Some(RepeatConfig {
                repeats: n,
                repeat_interval: self.interval,
                repeats_left: 0,
            });
        }
        self.job
    }
}

#[cfg(test)]
mod test {
    use super::Job;
    use crate::{intervals::*, timeprovider::TimeProvider};
    use chrono::prelude::*;

    #[test]
    fn test_repeating() {
        fn utc_hms(h: u32, m: u32, s: u32) -> DateTime<Utc> {
            Utc.from_utc_datetime(&NaiveDate::from_ymd(2020, 6, 16).and_hms(h, m, s))
        }
        struct TestTimeProvider;
        impl TimeProvider for TestTimeProvider {
            fn now<Tz>(tz: &Tz) -> chrono::DateTime<Tz>
            where
                Tz: chrono::TimeZone + Sync + Send,
            {
                utc_hms(7, 58, 0).with_timezone(tz)
            }
        }
        let mut job = Job::<Utc, TestTimeProvider>::new(1.hour(), Utc);
        job.repeating_every(45.minutes()).times(2).run(|| ());

        assert!(!job.is_pending(&utc_hms(7, 59, 0)));
        assert!(job.is_pending(&utc_hms(8, 0, 0)));
        job.execute(&utc_hms(8, 0, 0));
        println!("{:?}", job.next_run);

        assert!(!job.is_pending(&utc_hms(8, 44, 0)));
        assert!(job.is_pending(&utc_hms(8, 45, 0)));
        job.execute(&utc_hms(8, 45, 0));

        // Skips 9:00 because it's still repeating at 45 minutes
        assert!(!job.is_pending(&utc_hms(9, 0, 0)));

        assert!(!job.is_pending(&utc_hms(9, 29, 0)));
        assert!(job.is_pending(&utc_hms(9, 30, 0)));
        job.execute(&utc_hms(9, 30, 0));

        assert!(!job.is_pending(&utc_hms(9, 59, 0)));
        assert!(job.is_pending(&utc_hms(10, 0, 0)));
    }
}
