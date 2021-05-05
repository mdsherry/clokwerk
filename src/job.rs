use crate::{job_schedule::{Repeating, WithSchedule}};

use crate::{Interval, timeprovider::TimeProvider};
use chrono::prelude::*;

/// This trait provides an abstraction over [`SyncJob`](crate::SyncJob) and [`AsyncJob`](crate::AsyncJob), covering all the methods relating to scheduling, rather than execution.
pub trait Job<Tz, Tp> : WithSchedule<Tz, Tp> + Sized where Tz: TimeZone + Sync + Send, Tp: TimeProvider {

    /// Specify the time of day when a task should run, e.g.
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # use chrono::NaiveTime;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(1.day()).at("14:32").run(|| println!("Tea time!"));
    /// scheduler.every(Wednesday).at("6:32:21 PM").run(|| println!("Writing examples is hard"));
    /// ```
    /// Times can be specified using strings, with or without seconds, and in either 24-hour or 12-hour time.
    /// They can also be any other type that implements `TryInto<ClokwerkTime>`, which includes [`chrono::NaiveTime`].
    /// This method will panic if TryInto fails, e.g. because the time string could not be parsed.
    /// If the value comes from an untrusted source, e.g. user input, [`Job::try_at`] will return a result instead.
    ///
    /// This method is mutually exclusive with [`Job::plus()`].
    fn at(&mut self, time: &str) -> &mut Self {
        self.schedule_mut().try_at(time)
            .expect("Could not convert value into a time");
        self
    }

    /// Identical to [`Job::at`] except that it returns a Result instead of panicking if the conversion failed.
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(1.day()).try_at("14:32")?.run(|| println!("Tea time!"));
    /// # Ok::<(), chrono::ParseError>(())
    /// ```
    /// Times can be specified with or without seconds, and in either 24-hour or 12-hour time.
    /// Mutually exclusive with [`Job::plus()`].
    fn try_at(&mut self, time: &str) -> Result<&mut Self, chrono::ParseError> {
        self.schedule_mut().try_at(time)?;
        Ok(self)
    }

    /// Similar to [`Job::at`], but it takes a chrono::NaiveTime instead of a `&str`.
    /// Because it doesn't need to parse a string, this method will always succeed.
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # use chrono::NaiveTime;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(Weekday).at_time(NaiveTime::from_hms(23, 42, 16)).run(|| println!("Also works with NaiveTime"));
    /// ```

    fn at_time(&mut self, time: NaiveTime) -> &mut Self {
        self.schedule_mut().at_time(time);
        self
    }
    /// Specifies an offset to when a task should run, e.g.
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(1.day())
    ///     .plus(6.hours())
    ///     .plus(13.minutes())
    ///   .run(|| println!("Time to wake up!"));
    /// ```
    /// Mutually exclusive with [`Job::at()`].
    ///
    /// Note that this normally won't change the frequency with which a task runs, merely its timing.
    /// For instance, 
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # let mut scheduler = Scheduler::new();
    /// scheduler.every(1.hour())
    ///     .plus(30.minutes())
    ///   .run(|| println!("Time to wake up!"));
    /// ```
    /// will run at 00:30, 01:30, 02:30, etc., rather than at 00:00, 01:30, 03:00, etc.
    ///
    /// If that schedule is desired, then one would need to write
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # let mut scheduler = Scheduler::new();
    /// scheduler.every(90.minutes())
    ///   .run(|| println!("Time to wake up!"));
    /// ```
    /// 
    /// If the total offset exceeds the base frequency, the resulting behaviour can be unintuitive. For example,
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # let mut scheduler = Scheduler::new();
    /// scheduler.every(1.hour())
    ///   .plus(90.minutes())
    ///   .run(|| println!("Time to wake up!"));
    /// ```
    /// will run at 01:30, 02:30, 03:30, etc., while
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # let mut scheduler = Scheduler::new();
    /// scheduler.every(1.hour())
    ///   .plus(125.minutes())
    ///   .run(|| println!("Time to wake up!"));
    /// ```
    /// will run at 02:05, 04:05, 06:05, etc.
    fn plus(&mut self, ival: Interval) -> &mut Self {
        self.schedule_mut().plus(ival);
        self
    }

    /// Add an additional scheduling to the task. All schedules will be considered when determining
    /// when the task should next run.
    fn and_every(&mut self, ival: Interval) -> &mut Self {
        self.schedule_mut().and_every(ival);
        self
    }

    /// Execute the job only once. Equivalent to `_.count(1)`.
    fn once(&mut self) -> &mut Self {
        self.schedule_mut().once();
        self
    }

    /// Execute the job forever. This is the default behaviour.
    fn forever(&mut self) -> &mut Self {
        self.schedule_mut().forever();
        self
    }

    /// Execute the job only `count` times.
    fn count(&mut self, count: usize) -> &mut Self {
        self.schedule_mut().count(count);
        self
    }

    /// After running once, run again with the specified interval.
    ///
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # fn hit_snooze() {}
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(Weekday)
    ///   .at("7:40")
    ///   .repeating_every(10.minutes())
    ///   .times(5)
    ///   .run(|| hit_snooze());
    /// ```
    /// will hit snooze five times every morning, at 7:40, 7:50, 8:00, 8:10 and 8:20.
    ///
    /// Unlike [`Job::at`] and [`Job::plus`],
    /// this affects all intervals associated with the job, not just the most recent one.
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # fn hit_snooze() {}
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(Weekday)
    ///   .at("7:40")
    ///   .and_every(Saturday)
    ///   .at("9:15")
    ///   .and_every(Sunday)
    ///   .at("9:15")
    ///   .repeating_every(10.minutes())
    ///   .times(5)
    ///   .run(|| hit_snooze());
    /// ```
    /// hits snooze five times every day, not just Sundays.
    ///
    /// If a job is still repeating, it will ignore otherwise scheduled runs.
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # fn hit_snooze() {}
    /// let mut scheduler = Scheduler::new();
    /// scheduler.every(1.hour())
    ///   .repeating_every(45.minutes())
    ///   .times(3)
    ///   .run(|| println!("Hello"));
    /// ```
    /// If this is scheduled to run at 6 AM, it will print `Hello` at 6:00, 6:45, and 7:30, and then again at 8:00, 8:45, 9:30, etc.
    fn repeating_every(&mut self, interval: Interval) -> Repeating<Self, Tz, Tp> {
        Repeating::new(self, interval)
    }

    /// Test whether a job is scheduled to run again. This is usually only called by
    /// [Scheduler::run_pending()](crate::Scheduler::run_pending).
    fn is_pending(&self, now: &DateTime<Tz>) -> bool {
        self.schedule().is_pending(now)
    }
}

