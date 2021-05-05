use crate::Interval;
use crate::SyncJob;
use crate::{
    timeprovider::{ChronoTimeProvider, TimeProvider},
    Job,
};
use std::default::Default;
use std::marker::PhantomData;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
/// Synchronous job scheduler
///
/// ### Usage examples
/// ```rust
/// // Scheduler, trait for .seconds(), .minutes(), etc., and trait with job scheduling methods
/// use clokwerk::{Scheduler, TimeUnits, Job};
/// // Import week days and WeekDay
/// use clokwerk::Interval::*;
/// use std::thread;
/// use std::time::Duration;
///
/// // Create a new scheduler
/// let mut scheduler = Scheduler::new();
/// // or a scheduler with a given timezone
/// let mut scheduler = Scheduler::with_tz(chrono::Utc);
/// // Add some tasks to it
/// scheduler
///     .every(10.minutes())
///         .plus(30.seconds())
///     .run(|| println!("Periodic task"));
/// scheduler
///     .every(1.day())
///         .at("3:20 pm")
///     .run(|| println!("Daily task"));
/// scheduler
///     .every(Wednesday)
///         .at("14:20:17")
///     .run(|| println!("Weekly task"));
/// scheduler
///     .every(Tuesday)
///         .at("14:20:17")
///     .and_every(Thursday)
///         .at("15:00")
///     .run(|| println!("Biweekly task"));
/// scheduler
///     .every(Weekday)
///     .run(|| println!("Every weekday at midnight"));
/// scheduler
///     .every(1.day())
///         .at("3:20 pm")
///     .run(|| println!("I only run once")).once();
/// scheduler
///     .every(Weekday)
///         .at("12:00").count(10)
///     .run(|| println!("Countdown"));
/// scheduler
///     .every(1.day())
///         .at("10:00 am")
///         .repeating_every(30.minutes())
///             .times(6)
///     .run(|| println!("I run every half hour from 10 AM to 1 PM inclusive."));
/// scheduler
///     .every(1.day())
///         .at_time(chrono::NaiveTime::from_hms(13, 12, 14))
///     .run(|| println!("You can also pass chrono::NaiveTimes to `at_time`."));
///
/// // Manually run the scheduler in an event loop
/// for _ in 1..10 {
///     scheduler.run_pending();
///     thread::sleep(Duration::from_millis(10));
///     # break;
/// }
///
/// // Or run it in a background thread
/// let thread_handle = scheduler.watch_thread(Duration::from_millis(100));
/// // The scheduler stops when `thread_handle` is dropped, or `stop` is called
/// thread_handle.stop();
/// ```
#[derive(Debug)]
pub struct Scheduler<Tz = chrono::Local, Tp = ChronoTimeProvider>
where
    Tz: chrono::TimeZone,
    Tp: TimeProvider,
{
    jobs: Vec<SyncJob<Tz, Tp>>,
    tz: Tz,
    _tp: PhantomData<Tp>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler::<chrono::Local> {
            jobs: vec![],
            tz: chrono::Local,
            _tp: PhantomData,
        }
    }
}

impl Scheduler {
    /// Create a new scheduler. Dates and times will be interpretted using the local timezone
    pub fn new() -> Self {
        Scheduler::default()
    }

    /// Create a new scheduler. Dates and times will be interpretted using the specified timezone.
    pub fn with_tz<Tz: chrono::TimeZone>(tz: Tz) -> Scheduler<Tz> {
        Scheduler {
            jobs: vec![],
            tz,
            _tp: PhantomData,
        }
    }

    /// Create a new scheduler. Dates and times will be interpretted using the specified timezone.
    /// In addition, you can provide an alternate time provider. This is mostly useful for writing
    /// tests.
    pub fn with_tz_and_provider<Tz: chrono::TimeZone, Tp: TimeProvider>(
        tz: Tz,
    ) -> Scheduler<Tz, Tp> {
        Scheduler {
            jobs: vec![],
            tz,
            _tp: PhantomData,
        }
    }
}

