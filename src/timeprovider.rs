pub trait TimeProvider {
    fn now<Tz>(tz: &Tz) -> chrono::DateTime<Tz>
    where
        Tz: chrono::TimeZone + Sync + Send;
}

pub struct ChronoTimeProvider {}
impl TimeProvider for ChronoTimeProvider {
    fn now<Tz>(tz: &Tz) -> chrono::DateTime<Tz>
    where
        Tz: chrono::TimeZone + Sync + Send,
    {
        chrono::Local::now().with_timezone(tz)
    }
}
