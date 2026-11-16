//! The gallery (doc-17, R124): a program in this crate that shows every component this
//! package holds, in every state the design draws, light and dark. TASK-1.91's
//! placeholder proved the window opens, the light/dark switch works, and
//! `just capture-gallery` can get a picture of it with no person at the screen - all
//! three still hold. TASK-1.93 replaces the placeholder's own literal colours with
//! `steg_ui::foundations`: every colour, size, radius, duration and font this file
//! draws comes from a named foundations value, same as any real component would
//! (TASK-1.94's colour-literal check will refuse anything else here once it lands).
//!
//! What it shows, each from `foundations` by its own design name: every colour as a
//! swatch, light and dark; the two ramps; the type sizes; the spacing steps; the four
//! corners; and both terminal themes' 16 ANSI colours.
//!
//! The only thing in `steg-ui` that calls `gpui::Application::run`; `src/` never opens a
//! window, and nothing in `cargo build`/`cargo test` executes this file's `main`.

use gpui::{
    App, Application, Context, FocusHandle, Focusable, Hsla, KeyDownEvent, Render, TitlebarOptions,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use steg_ui::foundations::{
    self, Appearance, FOCUS_RING, FONT_TERMINAL, NAV_SIDE, PALETTE_INPUT, PANE_CAPTION,
    RADIUS_INNER, RADIUS_ROUND, RADIUS_SURFACE, RADIUS_TRACK, SPACE, Theme, UI_BODY, UI_LABEL,
    UI_SMALL,
};

/// Read the gallery's initial appearance from `STEG_GALLERY_THEME` ("light" or "dark",
/// any case). Anything else, including unset, is light - `just capture-gallery` sets
/// this explicitly for both of its runs, so it never relies on this default; a person
/// running `just run-gallery` by hand gets a predictable start instead of an error over
/// a typo.
fn appearance_from_env(value: Option<&str>) -> Appearance {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("dark") => Appearance::Dark,
        Some(_) | None => Appearance::Light,
    }
}

/// Build the theme for `appearance`, with the default session colour `ramp.1 violet`'s
/// own form - the data file's own `use` text names it "default session colour"
/// (crates/ui/design/foundations.json). `Theme::new` needs a session colour before any
/// of its other methods can be called, so this reaches `ramp_1_violet` through one
/// short-lived bootstrap `Theme` rather than a literal.
fn gallery_theme(appearance: Appearance) -> Theme {
    let bootstrap = Theme::new(appearance, gpui::rgb(0x000000).into());
    let session = bootstrap.ramp_1_violet();
    Theme::new(appearance, session)
}

/// Every named colour this gallery shows, paired with its own `Theme` method
/// (crates/ui/design/foundations.json's `colours` and `colour_aliases`).
fn colour_entries(theme: &Theme) -> Vec<(&'static str, Hsla)> {
    vec![
        ("surface.ground", theme.surface_ground()),
        ("surface.pane", theme.surface_pane()),
        ("surface.raised", theme.surface_raised()),
        ("surface.overlay", theme.surface_overlay()),
        ("fill.hover", theme.fill_hover()),
        ("fill.quiet", theme.fill_quiet()),
        ("outline", theme.outline()),
        ("text", theme.text()),
        ("text.secondary", theme.text_secondary()),
        ("text.muted", theme.text_muted()),
        ("ramp.1 violet", theme.ramp_1_violet()),
        ("ramp.2 pink", theme.ramp_2_pink()),
        ("ramp.3 red", theme.ramp_3_red()),
        ("ramp.4 orange", theme.ramp_4_orange()),
        ("ramp.5 amber", theme.ramp_5_amber()),
        ("ramp.6 green", theme.ramp_6_green()),
        ("session.blue", theme.session_blue()),
        ("track", theme.track()),
        ("blocked", theme.blocked()),
        ("done", theme.done()),
        ("error", theme.error()),
    ]
}

