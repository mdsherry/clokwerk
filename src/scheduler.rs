use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use Interval;
use std::sync::Arc;
use Job;

#[derive(Debug)]
pub struct Scheduler {
    jobs: Vec<Job>,
}
impl Scheduler {
    pub fn new() -> Self {
        Scheduler { jobs: vec![] }
    }
    pub fn every(&mut self, ival: Interval) -> &mut Job {
        let job = Job::new(ival);
        self.jobs.push(job);
        let last_index = self.jobs.len() - 1;
        &mut self.jobs[last_index]
    }

    pub fn run_pending(&mut self) {
        for job in &mut self.jobs {
            if job.is_pending() {
                job.execute();
            }
        }
    }

    pub fn watch_thread(self) -> ScheduleHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let my_stop = stop.clone();
        let mut me = self;
        let handle = thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                me.run_pending();
                thread::sleep(Duration::from_millis(500));
            }
        });
        ScheduleHandle {
            stop: my_stop,
            thread_handle: Some(handle),
        }
    }
}

pub struct ScheduleHandle {
    stop: Arc<AtomicBool>,
    thread_handle: Option<thread::JoinHandle<()>>,
}
impl ScheduleHandle {
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
    // use *;
    // use std::thread;
    // use std::time::Duration;

    // // #[test]
    // fn test_something() {
    //     let mut scheduler = Scheduler::new();
    //     scheduler
    //         .every(10.minutes())
    //         .and(5.seconds())
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
    //     let mut scheduler = Scheduler::new();
    //     scheduler.every(5.seconds()).run(|| println!("Running!"));
    //     let handle = scheduler.watch_thread();
    //     thread::sleep(Duration::from_secs(7));
    //     handle.stop();
    //     thread::sleep(Duration::from_secs(7));
    // }
}
