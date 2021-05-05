use std::{fmt, future::Future, pin::Pin};

use chrono::{DateTime, Local, TimeZone};

use crate::{
    job::Job,
    job_schedule::{JobSchedule, WithSchedule},
    timeprovider::{ChronoTimeProvider, TimeProvider},
    Interval,
};

pub type JobFuture = Box<dyn Future<Output = ()> + Send + 'static>;
/// An asynchronous job to run on the scheduler.
/// Create these by calling [`AsyncScheduler::every()`](crate::AsyncScheduler::every).
///
/// Methods for scheduling the job live in the [Job] trait.
pub struct AsyncJob<Tz = Local, Tp = ChronoTimeProvider>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    schedule: JobSchedule<Tz, Tp>,
    job: Option<Box<dyn GiveMeAPinnedFuture + Send>>,
}

trait GiveMeAPinnedFuture {
    fn get_pinned(&mut self) -> Pin<JobFuture>;
}

struct JobWrapper<F, T>
where
    F: FnMut() -> T,
    T: Future,
{
    f: F,
}

impl<F, T> JobWrapper<F, T>
where
    F: FnMut() -> T,
    T: Future,
{
    fn new(f: F) -> Self {
        JobWrapper { f }
    }
}

impl<F, T> GiveMeAPinnedFuture for JobWrapper<F, T>
where
    F: FnMut() -> T,
    T: Future<Output = ()> + Send + 'static,
{
    fn get_pinned(&mut self) -> Pin<JobFuture> {
        Box::pin((self.f)())
    }
}

impl<Tz, Tp> WithSchedule<Tz, Tp> for AsyncJob<Tz, Tp>
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

impl<Tz, Tp> fmt::Debug for AsyncJob<Tz, Tp>
where
    Tz: TimeZone,
    Tp: TimeProvider,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.schedule.fmt(f)
    }
}

impl<Tz, Tp> Job<Tz, Tp> for AsyncJob<Tz, Tp>
where
    Tz: TimeZone + Sync + Send,
    Tp: TimeProvider,
{
}

impl<Tz, Tp> AsyncJob<Tz, Tp>
where
    Tz: chrono::TimeZone + Sync + Send,
    Tp: TimeProvider,
{
    pub(crate) fn new(ival: Interval, tz: Tz) -> Self {
        AsyncJob {
            schedule: JobSchedule::new(ival, tz),
            job: None,
        }
    }

    /// Specify a task to run, and schedule its next run
    ///
    /// The function passed into this method should return a value implementing `Future<Output = ()>`.
    pub fn run<F, T>(&mut self, f: F) -> &mut Self
    where
        F: 'static + FnMut() -> T + Send,
        T: 'static + Future<Output = ()> + Send,
    {
        self.job = Some(Box::new(JobWrapper::new(f)));
        self.schedule.start_schedule();
        self
    }

    /// Run a task and re-schedule it. This is usually only called by
    /// [AsyncScheduler::run_pending()](crate::AsyncScheduler::run_pending).
    pub fn execute(&mut self, now: &DateTime<Tz>) -> Option<Pin<JobFuture>> {
        // Don't do anything if we're run out of runs
        if !self.schedule.can_run_again() {
            return None;
        }
        let rv = self.job.as_mut().map(|f| f.get_pinned());
        self.schedule.schedule_next(now);
        rv
    }
}