/// The 16 ANSI colours of one terminal theme, named the way `AnsiColours` lists them.
fn ansi_entries(ansi: &foundations::AnsiColours) -> Vec<(&'static str, Hsla)> {
    vec![
        ("black", ansi.black),
        ("red", ansi.red),
        ("green", ansi.green),
        ("yellow", ansi.yellow),
        ("blue", ansi.blue),
        ("magenta", ansi.magenta),
        ("cyan", ansi.cyan),
        ("white", ansi.white),
        ("bright_black", ansi.bright_black),
        ("bright_red", ansi.bright_red),
        ("bright_green", ansi.bright_green),
        ("bright_yellow", ansi.bright_yellow),
        ("bright_blue", ansi.bright_blue),
        ("bright_magenta", ansi.bright_magenta),
        ("bright_cyan", ansi.bright_cyan),
        ("bright_white", ansi.bright_white),
    ]
}

/// One colour, its design name beneath it, and a border so a colour close to the
/// window's own background still reads as its own swatch.
fn colour_swatch(
    name: &'static str,
    colour: Hsla,
    label_colour: Hsla,
    border: Hsla,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(2.0))
        .child(
            div()
                .w(px(56.0))
                .h(px(36.0))
                .rounded(RADIUS_INNER)
                .border_1()
                .border_color(border)
                .bg(colour),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(label_colour)
                .child(name),
        )
}

/// A titled section: a heading in the design's label style, then its own content.
fn section(title: &'static str, label_colour: Hsla, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_size(UI_LABEL.size)
                .text_color(label_colour)
                .child(title.to_uppercase()),
        )
        .child(content)
}

struct Gallery {
    theme: Theme,
    focus_handle: FocusHandle,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>, appearance: Appearance) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);
        let theme = gallery_theme(appearance);
        cx.set_global(gallery_theme(theme.appearance()));
        Self {
            theme,
            focus_handle,
        }
    }

    /// `t` toggles the gallery's own appearance. Only a person looking at
    /// `just run-gallery` can press it; `just capture-gallery` instead launches the
    /// binary twice, once per `STEG_GALLERY_THEME` value, so the capture needs no
    /// synthetic keystroke (which would need its own, separate Accessibility
    /// permission grant this recipe does not take on).
    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "t" {
            self.theme = gallery_theme(self.theme.appearance().toggled());
            cx.set_global(gallery_theme(self.theme.appearance()));
            cx.notify();
        }
    }
}

impl Focusable for Gallery {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let border = theme.outline();
        let text = theme.text();
        let muted = theme.text_muted();

        let colours = div().flex().flex_wrap().gap(px(10.0)).children(
            colour_entries(theme)
                .into_iter()
                .map(|(name, colour)| colour_swatch(name, colour, text, border)),
        );

        let ramp = theme.ramp();
        let ramp_bar = div()
            .flex()
            .h(px(20.0))
            .w(px(320.0))
            .rounded(RADIUS_INNER)
            .children(
                ramp.stops
                    .iter()
                    .map(|(_, colour)| div().flex_1().h_full().bg(*colour)),
            );
        let ramp_extended_bar = div()
            .flex()
            .h(px(20.0))
            .w(px(320.0))
            .rounded(RADIUS_INNER)
            .children(
                theme
                    .ramp_extended()
                    .iter()
                    .map(|(_, colour)| div().flex_1().h_full().bg(*colour)),
            );
        let ramps = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(ramp_bar)
            .child(ramp_extended_bar);

        let spacing = div().flex().items_end().gap(px(6.0)).children(
            SPACE
                .iter()
                .map(|step| div().w(px(16.0)).h(*step).bg(theme.ramp_1_violet())),
        );

