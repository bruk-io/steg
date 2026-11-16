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
/// form (R123) - reaching one not through `Theme` is a compile error, not a convention,
/// checked by `tools/src/steg_tools/foundations_public_surface.py` (review #5 of
/// TASK-1.93 found this claim false once, for exactly the case a hand-reader expects
/// to be the hardest to get right: `Theme`'s own bootstrap, below).
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

    /// A `Theme` with the design's own default session colour: `ramp.1 violet`'s own
    /// form for `appearance` (the data file's own `use` text names it "default
    /// session colour"). For bootstrapping before any real session is known (the
    /// gallery; later, the app before its first session connects) - this hands out a
    /// `Theme`, never a bare colour: there is still no public function in this crate
    /// that returns an `Hsla` outside a `Theme` method (R123), this one included.
    /// `gpui::black()` (a `const fn`, not a literal) is only this function's own
    /// one-time placeholder, to get a `Theme` built at all before `ramp_1_violet` can
    /// be called on it.
    pub fn with_default_session(appearance: Appearance) -> Self {
        let session = Theme::new(appearance, gpui::black()).ramp_1_violet();
        Theme::new(appearance, session)
    }

    pub fn appearance(&self) -> Appearance {
        self.appearance
    }

    pub fn session(&self) -> Hsla {
        self.session
    }
}

impl Global for Theme {}

/// `Hsla` -> `"#rrggbb"`, for display only (the gallery's own swatches) - the
/// standard HSL-to-RGB conversion, run backwards on a colour a `Theme` already
/// resolved. Takes a colour, returns text about it; doesn't hand out a colour, so
/// R123's rule (no public function returns a bare `Hsla` outside a `Theme` method)
/// doesn't apply to it - and it lives here, in `foundations`, rather than in the
/// gallery, because formatting one of this crate's own colours belongs with them
/// (review #5 of TASK-1.93), and because `crates/ui/src/foundations/**` is the one
/// path TASK-1.94's colour-literal check always exempts - this function's own tests
/// need a literal hex to assert against, by the nature of what they prove.
pub fn hex_of(colour: Hsla) -> String {
    let h = colour.h * 6.0;
    let c = (1.0 - (2.0 * colour.l - 1.0).abs()) * colour.s;
    let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let m = colour.l - c / 2.0;
    let (r1, g1, b1) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to_byte = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", to_byte(r1), to_byte(g1), to_byte(b1))
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
    fn with_default_session_matches_ramp_1_violets_own_form() {
        let dark = Theme::with_default_session(Appearance::Dark);
        let light = Theme::with_default_session(Appearance::Light);
        assert_eq!(dark.session(), dark.ramp_1_violet());
        assert_eq!(light.session(), light.ramp_1_violet());
        assert_eq!(dark.appearance(), Appearance::Dark);
        assert_eq!(light.appearance(), Appearance::Light);
    }

    #[test]
    fn hex_of_round_trips_a_named_colours_own_hex() {
        // ramp.1 violet, dark: #a98cff (crates/ui/design/foundations.json).
        assert_eq!(hex_of(gpui::rgb(0xa98cff).into()), "#a98cff");
    }

    #[test]
    fn hex_of_round_trips_a_near_neutral_colour() {
        // surface.ground, light: #f1efec - near-grey (low saturation), the case the
        // plan flagged as a real risk for this conversion (hue is ill-conditioned at
        // low saturation) - checked directly rather than assumed fine.
        assert_eq!(hex_of(gpui::rgb(0xf1efec).into()), "#f1efec");
    }
}
