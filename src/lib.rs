//! # Clokwerk, a simple scheduler
//!
//! Clokwerk is a simple scheduler, inspired by Python's [Schedule](https://schedule.readthedocs.io/en/stable/)
//! and Ruby's [clockwork](https://github.com/Rykian/clockwork). It uses a similar DSL for scheduling, rather than
//! parsing cron strings.
//!
//! ## Usage
//! ### Synchronous
//! See [`Scheduler`].
//! ### Asynchronous
//! See [`AsyncScheduler`].
//! ## Caveats
//! Some combinations of times or intervals are permissible, but make little sense, e.g. `every(10.seconds()).at("16:00")`, which would next run at the next 4 PM after the next multiple of 10 seconds.
//!
//! ## Similar libraries
//! * [schedule-rs](https://github.com/mehcode/schedule-rs) and [job_scheduler](https://github.com/lholden/job_scheduler) are two other Rust scheduler libraries. Both use `cron` syntax for scheduling.
#[cfg(feature = "async")]
mod async_job;
#[cfg(feature = "async")]
mod async_scheduler;
mod intervals;
mod job;
mod job_schedule;
mod scheduler;
mod sync_job;
pub mod timeprovider;

pub use crate::intervals::{Interval, NextTime, TimeUnits};
pub use crate::job::Job;
pub use crate::scheduler::{ScheduleHandle, Scheduler};
pub use crate::sync_job::SyncJob;

#[cfg(feature = "async")]
pub use crate::async_job::AsyncJob;
#[cfg(feature = "async")]
pub use crate::async_scheduler::AsyncScheduler;
