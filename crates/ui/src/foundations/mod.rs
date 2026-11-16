//! Foundations (doc-17): a value, or a function from values to a value. Draws nothing,
//! and depends on nothing else in this crate (R119).
//!
//! `generated.rs` (TASK-1.93, from `crates/ui/design/foundations.json`) holds the design's
//! own named colours, type, sizes, corners, terminal themes and the focus ring (R120,
//! R121) - every value that has a light and a dark form (R123) as a method on [`Theme`],
//! generated from the data file; everything else as a plain `pub const`/struct, also
//! generated. [`Theme`] and [`Appearance`] themselves, below, are hand-written: they are
//! the crate's own structural scaffolding, not a value the design names (R121 only binds
//! the *values*).

mod generated;

pub use generated::*;

use gpui::{Global, Hsla};

/// Which of the design's two forms a light/dark-paired value takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Dark,
    Light,
}

impl Appearance {
    pub fn toggled(self) -> Self {
        match self {
            Appearance::Dark => Appearance::Light,
            Appearance::Light => Appearance::Dark,
        }
    }
}

/// The design's values in force right now: which [`Appearance`] is showing, and the
/// active session's own colour - R123's `"session"`, never a fixed value in
/// `generated.rs`, always supplied here instead. Every value that has a light/dark
/// pair in `crates/ui/design/foundations.json` is a method on `Theme` (generated); there
/// is no public free function anywhere in this crate that returns a bare light-or-dark
/// form (R123) - reaching one not through `Theme` is a compile error, not a convention.
///
/// A GPUI [`Global`] (`cx.set_global`/`cx.global::<Theme>()`); [`ActiveTheme`] below is
/// the one spelling a component uses to reach it.
pub struct Theme {
    appearance: Appearance,
    session: Hsla,
}

impl Theme {
    pub fn new(appearance: Appearance, session: Hsla) -> Self {
        Self {
            appearance,
            session,
        }
    }

    pub fn appearance(&self) -> Appearance {
        self.appearance
    }

    pub fn session(&self) -> Hsla {
        self.session
    }
}

impl Global for Theme {}

/// The design's own default session colour: `ramp.1 violet`'s own form for
/// `appearance` - the data file's own `use` text names it "default session colour"
/// (crates/ui/design/foundations.json). For bootstrapping a `Theme` before any real
/// session is known (the gallery; later, the app before its first session connects) -
/// reached through a `Theme`, never a literal, since `ramp_1_violet` is a `Theme`
/// method like every other colour (R123); `gpui::black()` (a `const fn`, not a
/// literal) is this function's own one-time placeholder to get that bootstrap `Theme`
/// built at all.
pub fn default_session(appearance: Appearance) -> Hsla {
    Theme::new(appearance, gpui::black()).ramp_1_violet()
}

/// `cx.theme()` - the one spelling a component uses to reach the theme in force (R123),
/// never `cx.global::<Theme>()` directly (both work; this is the crate's own
/// convention, documented once here rather than repeated at every call site).
pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for gpui::App {
    fn theme(&self) -> &Theme {
        self.global::<Theme>()
    }
}

impl<T> ActiveTheme for gpui::Context<'_, T> {
    fn theme(&self) -> &Theme {
        self.global::<Theme>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_stand_in() -> Hsla {
        gpui::rgb(0xa98cff).into()
    }

    #[test]
    fn dark_and_light_pick_the_right_form_of_a_colour() {
        let dark = Theme::new(Appearance::Dark, session_stand_in());
        let light = Theme::new(Appearance::Light, session_stand_in());
        // surface.ground: dark #0e0d10, light #f1efec - the data file's own values.
        assert_eq!(dark.surface_ground(), gpui::rgb(0x0e0d10).into());
        assert_eq!(light.surface_ground(), gpui::rgb(0xf1efec).into());
    }

    #[test]
    fn a_colour_alias_matches_the_colour_it_names() {
        let theme = Theme::new(Appearance::Dark, session_stand_in());
        // blocked -> ramp.5 amber, dark #ffc25e.
        assert_eq!(theme.blocked(), theme.ramp_5_amber());
    }

    #[test]
    fn the_session_colour_is_whatever_new_was_given() {
        let session = session_stand_in();
        let theme = Theme::new(Appearance::Dark, session);
        assert_eq!(theme.session(), session);
        assert_eq!(theme.focus_ring_colour(), session);
    }

    #[test]
    fn appearance_toggles_to_its_own_inverse() {
        assert_eq!(Appearance::Dark.toggled(), Appearance::Light);
        assert_eq!(Appearance::Light.toggled(), Appearance::Dark);
    }

    #[test]
    fn a_plain_value_with_no_light_dark_pair_needs_no_theme() {
        assert_eq!(RADIUS_ROUND, gpui::px(999.0));
        assert_eq!(NAV_SIDE.width, gpui::px(232.0));
    }

    #[test]
    fn default_session_matches_ramp_1_violets_own_form() {
        let dark = Theme::new(Appearance::Dark, default_session(Appearance::Dark));
        let light = Theme::new(Appearance::Light, default_session(Appearance::Light));
        assert_eq!(default_session(Appearance::Dark), dark.ramp_1_violet());
        assert_eq!(default_session(Appearance::Light), light.ramp_1_violet());
    }
}
