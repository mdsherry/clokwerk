use crate::Interval;
use crate::{
    job::Job,
    job_schedule::{JobSchedule, WithSchedule},
};

use crate::timeprovider::{ChronoTimeProvider, TimeProvider};
use chrono::prelude::*;
use std::fmt;

/// A job to run on the scheduler.
/// Create these by calling [`Scheduler::every()`](crate::Scheduler::every).
///
/// Methods for scheduling the job live in the [Job] trait.
pub struct SyncJob<Tz = Local, Tp = ChronoTimeProvider>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    schedule: JobSchedule<Tz, Tp>,
    job: Option<Box<dyn FnMut() + Send>>,
}

impl<Tz, Tp> WithSchedule<Tz, Tp> for SyncJob<Tz, Tp>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    fn schedule_mut(&mut self) -> &mut JobSchedule<Tz, Tp> {
        &mut self.schedule
    }

    fn schedule(&self) -> &JobSchedule<Tz, Tp> {
        &self.schedule
    }
}

impl<Tz, Tp> fmt::Debug for SyncJob<Tz, Tp>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.schedule.fmt(f)
    }
}

impl<Tz, Tp> Job<Tz, Tp> for SyncJob<Tz, Tp>
where
    Tz: TimeZone + Sync + Send,
    Tp: TimeProvider,
{
}

impl<Tz, Tp> SyncJob<Tz, Tp>
where
    Tz: chrono::TimeZone + Sync + Send,
    Tp: TimeProvider,
{
    pub(crate) fn new(ival: Interval, tz: Tz) -> Self {
        SyncJob {
            schedule: JobSchedule::new(ival, tz),
            job: None,
        }
    }

    /// Specify a task to run, and schedule its next run
    pub fn run<F>(&mut self, f: F) -> &mut Self
    where
        F: 'static + FnMut() + Send,
    {
        self.job = Some(Box::new(f));
        self.schedule.start_schedule();
        self
    }

    /// Run a task and re-schedule it. This is usually only called by
    /// [Scheduler::run_pending()](crate::Scheduler::run_pending).
    pub fn execute(&mut self, now: &DateTime<Tz>) {
        // Don't do anything if we're run out of runs
        if !self.schedule.can_run_again() {
            return;
        }
        self.job.as_mut().map(|f| f());
        self.schedule.schedule_next(now);
    }
}
