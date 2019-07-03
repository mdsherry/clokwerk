use std::default::Default;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use Interval;
use Job;

/// Job scheduler
#[derive(Debug)]
pub struct Scheduler<Tz = chrono::Local>
where
    Tz: chrono::TimeZone,
{
    jobs: Vec<Job<Tz>>,
    tz: Tz
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler::<chrono::Local> { jobs: vec![], tz: chrono::Local }
    }
}

impl Scheduler {
    /// Create a new scheduler. Dates and times will be interpretted using the local timezone
    pub fn new() -> Self {
        Scheduler::default()
    }


    /// Create a new scheduler. Dates and times will be interpretted using the specified
    pub fn with_tz<Tz: chrono::TimeZone>(tz: Tz) -> Scheduler<Tz> {
        Scheduler { jobs: vec![], tz }
    }
}

impl<Tz> Scheduler<Tz> where 
    Tz: chrono::TimeZone + Sync + Send {
    
        

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
    pub fn every(&mut self, ival: Interval) -> &mut Job<Tz> {
        let job = Job::<Tz>::new(ival, self.tz.clone());
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

impl<Tz> Scheduler<Tz> where 
    Tz: chrono::TimeZone + Sync + Send + 'static,
    <Tz as chrono::TimeZone>::Offset: Send {

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
    // use super::Scheduler;

    // use std::thread;
    // use std::time::Duration;
    // use *;

    // These tests don't actually pass or fail; some of them could be rewritten to, but others are a bit too finicky

    // #[test]
    // fn test_something() {
    //     let mut scheduler = Scheduler::new();
    //     scheduler
    //         .every(10.minutes())
    //         .plus(5.seconds())
    //         .run(|| println!("I'm running!"));
    //     scheduler
    //         .every(3.days())
    //         .at("15:23")
    //         .run(|| println!("I'm running!"));
    //     println!("{:?}", scheduler);
    //     scheduler.run_pending();
    //     println!("{:?}", scheduler);

    //     assert!(false);
    // }

    // #[test]
    // fn test_something_else() {
    //     let mut scheduler = Scheduler::with_tz(chrono::Utc);
    //     scheduler
    //         .every(5.seconds())
    //         .and_every(2.seconds())
    //         .run(|| println!("Running!"));
    //     let handle = scheduler.watch_thread(Duration::from_millis(100));
    //     thread::sleep(Duration::from_secs(7));
    //     handle.stop();
    //     thread::sleep(Duration::from_secs(7));
    // }

    // #[test]
    // fn test_specific_time() {
    //     let mut scheduler = Scheduler::with_tz(chrono::Utc);
    //     scheduler.every(crate::intervals::Interval::Wednesday).at("3:57 AM").run(|| println!("UTC scheduling works"));
    //     let handle = scheduler.watch_thread(Duration::from_millis(100));
    //     thread::sleep(Duration::from_secs(60));
    //     handle.stop();
    //     thread::sleep(Duration::from_secs(60));
    // }
}
