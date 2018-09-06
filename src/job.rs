use chrono::prelude::*;
use std::fmt;
use RunConfig;
use Interval;
use NextTime;

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

    pub fn at(&mut self, s: &str) -> &mut Self {
        self.frequency = self.frequency.with_time(s);
        self
    }

    pub fn and(&mut self, ival: Interval) -> &mut Self {
        self.frequency = self.frequency.with_subinterval(ival);
        self
    }

    pub fn run<F>(&mut self, f: F) -> &mut Self
    where
        F: 'static + FnMut() + Sync + Send,
    {
        self.job = Some(Box::new(f));
        self
    }

    pub fn is_pending(&mut self) -> bool {
        let now = Local::now();
        match self.next_run {
            Some(dt) => dt <= now,
            None => {
                self.next_run = Some(self.frequency.next(&now));
                false
            }
        }
    }

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