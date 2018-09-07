# Clokwerk, a simple scheduler

Clokwerk is a simple scheduler, inspired by Python's [Schedule](https://schedule.readthedocs.io/en/stable/)
and Ruby's [clockwork](https://github.com/Rykian/clockwork). It uses a similar DSL for scheduling, rather than
parsing cron strings.

## Usage
```rust
// Scheduler, and trait for .seconds(), .minutes(), etc.
use clokwerk::{Scheduler, TimeUnit};
// Import week days and WeekDay
use clokwerk::Interval::*;
use std::thread;
use std::time::Duration;

// Create a new scheduler
let mut scheduler = Scheduler::new();
// Add some tasks to it
scheduler.every(10.minutes()).plus(30.seconds()).run(|| println!("Periodic task"));
scheduler.every(1.day()).at("3:20 pm").run(|| println!("Daily task"));
scheduler.every(Wednesday).at("14:20:17").run(|| println!("Weekly task"));
scheduler.every(Weekday).run(|| println!("Every weekday at midnight"));

// Manually run the scheduler in an event loop
for _ in 1..10 {
    scheduler.run_pending();
    thread::sleep(Duration::from_secs(10));
}

// Or run it in a background thread
let thread_handle = scheduler.watch_thread(Duration::from_millis(100));
// The scheduler stops when `thread_handle` is dropped, or `stop` is called
thread_handle.stop();
```

## Caveats
Some combinations of times or intervals are permissible, but make little sense, e.g. `every(10.seconds()).at("16:00")`, which would next run at the next 4 PM after the next multiple of 10 seconds.

## Similar libraries
* [schedule-rs](https://github.com/mehcode/schedule-rs) and [job_scheduler](https://github.com/lholden/job_scheduler) are two other Rust scheduler libraries. Both use `cron` syntax for scheduling.