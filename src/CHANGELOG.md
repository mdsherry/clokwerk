## 0.3.4
* Times returned by `Interval::next` and `Interval::prev` now have nanoseconds set to 0; previously, the nanoseconds of the current time would be used.
* Improve documentation on `Scheduler::watch_thread`.

## 0.3.3
* Adding the trait in 0.3.1 was accidentally a breaking change, and hurt usability, so that change was reverted.

## 0.3.2
* Fix documentation link

## 0.3.1
* Support repeating jobs in quicker succession than normally scheduled
* New trait to allow more flexibility in specifying times using Job::at

## 0.3.0
* Remove `Sync` requirement for jobs
* Add `TimeProvider` type parameter to allow custom times when testing
* Add license file
* Make `now` a parameter of `Job::is_pending` and `Job::execute`
* Let a job run only a finite number of times with new methods `Job::once` and `Job::count`. (`Job::forever` also exists, in case you change your mind.)
* Expose the `NextTime` trait to let others compute next and previous intervals

## 0.2.2
* Fix divide-by-zero in interval calculation

## 0.2.1
* Remove debug println

## 0.2.0
* Custom timezone support

## 0.1.1
* Minor fixes and documentation

## 0.1.0
* Original release