impl<Tz, Tp> Scheduler<Tz, Tp>
where
    Tz: chrono::TimeZone + Sync + Send,
    Tp: TimeProvider,
{
    /// Add a new job to the scheduler to be run on the given interval
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(10.minutes()).plus(30.seconds()).run(|| println!("Periodic task"));
    /// scheduler.every(1.day()).at("3:20 pm").run(|| println!("Daily task"));
    /// scheduler.every(Wednesday).at("14:20:17").run(|| println!("Weekly task"));
    /// scheduler.every(Weekday).run(|| println!("Every weekday at midnight"));
    /// ```
    pub fn every(&mut self, ival: Interval) -> &mut SyncJob<Tz, Tp> {
        let job = SyncJob::<Tz, Tp>::new(ival, self.tz.clone());
        self.jobs.push(job);
        let last_index = self.jobs.len() - 1;
        &mut self.jobs[last_index]
    }

    /// Run all jobs that should run at this time.
    ///
    /// This method blocks while jobs are being run. If a job takes a long time, it may prevent
    /// other tasks from running as scheduled. If you have a long-running task, you might consider
    /// having the job move the work into another thread so that it can return promptly.
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// use std::thread;
    /// use std::time::Duration;
    /// # let mut scheduler = Scheduler::new();
    /// loop {
    ///     scheduler.run_pending();
    ///     thread::sleep(Duration::from_millis(100));
    ///     # break
    /// }
    /// ```
    pub fn run_pending(&mut self) {
        let now = Tp::now(&self.tz);
        for job in &mut self.jobs {
            if job.is_pending(&now) {
                job.execute(&now);
            }
        }
    }
}

impl<Tz> Scheduler<Tz>
where
    Tz: chrono::TimeZone + Sync + Send + 'static,
    <Tz as chrono::TimeZone>::Offset: Send,
{
    /// Start a background thread to call [Scheduler::run_pending()] repeatedly.
    /// The frequency argument controls how long the thread will sleep between calls
    /// to [Scheduler::run_pending()].
    /// If the returned [ScheduleHandle] is dropped, the resulting thread will end
    /// cleanly when [Scheduler::run_pending()] would have next been called.
    ///
    /// Passing large durations for `frequency` can cause long delays when [ScheduleHandle::stop()]
    /// is called, or the [ScheduleHandle] is dropped, as it waits for the thread to finish sleeping.
    /// This could affect how long it takes for the program to exit.
    ///
    /// Reasonable values for `frequency` would be between 100 ms and 10 seconds.
    /// If in doubt, choose a smaller value.
    #[must_use = "The scheduler is halted when the returned handle is dropped"]
    pub fn watch_thread(self, frequency: Duration) -> ScheduleHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let my_stop = stop.clone();
        let mut me = self;
        let handle = thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                me.run_pending();
                thread::sleep(frequency);
            }
        });
        ScheduleHandle {
            stop: my_stop,
            thread_handle: Some(handle),
        }
    }
}

/// Guard object for the scheduler background thread. The thread is terminated if this object
/// is dropped, or [ScheduleHandle::stop()] is called
pub struct ScheduleHandle {
    stop: Arc<AtomicBool>,
    thread_handle: Option<thread::JoinHandle<()>>,
}
impl ScheduleHandle {
    /// Halt the scheduler background thread
    pub fn stop(self) {}
}

impl Drop for ScheduleHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let handle = self.thread_handle.take();
        handle.unwrap().join().ok();
    }
}

#[cfg(test)]
mod tests {
    use super::{Job, Scheduler, TimeProvider};
    use crate::intervals::*;
    use std::sync::{atomic::AtomicU32, atomic::Ordering, Arc};

