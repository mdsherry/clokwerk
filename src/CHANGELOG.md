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