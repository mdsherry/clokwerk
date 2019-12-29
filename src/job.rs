use crate::timeprovider::{ChronoTimeProvider, TimeProvider};
use chrono::prelude::*;
use intervals::NextTime;
use std::fmt;
use std::marker::PhantomData;
use Interval;
use RunConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunCount {
    Never,
    Times(usize),
    Forever,
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
            run_count: RunCount::Forever,
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

    /// Specify a task to run, and schedule its next run
    pub fn run<F>(&mut self, f: F) -> &mut Self
    where
        F: 'static + FnMut() + Send,
    {
        self.job = Some(Box::new(f));
        match self.next_run {
            Some(_) => (),
            None => {
                let now = Tp::now(&self.tz);
                self.next_run = self.next_run_time(&now);
            }
        };
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
        self.last_run = Some(now.clone());
        self.next_run = self.next_run_time(now);
        self.run_count = match self.run_count {
            RunCount::Never => RunCount::Never,
            RunCount::Times(n) if n > 1 => RunCount::Times(n - 1),
            RunCount::Times(_) => RunCount::Never,
            RunCount::Forever => RunCount::Forever,
        }
    }
}
