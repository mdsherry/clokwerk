use crate::timeprovider::{ChronoTimeProvider, TimeProvider};
use std::default::Default;
use std::marker::PhantomData;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use Interval;
use Job;
/// Job scheduler
#[derive(Debug)]
pub struct Scheduler<Tz = chrono::Local, Tp = ChronoTimeProvider>
where
    Tz: chrono::TimeZone,
    Tp: TimeProvider,
{
    jobs: Vec<Job<Tz, Tp>>,
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

    /// Create a new scheduler. Dates and times will be interpretted using the specified
    pub fn with_tz<Tz: chrono::TimeZone>(tz: Tz) -> Scheduler<Tz> {
        Scheduler {
            jobs: vec![],
            tz,
            _tp: PhantomData,
        }
    }

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
    /// # extern crate clokwerk;
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(10.minutes()).plus(30.seconds()).run(|| println!("Periodic task"));
    /// scheduler.every(1.day()).at("3:20 pm").run(|| println!("Daily task"));
    /// scheduler.every(Wednesday).at("14:20:17").run(|| println!("Weekly task"));
    /// scheduler.every(Weekday).run(|| println!("Every weekday at midnight"));
    /// ```
    pub fn every(&mut self, ival: Interval) -> &mut Job<Tz, Tp> {
        let job = Job::<Tz, Tp>::new(ival, self.tz.clone());
        self.jobs.push(job);
        let last_index = self.jobs.len() - 1;
        &mut self.jobs[last_index]
    }

    /// Run all jobs that should run at this time.
    /// ```rust
    /// # extern crate clokwerk;
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
        for job in &mut self.jobs {
            if job.is_pending() {
                job.execute();
            }
        }
    }
}

impl<Tz> Scheduler<Tz>
where
    Tz: chrono::TimeZone + Sync + Send + 'static,
    <Tz as chrono::TimeZone>::Offset: Send,
{
    /// Start a background thread to call [Scheduler::run_pending()] with the specified frequency.
    /// The resulting thread fill end cleanly if the returned [ScheduleHandle] is dropped.
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
    use super::{Scheduler, TimeProvider};
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
            "2019-10-22T12:40:10Z",
            "2019-10-22T12:50:20Z",
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
        // We ask for the time to see if we should run it, and again when computing the next time to run
        assert_eq!(4, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        assert_eq!(5, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_every_at() {
        make_time_provider!(FakeTimeProvider:
            "2019-10-22T12:40:00Z",
            "2019-10-22T12:40:10Z",
            "2019-10-25T12:50:20Z",
            "2019-10-25T15:23:20Z",
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
        // We ask for the time to see if we should run it, and again when computing the next time to run
        assert_eq!(4, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        assert_eq!(5, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_every_and_every() {
        make_time_provider!(FakeTimeProvider:
            "2019-10-22T12:40:01Z",
            "2019-10-22T12:40:01Z",
            "2019-10-22T12:40:02Z",
            "2019-10-22T12:40:02Z",
            "2019-10-22T12:40:03Z",
            "2019-10-22T12:40:04Z",
            "2019-10-22T12:40:04Z",
            "2019-10-22T12:40:05Z",
            "2019-10-22T12:40:05Z",
            "2019-10-22T12:40:06Z",
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
        assert_eq!(4, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(5, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(1, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(7, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(2, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(9, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(3, times_called.load(Ordering::SeqCst));
        scheduler.run_pending();
        assert_eq!(11, TIMES_TIME_REQUESTED.load(Ordering::SeqCst));
        assert_eq!(4, times_called.load(Ordering::SeqCst));
    }
}
