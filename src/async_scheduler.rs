use std::{future::Future, marker::PhantomData, pin::Pin, task::Poll};

use crate::{async_job::JobFuture, timeprovider::{ChronoTimeProvider, TimeProvider}, Job};
use crate::Interval;
use crate::AsyncJob;

/// An asynchronous job scheduler, for use with `Future`s.
///
/// The asynchronous scheduler works almost identically to the [synchronous one](crate::Scheduler), except that
/// instead of taking functions or closures returning `()`, it takes functions or closures returning values implementing `Future<Output = ()>`.
///
/// Unlike the synchronous version, there is no [`watch_thread`](crate::Scheduler::watch_thread) method, as it would tie
/// this crate to a specific runtime, and also because it's trivial to implement by hand. For example, using tokio:
///
/// ```no_run
/// # use clokwerk::*;
/// # use std::time::Duration;
/// # let mut scheduler = AsyncScheduler::new();
/// tokio::spawn(async move {
///   loop {
///     scheduler.run_pending().await;
///     tokio::time::sleep(Duration::from_millis(100)).await;
///   }
/// });
/// ```
/// For async_std:
/// ```no_run
/// # use clokwerk::*;
/// # use std::time::Duration;
/// # let mut scheduler = AsyncScheduler::new();
/// async_std::task::spawn(async move {
///   loop {
///     scheduler.run_pending().await;
///     async_std::task::sleep(Duration::from_millis(100)).await;
///   }
/// });
/// ```
/// ### Usage examples
/// The examples below are intended to demonstrate how to work with various types of Future. 
/// See [synchronous examples](crate::Scheduler) for more examples of how to schedule tasks.
///
/// ```rust
/// // Scheduler, trait for .seconds(), .minutes(), etc., and trait with job scheduling methods
/// use clokwerk::{AsyncScheduler, TimeUnits, Job};
/// // Import week days and WeekDay
/// use clokwerk::Interval::*;
/// use std::time::Duration;
/// # use std::future::Future;
/// # use std::pin::Pin;
/// # async fn some_async_fn() {}
/// # fn returns_boxed_future() -> Box<dyn Future<Output=()> + Send> { Box::new(some_async_fn()) }
/// # fn returns_pinned_boxed_future() -> Pin<Box<dyn Future<Output=()> + Send>> { Box::pin(some_async_fn()) }
///
/// // Create a new scheduler
/// let mut scheduler = AsyncScheduler::new();
/// // Add some tasks to it
/// scheduler
///     .every(10.minutes())
///         .plus(30.seconds())
///     .run(|| async { println!("Simplest is just using an async block"); });
/// scheduler
///     .every(1.day())
///         .at("3:20 pm")
///     .run(|| some_async_fn());
/// scheduler
///     .every(Wednesday)
///         .at("14:20:17")
///     .run(some_async_fn);
/// scheduler
///     .every(Tuesday)
///         .at("14:20:17")
///     .and_every(Thursday)
///         .at("15:00")
///     .run(|| std::pin::Pin::from(returns_boxed_future()));
/// scheduler
///     .every(Weekday)
///     .run(|| returns_pinned_boxed_future());
/// scheduler
///     .every(1.day())
///         .at("3:20 pm")
///     .run(returns_pinned_boxed_future).once();
/// # tokio_test::block_on(async move {
/// // Manually run the scheduler forever
/// loop {
///     scheduler.run_pending().await;
///     tokio::time::sleep(Duration::from_millis(10)).await;
///     # break;
/// }
///
/// // Or spawn a task to run it forever
/// tokio::spawn(async move {
///   loop {
///     scheduler.run_pending().await;
///     tokio::time::sleep(Duration::from_millis(100)).await;
///   }
/// });
/// # });
/// ```
#[derive(Debug)]
pub struct AsyncScheduler<Tz = chrono::Local, Tp = ChronoTimeProvider>
where
    Tz: chrono::TimeZone,
    Tp: TimeProvider,
{
    jobs: Vec<AsyncJob<Tz, Tp>>,
    tz: Tz,
    _tp: PhantomData<Tp>,
}

impl Default for AsyncScheduler {
    fn default() -> AsyncScheduler {
        AsyncScheduler::<chrono::Local> {
            jobs: vec![],
            tz: chrono::Local,
            _tp: PhantomData,
        }
    }
}

