use chrono::prelude::*;
use std::fmt;
use RunConfig;
use Interval;
use intervals::NextTime;

/// A job to run on the scheduler.
/// Create these by calling [`Scheduler::every()`](::Scheduler::every). 
pub struct Job {
    frequency: RunConfig,
    next_run: Option<DateTime<Local>>,
    last_run: Option<DateTime<Local>>,
    job: Option<Box<FnMut() + Sync + Send>>,
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Job {{ frequency: {:?}, next_run: {:?}, last_run: {:?}, job: ??? }}",
            self.frequency, self.next_run, self.last_run
        )
    }
}

impl Job {
    pub(crate) fn new(ival: Interval) -> Self {
        Job {
            frequency: RunConfig::from_interval(ival),
            next_run: None,
            last_run: None,
            job: None,
        }
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
    /// Mutually exclusive with [`Job::and()`].
    pub fn at(&mut self, s: &str) -> &mut Self {
        self.frequency = self.frequency.with_time(s);
        self
    }

    /// Add additional precision time to when a task should run, e.g.
    /// ```rust
    /// # extern crate clokwerk;
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(1.day())
    ///     .and(6.hours())
    ///     .and(13.minutes())
    ///   .run(|| println!("Time to wake up!"));
    /// ```
    /// Mutually exclusive with [`Job::at()`].
    pub fn and(&mut self, ival: Interval) -> &mut Self {
        self.frequency = self.frequency.with_subinterval(ival);
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
                let now = Local::now();
                self.next_run = Some(self.frequency.next(&now)); 
            }
        };
        self
    }

    /// Test whether a job is scheduled to run again. This is usually only called by 
    /// [Scheduler::run_pending()](::Scheduler::run_pending).
    pub fn is_pending(&self) -> bool {
        let now = Local::now();
        match self.next_run {
            Some(dt) => dt <= now,
            None => false
        }
    }

    /// Run a task and re-schedule it. This is usually only called by 
    /// [Scheduler::run_pending()](::Scheduler::run_pending).
    pub fn execute(&mut self) {
        let now = Local::now();
        match self.job {
            Some(ref mut f) => f(),
            _ => ()
        };        
        self.last_run = Some(now.clone());
        self.next_run = Some(self.frequency.next(&now));
    }
}