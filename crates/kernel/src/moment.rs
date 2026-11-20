//! A point in time, as plain data (doc-16 R110: the kernel never reads a
//! clock, so this crate never produces one of these by reading one).

use std::time::Duration;

/// A point in time the caller — the server, which owns a real clock —
/// chose to represent as a duration since whatever origin it picked once
/// and keeps using (its own process start, the Unix epoch, anything).
/// `Moment`s are only ever compared or offset against each other inside
/// this crate, never read as a wall-clock value, so the choice of origin
/// never matters here: the kernel only asks "is this one later than that
/// one" and "this one, ten minutes on" (R83), never "what time is it".
///
/// Deliberately a newtype over [`Duration`], not `std::time::Instant`:
/// `Instant` has no public constructor besides `Instant::now()`, which
/// `crates/kernel/clippy.toml` bans everywhere in this crate, tests
/// included (`cargo clippy -D warnings`, `justfile`'s `check` recipe, runs
/// over `--all-targets`) — so a test exercising anything timed would need
/// a scoped `#[expect(clippy::disallowed_methods)]` workaround just to
/// build one. `Duration` has ordinary `const fn` constructors
/// (`Duration::from_secs`, ...), so a `Moment` is constructible in a test
/// with a plain literal, with no such exception anywhere.
///
/// Never crosses the API (R105's "defined once, in the sdk" is about
/// shapes that do): nothing on the wire carries a `Moment`, so it has no
/// business in the sdk, whose own dependency table (`crates/sdk/CLAUDE.md`)
/// gives it nothing to depend on anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Moment(Duration);

impl Moment {
    /// Builds a `Moment` this far after the caller's own origin. Pure
    /// arithmetic on a plain `Duration` - no clock involved, so this is as
    /// constructible in a test as in the server that has a real one to
    /// convert.
    #[must_use]
    pub const fn new(since_the_callers_origin: Duration) -> Self {
        Self(since_the_callers_origin)
    }

    /// This moment, offset later by `duration` (R83's ten-minute
    /// deadline). Saturates at `Duration::MAX` rather than panicking
    /// (`Duration::saturating_add`), though nothing realistic gets
    /// anywhere near it.
    #[must_use]
    pub const fn plus(self, duration: Duration) -> Self {
        Self(self.0.saturating_add(duration))
    }

    /// This moment's own duration since whatever origin its caller picked
    /// (TASK-1.88, `server.show`'s own uptime field) - the one legitimate
    /// exception to this type's own doc above ("never read as a wall-clock
    /// value... the kernel only asks is this one later than that one, this
    /// one ten minutes on, never what time is it"): the caller already
    /// knows what the origin means (`Runtime::new`'s own construction
    /// time), and uptime literally *is* that duration. The one honesty gap
    /// this carries: `clock_origin` is `Runtime::new`'s own construction
    /// time, a few milliseconds after the arbiter actually bound the
    /// socket - not the process's own start.
    #[must_use]
    pub const fn since_origin(self) -> Duration {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::Moment;
    use std::time::Duration;

    #[test]
    fn plus_adds_the_duration() {
        let moment = Moment::new(Duration::from_secs(10));

        assert_eq!(
            moment.plus(Duration::from_secs(5)),
            Moment::new(Duration::from_secs(15))
        );
    }

    #[test]
    fn a_later_moment_compares_greater() {
        let earlier = Moment::new(Duration::from_secs(1));
        let later = Moment::new(Duration::from_secs(2));

        assert!(later > earlier);
        assert!(earlier < later);
    }

    #[test]
    fn equal_moments_compare_equal() {
        assert_eq!(
            Moment::new(Duration::from_secs(1)),
            Moment::new(Duration::from_secs(1))
        );
    }

    #[test]
    fn since_origin_returns_exactly_what_new_was_given() {
        let moment = Moment::new(Duration::from_secs(42));

        assert_eq!(moment.since_origin(), Duration::from_secs(42));
    }

    #[test]
    fn plus_saturates_instead_of_overflowing() {
        let moment = Moment::new(Duration::MAX);

        assert_eq!(
            moment.plus(Duration::from_secs(1)),
            Moment::new(Duration::MAX)
        );
    }
}
