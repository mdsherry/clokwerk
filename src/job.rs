use crate::timeprovider::{ChronoTimeProvider, TimeProvider};
use chrono::prelude::*;
use intervals::NextTime;
use std::fmt;
use std::marker::PhantomData;
use Interval;
use RunConfig;

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
    job: Option<Box<dyn FnMut() + Sync + Send>>,
    tz: Tz,
    _tp: PhantomData<Tp>,
}

impl<Tz, Tp> fmt::Debug for Job<Tz, Tp>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Job {{ frequency: {:?}, next_run: {:?}, last_run: {:?}, job: ??? }}",
            self.frequency, self.next_run, self.last_run
        )
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
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(1.day()).at("14:32").run(|| println!("Tea time!"));
    /// scheduler.every(Wednesday).at("6:32:21 PM").run(|| println!("Writing examples is hard"));
    /// ```
    /// Times can be specified with or without seconds, and in either 24-hour or 12-hour time.
    /// Mutually exclusive with [`Job::plus()`].
    pub fn at(&mut self, s: &str) -> &mut Self {
        {
            let frequency = self.last_frequency();
            *frequency = frequency.with_time(s);
        }
        self
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

    /// Specify a task to run, and schedule its next run
    pub fn run<F>(&mut self, f: F) -> &mut Self
    where
        F: 'static + FnMut() + Sync + Send,
    {
        self.job = Some(Box::new(f));
        match self.next_run {
            Some(_) => (),
            None => {
                let now = Tp::now(&self.tz);
                self.next_run = self.frequency.iter().map(|freq| freq.next(&now)).min();
            }
        };
        self
    }

    /// Test whether a job is scheduled to run again. This is usually only called by
    /// [Scheduler::run_pending()](::Scheduler::run_pending).
    pub fn is_pending(&self) -> bool {
        let now = Tp::now(&self.tz);
        match &self.next_run {
            Some(dt) => *dt <= now,
            None => false,
        }
    }

    /// Run a task and re-schedule it. This is usually only called by
    /// [Scheduler::run_pending()](::Scheduler::run_pending).
    pub fn execute(&mut self) {
        let now = Tp::now(&self.tz);
        if let Some(ref mut f) = self.job {
            f();
        }
        self.last_run = Some(now.clone());
        self.next_run = self.frequency.iter().map(|freq| freq.next(&now)).min();
    }
}