impl AsyncScheduler {
    /// Create a new scheduler. Dates and times will be interpretted using the local timezone
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new scheduler. Dates and times will be interpretted using the specified timezone.
    pub fn with_tz<Tz: chrono::TimeZone>(tz: Tz) -> AsyncScheduler<Tz> {
        AsyncScheduler {
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
    ) -> AsyncScheduler<Tz, Tp> {
        AsyncScheduler {
            jobs: vec![],
            tz,
            _tp: PhantomData,
        }
    }
}

impl<Tz, Tp> AsyncScheduler<Tz, Tp>
where
    Tz: chrono::TimeZone + Sync + Send,
    Tp: TimeProvider,
{
    /// Add a new job to the scheduler to be run on the given interval
    /// ```rust
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # use std::future::Future;
    /// # use std::pin::Pin;
    /// # async fn some_async_fn() {}
    /// # fn returns_boxed_future() -> Box<dyn Future<Output=()> + Send> { Box::new(some_async_fn()) }
    /// # fn returns_pinned_boxed_future() -> Pin<Box<dyn Future<Output=()> + Send>> { Box::pin(some_async_fn()) }
    /// let mut scheduler = AsyncScheduler::new();
    /// scheduler.every(10.minutes()).plus(30.seconds()).run(|| async { println!("Periodic task") });
    /// scheduler.every(1.day()).at("3:20 pm").run(|| some_async_fn());
    /// scheduler.every(Wednesday).at("14:20:17").run(|| Pin::from(returns_boxed_future()));
    /// scheduler.every(Weekday).run(|| returns_pinned_boxed_future());
    /// ```
    pub fn every(&mut self, ival: Interval) -> &mut AsyncJob<Tz, Tp> {
        let job = AsyncJob::<Tz, Tp>::new(ival, self.tz.clone());
        self.jobs.push(job);
        let last_index = self.jobs.len() - 1;
        &mut self.jobs[last_index]
    }

    /// Run all jobs that should run at this time.
    ///
    /// This method returns a future that will poll each of the tasks until they are completed.
    /// ```no_run
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// use std::time::Duration;
    /// # let mut scheduler = AsyncScheduler::new();
    /// # async {
    /// loop {
    ///     scheduler.run_pending().await;
    ///     tokio::time::sleep(Duration::from_millis(100)).await;
    ///     # break
    /// }
    /// # };
    /// ```
    /// Note that while all pending jobs will run asynchronously, a long-running task can still
    /// block future executions if you `await` the future returned by this method.
    /// If you are concerned that a task might run for a long time, there are several possible approaches:
    ///
    /// 1. Pass the result of `scheduler.run_pending()` to your runtime's `spawn` function. This might
    ///    result in multiple invocations of the same task running concurrently.
    /// 2. Use `spawn` or `spawn_blocking` in your task itself. This has the same concurrent execution risk 
    ///    as approach 1, but limited to that specific task.
    /// 3. Use `tokio::time::timeout` or equivalent to prevent `scheduler.run_pending()` or the task itself
    ///    from running more than an expected amount of time. E.g.
    /// ```no_run
    /// # use clokwerk::*;
    /// # use clokwerk::Interval::*;
    /// # async fn scrape_pages() {}
    /// use std::time::Duration;
    /// let mut scheduler = AsyncScheduler::new();
    /// scheduler.every(10.minutes()).run(|| async {
    ///   if let Err(_) = tokio::time::timeout(Duration::from_secs(10 * 60), scrape_pages()).await {
    ///     eprintln!("Timed out scraping pages")
    ///   }
    /// });
    /// ```
    pub fn run_pending(&mut self) -> AsyncSchedulerFuture {
        let now = Tp::now(&self.tz);
        let mut futures = vec![];
        for job in &mut self.jobs {
            if job.is_pending(&now) {
                if let Some(future) = job.execute(&now) {
                    futures.push(Some(future.into()));
                }
            }
        }
        AsyncSchedulerFuture {
            futures
        }
    }
}

pub struct AsyncSchedulerFuture {
    futures: Vec<Option<Pin<JobFuture>>>
}

impl Future for AsyncSchedulerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let mut all_done = true;
        
        for future in &mut self.get_mut().futures {
            if let Some(this_future) = future {
                if this_future.as_mut().poll(cx) == Poll::Ready(()) {
                    future.take();
                } else {
                    all_done = false;
                }
            }
        }
        if all_done {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

