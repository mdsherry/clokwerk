/// A trait for providing custom time providers. `TimeProvider`s are used to
/// specify where the source of DateTimes used by the scheduler. For most
/// purposes, the default `ChronoTimeProvider` is sufficient; the main
/// use case for custom `TimeProvider`s is for writing tests.
pub trait TimeProvider {
    /// Returns the current time, according to the TimeProvider
    fn now<Tz>(tz: &Tz) -> chrono::DateTime<Tz>
    where
        Tz: chrono::TimeZone + Sync + Send;
}

/// The default TimeProvider. It returns the time according to the system clock.
pub struct ChronoTimeProvider {}
impl TimeProvider for ChronoTimeProvider {
    /// Returns the current time, according to `chrono`
    fn now<Tz>(tz: &Tz) -> chrono::DateTime<Tz>
    where
        Tz: chrono::TimeZone + Sync + Send,
    {
        chrono::Local::now().with_timezone(tz)
    }
}