        let corners = div().flex().gap(px(10.0)).children(
            [
                ("radius.surface", RADIUS_SURFACE),
                ("radius.track", RADIUS_TRACK),
                ("radius.inner", RADIUS_INNER),
                ("radius.round", RADIUS_ROUND),
            ]
            .into_iter()
            .map(|(name, radius)| {
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(2.0))
                    .child(
                        div()
                            .w(px(48.0))
                            .h(px(48.0))
                            .rounded(radius)
                            .bg(theme.ramp_2_pink()),
                    )
                    .child(div().text_size(px(10.0)).text_color(muted).child(name))
            }),
        );

        let type_sizes = div().flex().flex_col().gap(px(4.0)).children(
            [
                ("ui.small", UI_SMALL),
                ("palette.input", PALETTE_INPUT),
                ("ui.body", UI_BODY.size),
                ("ui.label", UI_LABEL.size),
            ]
            .into_iter()
            .map(|(name, size)| {
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(90.0))
                            .text_size(px(10.0))
                            .text_color(muted)
                            .child(name),
                    )
                    .child(div().text_size(size).text_color(text).child("Aa steg"))
            }),
        );

        let dark_ansi = foundations::steg_dark(theme.ramp_1_violet());
        let light_ansi = foundations::steg_light(theme.ramp_1_violet());
        let terminal_themes = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div().flex().flex_wrap().gap(px(4.0)).children(
                    ansi_entries(&dark_ansi.ansi)
                        .into_iter()
                        .map(|(name, colour)| colour_swatch(name, colour, muted, border)),
                ),
            )
            .child(
                div().flex().flex_wrap().gap(px(4.0)).children(
                    ansi_entries(&light_ansi.ansi)
                        .into_iter()
                        .map(|(name, colour)| colour_swatch(name, colour, muted, border)),
                ),
            );

        div()
            .id("gallery-root")
            .track_focus(&self.focus_handle)
            .on_key_down(_cx.listener(Self::on_key_down))
            .size_full()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .p(px(24.0))
            .bg(theme.surface_ground())
            .text_color(text)
            .child(format!(
                "steg-ui gallery ({}; press t to switch) - NAV_SIDE.width {:?}, FONT_TERMINAL {:?}, FOCUS_RING.width {:?}, PANE_CAPTION {:?}",
                if matches!(theme.appearance(), Appearance::Dark) { "dark" } else { "light" },
                NAV_SIDE.width,
                FONT_TERMINAL,
                FOCUS_RING.width,
                PANE_CAPTION,
            ))
            .child(section("Colours", muted, colours))
            .child(section("Ramps", muted, ramps))
            .child(section("Spacing", muted, spacing))
            .child(section("Corners", muted, corners))
            .child(section("Type", muted, type_sizes))
            .child(section("Terminal themes (dark, light)", muted, terminal_themes))
    }
}

fn main() {
    let appearance = appearance_from_env(std::env::var("STEG_GALLERY_THEME").ok().as_deref());
    Application::new().run(move |cx: &mut App| {
        let bounds = WindowBounds::centered(size(px(1040.0), px(1020.0)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                titlebar: Some(TitlebarOptions {
                    title: Some("steg-ui gallery".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| Gallery::new(window, cx, appearance)),
        );
        if let Err(error) = result {
            eprintln!("gallery: failed to open window: {error}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_is_light() {
        assert_eq!(appearance_from_env(None), Appearance::Light);
    }

    #[test]
    fn dark_is_recognised_in_any_case() {
        assert_eq!(appearance_from_env(Some("dark")), Appearance::Dark);
        assert_eq!(appearance_from_env(Some("DARK")), Appearance::Dark);
        assert_eq!(appearance_from_env(Some("Dark")), Appearance::Dark);
    }

    #[test]
    fn light_is_explicit_too() {
        assert_eq!(appearance_from_env(Some("light")), Appearance::Light);
    }

    #[test]
    fn an_unrecognised_value_falls_back_to_light_rather_than_failing() {
        assert_eq!(appearance_from_env(Some("sepia")), Appearance::Light);
    }

    #[test]
    fn the_gallery_theme_carries_the_appearance_asked_for() {
        assert_eq!(
            gallery_theme(Appearance::Dark).appearance(),
            Appearance::Dark
        );
        assert_eq!(
            gallery_theme(Appearance::Light).appearance(),
            Appearance::Light
        );
    }
}
