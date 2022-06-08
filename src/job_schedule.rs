use std::{fmt, marker::PhantomData};

use chrono::{DateTime, Local, NaiveTime, TimeZone};

use crate::{
    intervals::{parse_time, RunConfig},
    timeprovider::{ChronoTimeProvider, TimeProvider},
    Interval, NextTime,
};

#[doc(hidden)]
pub trait WithSchedule<Tz, Tp>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    fn schedule_mut(&mut self) -> &mut JobSchedule<Tz, Tp>;
    fn schedule(&self) -> &JobSchedule<Tz, Tp>;
}

pub struct Repeating<'a, T, Tz, Tp> {
    job: &'a mut T,
    interval: Interval,
    _tz: PhantomData<Tz>,
    _tp: PhantomData<Tp>,
}

impl<'a, T, Tz, Tp> Repeating<'a, T, Tz, Tp>
where
    T: WithSchedule<Tz, Tp>,
    Tz: TimeZone,
    Tp: TimeProvider,
{
    pub(crate) fn new(job: &'a mut T, interval: Interval) -> Repeating<'a, T, Tz, Tp> {
        Self {
            job,
            interval,
            _tz: PhantomData,
            _tp: PhantomData,
        }
    }
    /// Indicate the number of additoinal times the job should be run every time it's scheduled.
    /// Passing a value of 0 here is the same as not specifying a repeat at all.
    pub fn times(self, n: usize) -> &'a mut T {
        if n >= 1 {
            self.job.schedule_mut().repeat_config = Some(RepeatConfig {
                repeats: n,
                repeat_interval: self.interval,
                repeats_left: 0,
            });
        }
        self.job
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunCount {
    Never,
    Times(usize),
    Forever,
}

#[derive(Debug, Clone)]
pub(crate) struct RepeatConfig {
    repeats: usize,
    repeat_interval: Interval,
    repeats_left: usize,
}

pub struct JobSchedule<Tz = Local, Tp = ChronoTimeProvider>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    frequency: Vec<RunConfig>,
    next_run: Option<DateTime<Tz>>,
    last_run: Option<DateTime<Tz>>,
    run_count: RunCount,
    repeat_config: Option<RepeatConfig>,
    tz: Tz,
    _tp: PhantomData<Tp>,
}

impl<Tz, Tp> fmt::Debug for JobSchedule<Tz, Tp>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("JobSchedule")
            .field("frequency", &self.frequency)
            .field("next_run", &self.next_run)
            .field("last_run", &self.last_run)
            .field("run_count", &self.run_count)
            .field("repeat_config", &self.repeat_config)
            .finish()
    }
}

impl<Tz, Tp> JobSchedule<Tz, Tp>
where
    Tz: chrono::TimeZone + Sync + Send,
    Tp: TimeProvider,
{
    pub(crate) fn new(ival: Interval, tz: Tz) -> Self {
        Self {
            frequency: vec![RunConfig::from_interval(ival)],
            next_run: None,
            last_run: None,
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

    pub fn at(&mut self, time: &str) -> &mut Self {
        self.try_at(time)
            .expect("Could not convert value into a time")
    }

    pub fn try_at(&mut self, time: &str) -> Result<&mut Self, chrono::ParseError> {
        Ok(self.at_time(parse_time(time)?))
    }

    pub fn at_time(&mut self, time: NaiveTime) -> &mut Self {
        {
            let frequency = self.last_frequency();
            *frequency = frequency.with_time(time);
        }
        self
    }

    pub fn plus(&mut self, ival: Interval) -> &mut Self {
        {
            let frequency = self.last_frequency();
            *frequency = frequency.with_subinterval(ival);
        }
        self
    }

    pub fn and_every(&mut self, ival: Interval) -> &mut Self {
        self.frequency.push(RunConfig::from_interval(ival));
        self
    }

    pub fn once(&mut self) -> &mut Self {
        self.run_count = RunCount::Times(1);
        self
    }

    pub fn forever(&mut self) -> &mut Self {
        self.run_count = RunCount::Forever;
        self
    }

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

    /// Has this job exhausted its runs?
    pub fn can_run_again(&self) -> bool {
        self.run_count != RunCount::Never
    }

    /// Specify a task to run, and schedule its next run
    pub fn start_schedule(&mut self) -> &mut Self {
        if self.next_run.is_none() {
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
    pub fn schedule_next(&mut self, now: &DateTime<Tz>) {
        // Don't do anything if we're run out of runs
        if self.run_count == RunCount::Never {
            return;
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

#[cfg(test)]
mod test {
    use super::JobSchedule;
    use crate::{intervals::*, timeprovider::TimeProvider, Job, SyncJob};
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
        let mut job = SyncJob::<Utc, TestTimeProvider>::new(1.hour(), Utc);
        job.repeating_every(45.minutes()).times(2);
        job.run(|| {});

        assert!(!job.is_pending(&utc_hms(7, 59, 0)));
        assert!(job.is_pending(&utc_hms(8, 0, 0)));
        job.execute(&utc_hms(8, 0, 0));

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

    #[test]
    fn test_time_coercion() {
        let mut job = JobSchedule::<Utc>::new(1.day(), Utc);
        // &str
        job.try_at("12:32").unwrap();
        // &String
        job.try_at(&format!("{}:{}", 12, 32)).unwrap();
        // NaiveTime
        job.at_time(NaiveTime::from_hms(12, 32, 0));
    }
}
