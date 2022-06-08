# Clokwerk, a simple scheduler

[![Crate](https://img.shields.io/crates/v/clokwerk)](https://crates.io/crates/clokwerk)
[![API](https://docs.rs/clokwerk/badge.svg)](https://docs.rs/clokwerk)

Clokwerk is a simple scheduler, inspired by Python's [Schedule](https://schedule.readthedocs.io/en/stable/)
and Ruby's [clockwork](https://github.com/Rykian/clockwork). It uses a similar DSL for scheduling, rather than
parsing cron strings.

By default, times and dates are relative to the local timezone, but the scheduler can be made to use a 
different timezone using the `Scheduler::with_tz` constructor.

Since version 0.4, Clokwerk has also supported a separate `AsyncScheduler`, which can easily run asynchronous tasks concurrently.

## Usage
```rust
// Scheduler, and trait for .seconds(), .minutes(), etc.
use clokwerk::{Scheduler, TimeUnits};
// Import week days and WeekDay
use clokwerk::Interval::*;
use std::thread;
use std::time::Duration;

// Create a new scheduler
let mut scheduler = Scheduler::new();
// or a scheduler with a given timezone
let mut scheduler = Scheduler::with_tz(chrono::Utc);
// Add some tasks to it
scheduler.every(10.minutes()).plus(30.seconds()).run(|| println!("Periodic task"));
scheduler.every(1.day()).at("3:20 pm").run(|| println!("Daily task"));
scheduler.every(Tuesday).at("14:20:17").and_every(Thursday).at("15:00").run(|| println!("Biweekly task"));

// Manually run the scheduler in an event loop
for _ in 1..10 {
    scheduler.run_pending();
    thread::sleep(Duration::from_millis(10));
}
// Or run it in a background thread
let thread_handle = scheduler.watch_thread(Duration::from_millis(100));
// The scheduler stops when `thread_handle` is dropped, or `stop` is called
thread_handle.stop();
```

See [documentation](https://docs.rs/clokwerk) for additional examples of usage.

## Features

Default feature:

* `async`: Exposes `AsyncSchedular` and `AsyncJob`.

Optional feature:

* `serde-1`: Provides serialization and deserialzation for `Interval`, `RunConfig` and `Adjustment`.

## Similar libraries
* [schedule-rs](https://github.com/mehcode/schedule-rs) and [job_scheduler](https://github.com/lholden/job_scheduler) are two other Rust scheduler libraries. Both use `cron` syntax for scheduling.