    macro_rules! make_time_provider {
        ($name:ident : $($time:literal),+) => {
            #[derive(Debug)]
            struct $name {}
            static TIMES_TIME_REQUESTED: once_cell::sync::Lazy<AtomicU32> = once_cell::sync::Lazy::new(|| AtomicU32::new(0));
            impl TimeProvider for $name {
                fn now<Tz>(tz: &Tz) -> chrono::DateTime<Tz>
                where
                    Tz: chrono::TimeZone + Sync + Send,
                    {
                        let times = [$(chrono::DateTime::parse_from_rfc3339($time).unwrap()),+];
                        let idx = TIMES_TIME_REQUESTED.fetch_add(1, Ordering::SeqCst) as usize;
                        times[idx].with_timezone(&tz)
                    }
            }
        };
    }

    #[test]
    fn test_every_plus() {
        make_time_provider!(FakeTimeProvider :
            "2019-10-22T12:40:00Z",
            "2019-10-22T12:40:00Z",
            "2019-10-22T12:50:20Z",
            "2019-10-22T12:50:30Z"
        );
        let mut scheduler =
            Scheduler::with_tz_and_provider::<chrono::Utc, FakeTimeProvider>(chrono::Utc);
        let times_called = Arc::new(AtomicU32::new(0));
        {
            let times_called = times_called.clone();
            scheduler
                .every(10.minutes())
                .plus(5.seconds())
                .run(move || {
                    times_called.fetch_add(1, Ordering::SeqCst);
                });
        }
        assert_eq!(1, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(0, times_called.load(Ordering::SeqCst));
        assert_eq!(2, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        assert_eq!(3, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        assert_eq!(4, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_every_at() {
        make_time_provider!(FakeTimeProvider:
            "2019-10-22T12:40:00Z",
            "2019-10-22T12:40:10Z",
            "2019-10-25T12:50:20Z",
            "2019-10-25T15:23:30Z",
            "2019-10-26T15:50:30Z"
        );
        let mut scheduler =
            Scheduler::with_tz_and_provider::<chrono::Utc, FakeTimeProvider>(chrono::Utc);
        let times_called = Arc::new(AtomicU32::new(0));
        {
            let times_called = times_called.clone();
            scheduler.every(3.days()).at("15:23").run(move || {
                times_called.fetch_add(1, Ordering::SeqCst);
            });
        }
        assert_eq!(1, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(0, times_called.load(Ordering::SeqCst));
        assert_eq!(2, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        assert_eq!(3, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        assert_eq!(4, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_every_and_every() {
        make_time_provider!(FakeTimeProvider:
            "2019-10-22T12:40:01Z",
            "2019-10-22T12:40:01Z",
            "2019-10-22T12:40:02Z",
            "2019-10-22T12:40:03Z",
            "2019-10-22T12:40:04Z",
            "2019-10-22T12:40:05Z",
            "2019-10-22T12:40:06Z"
        );
        let mut scheduler =
            Scheduler::with_tz_and_provider::<chrono::Utc, FakeTimeProvider>(chrono::Utc);
        let times_called = Arc::new(AtomicU32::new(0));
        {
            let times_called = times_called.clone();
            scheduler
                .every(5.seconds())
                .and_every(2.seconds())
                .run(move || {
                    times_called.fetch_add(1, Ordering::SeqCst);
                });
        }
        assert_eq!(1, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(2, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(0, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(3, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(4, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(5, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(2, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(6, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(3, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(7, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(4, times_called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_once() {
        make_time_provider!(FakeTimeProvider:
            "2019-10-22T12:40:01Z",
            "2019-10-22T12:40:01Z",
            "2019-10-22T12:40:02Z",
            "2019-10-22T12:40:03Z"
        );
        let mut scheduler =
            Scheduler::with_tz_and_provider::<chrono::Utc, FakeTimeProvider>(chrono::Utc);
        let times_called = Arc::new(AtomicU32::new(0));
        {
            let times_called = times_called.clone();
            scheduler.every(1.seconds()).once().run(move || {
                times_called.fetch_add(1, Ordering::SeqCst);
            });
        }
        assert_eq!(1, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(2, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(0, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(3, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(4, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(1, times_called.load(Ordering::SeqCst));
    }
}
