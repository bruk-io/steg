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
//! corners; both terminal themes' 16 ANSI colours; and - as plain `name: value` rows,
//! "Other named values" - every one of the data file's 54 names (AC#4) that isn't
//! already a swatch, bar or sample above (`faint`, `tint`, the rest of `type` and
//! `sizes`, `motion.status-fade`, `focus_ring`), so a reviewer can find *every* design
//! name somewhere in this image, not only the ones that draw naturally as a picture.
//!
//! The only thing in `steg-ui` that calls `gpui::Application::run`; `src/` never opens a
//! window, and nothing in `cargo build`/`cargo test` executes this file's `main`.

use gpui::{
    App, Application, Context, FocusHandle, Focusable, Hsla, KeyDownEvent, Pixels, Render,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, linear_color_stop, linear_gradient,
    prelude::*, px, size,
};
use steg_ui::foundations::{
    self, Appearance, FOCUS_RING_DESCRIPTION, FONT_TERMINAL_DESCRIPTION, FONT_UI_DESCRIPTION,
    MOTION_STATUS_FADE_DESCRIPTION, NAV_SIDE_DESCRIPTION, NAV_TOP_DESCRIPTION,
    OVERLAY_LAYOUT_DESCRIPTION, PALETTE_DESCRIPTION, PALETTE_INPUT, PANE_CAPTION_DESCRIPTION,
    PANE_GAP_DESCRIPTION, RADIUS_INNER, RADIUS_ROUND, RADIUS_SURFACE, RADIUS_TRACK,
    ROW_GAP_DESCRIPTION, SPACE, SPLIT_LAYOUT_DESCRIPTION, STACK_GAP_DESCRIPTION,
    STATUS_RING_DESCRIPTION, TAB_WIDTH_DESCRIPTION, TERMINAL_DESCRIPTION, TRACK_LAYOUT_DESCRIPTION,
    Theme, UI_BODY, UI_LABEL, UI_SMALL,
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

/// Every named colour this gallery shows, paired with its own `Theme` method
/// (crates/ui/design/foundations.json's `colours` and `colour_aliases`) - plus `tint`
/// and `scrim`'s own colour component, which belong here rather than in "Other named
/// values" (review #4: a colour wants a swatch and its hex, not debug text). `tint`
/// and `scrim` carry their own opacity in the design (review #5) - stated in the name
/// itself, since that's the one property `colour_swatch` doesn't otherwise show.
fn colour_entries(theme: &Theme) -> Vec<(String, Hsla)> {
    let (tint_colour, tint_opacity) = theme.tint();
    let scrim = theme.scrim();
    vec![
        ("surface.ground".to_string(), theme.surface_ground()),
        ("surface.pane".to_string(), theme.surface_pane()),
        ("surface.raised".to_string(), theme.surface_raised()),
        ("surface.overlay".to_string(), theme.surface_overlay()),
        ("fill.hover".to_string(), theme.fill_hover()),
        ("fill.quiet".to_string(), theme.fill_quiet()),
        ("outline".to_string(), theme.outline()),
        ("text".to_string(), theme.text()),
        ("text.secondary".to_string(), theme.text_secondary()),
        ("text.muted".to_string(), theme.text_muted()),
        ("ramp.1 violet".to_string(), theme.ramp_1_violet()),
        ("ramp.2 pink".to_string(), theme.ramp_2_pink()),
        ("ramp.3 red".to_string(), theme.ramp_3_red()),
        ("ramp.4 orange".to_string(), theme.ramp_4_orange()),
        ("ramp.5 amber".to_string(), theme.ramp_5_amber()),
        ("ramp.6 green".to_string(), theme.ramp_6_green()),
        ("session.blue".to_string(), theme.session_blue()),
        ("track".to_string(), theme.track()),
        ("blocked".to_string(), theme.blocked()),
        ("done".to_string(), theme.done()),
        ("error".to_string(), theme.error()),
        (format!("tint @ {:.0}%", tint_opacity * 100.0), tint_colour),
        (
            format!("scrim @ {:.0}%", scrim.opacity * 100.0),
            scrim.colour,
        ),
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

/// Every design name not already shown as a swatch, bar or sample elsewhere in this
/// gallery (AC#4: every one of the 54 names findable somewhere in the image) - `faint`,
/// the rest of `type` and `sizes`, `motion.status-fade` and `focus_ring` (`tint` and
/// `scrim` moved to `colour_entries` - a colour gets a swatch, not text). Each value
/// reads its generated `*_DESCRIPTION` - the design's own words ("width 232px, ..."),
/// never a Rust debug dump of struct/field syntax (team lead's review of the first
/// capture). The three groups whose one colour field is excluded from their own
/// `_DESCRIPTION` (`status.ring`, `track.layout`, `focus_ring`) name the already-shown
/// colour their field equals, rather than drawing it a second time.
fn other_named_values(theme: &Theme) -> Vec<(&'static str, String)> {
    vec![
        ("faint", format!("{}", theme.faint())),
        ("font.ui", FONT_UI_DESCRIPTION.to_string()),
        ("font.terminal", FONT_TERMINAL_DESCRIPTION.to_string()),
        ("terminal", TERMINAL_DESCRIPTION.to_string()),
        ("nav.side", NAV_SIDE_DESCRIPTION.to_string()),
        ("nav.top", NAV_TOP_DESCRIPTION.to_string()),
        ("tab.width", TAB_WIDTH_DESCRIPTION.to_string()),
        ("pane.gap", PANE_GAP_DESCRIPTION.to_string()),
        ("pane.caption", PANE_CAPTION_DESCRIPTION.to_string()),
        ("palette", PALETTE_DESCRIPTION.to_string()),
        (
            "status.ring",
            format!("{STATUS_RING_DESCRIPTION}, track colour = track"),
        ),
        ("row.gap", ROW_GAP_DESCRIPTION.to_string()),
        ("stack.gap", STACK_GAP_DESCRIPTION.to_string()),
        (
            "track.layout",
            format!("{TRACK_LAYOUT_DESCRIPTION}, fill colour = tint"),
        ),
        ("split.layout", SPLIT_LAYOUT_DESCRIPTION.to_string()),
        ("overlay.layout", OVERLAY_LAYOUT_DESCRIPTION.to_string()),
        (
            "motion.status-fade",
            MOTION_STATUS_FADE_DESCRIPTION.to_string(),
        ),
        (
            "focus_ring",
            format!("{FOCUS_RING_DESCRIPTION}, colour = session"),
        ),
    ]
}

/// One colour: a border so a colour close to the window's own background still reads
/// as its own swatch, and its design name beneath it. `with_hex` also shows the
/// colour's own hex, via `foundations::hex_of` (review #4: a colour wants a swatch
/// *and* its hex, not just a name) - skipped for the 21-wide terminal theme rows,
/// where this screen's own height is the binding constraint (every row already
/// named, every hex still one `hex_of` call away from any of them).
fn colour_swatch(
    name: impl Into<String>,
    colour: Hsla,
    label_colour: Hsla,
    border: Hsla,
    with_hex: bool,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(2.0))
        .child(
            div()
                .w(px(44.0))
                .h(px(28.0))
                .rounded(RADIUS_INNER)
                .border_1()
                .border_color(border)
                .bg(colour),
        )
        .child(
            div()
                .text_size(px(9.0))
                .text_color(label_colour)
                .child(name.into()),
        )
        .when(with_hex, |swatch| {
            swatch.child(
                div()
                    .text_size(px(9.0))
                    .text_color(label_colour)
                    .child(foundations::hex_of(colour)),
            )
        })
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

/// A ramp's stops, drawn as gradients and marked by percentage - not hard bands
/// (team lead's review: "the design's ramps are gradients between named stops").
/// gpui 0.2.2's own `linear_gradient` takes exactly two stops at a time
/// (`color.rs`'s `LinearColorStop`, a fixed `[_; 2]`), so an N-stop ramp is N-1
/// adjacent two-stop gradients, each sized to its own share of the bar's width.
fn ramp_gradient(
    stops: &[(f32, Hsla)],
    width: Pixels,
    height: Pixels,
    muted: Hsla,
) -> impl IntoElement {
    let bar = div()
        .flex()
        .h(height)
        .w(width)
        .rounded(RADIUS_INNER)
        .children(stops.windows(2).map(|pair| {
            let (from_pct, from_colour) = pair[0];
            let (to_pct, to_colour) = pair[1];
            div()
                .w(width * ((to_pct - from_pct) / 100.0))
                .h_full()
                .bg(linear_gradient(
                    90.0,
                    linear_color_stop(from_colour, 0.0),
                    linear_color_stop(to_colour, 1.0),
                ))
        }));
    let legend = div()
        .flex()
        .justify_between()
        .w(width)
        .children(stops.iter().map(|(pct, _)| {
            div()
                .text_size(px(9.0))
                .text_color(muted)
                .child(format!("{pct:.0}%"))
        }));
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(bar)
        .child(legend)
}

/// One terminal theme, labelled by its own design name, its base colours (background,
/// the inactive background, foreground, the cursor - always the session colour - and
/// selection) and its 16 ANSI colours, each a swatch with its own hex (team lead's
/// review: "label the rows and add the base colours").
fn terminal_theme_row(
    label: &'static str,
    theme: &foundations::TerminalTheme,
    muted: Hsla,
    border: Hsla,
) -> impl IntoElement {
    let mut entries = vec![
        ("bg", theme.bg),
        ("bg_inactive", theme.bg_inactive),
        ("fg", theme.fg),
        ("cursor (session)", theme.cursor),
        ("selection", theme.selection),
    ];
    entries.extend(ansi_entries(&theme.ansi));
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(div().text_size(px(10.0)).text_color(muted).child(label))
        .child(
            div().flex().flex_wrap().gap(px(4.0)).children(
                entries
                    .into_iter()
                    .map(|(name, colour)| colour_swatch(name, colour, muted, border, false)),
            ),
        )
}

struct Gallery {
    theme: Theme,
    focus_handle: FocusHandle,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>, appearance: Appearance) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);
        let theme = Theme::with_default_session(appearance);
        cx.set_global(Theme::with_default_session(theme.appearance()));
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
            self.theme = Theme::with_default_session(self.theme.appearance().toggled());
            cx.set_global(Theme::with_default_session(self.theme.appearance()));
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
                .map(|(name, colour)| colour_swatch(name, colour, text, border, true)),
        );

        let ramp = theme.ramp();
        let ramp_extended = theme.ramp_extended();
        let ramps = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(ramp_gradient(&ramp.stops, px(320.0), px(20.0), muted))
            .child(ramp_gradient(&ramp_extended, px(320.0), px(20.0), muted));

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

        let dark_terminal = foundations::steg_dark(theme.ramp_1_violet());
        let light_terminal = foundations::steg_light(theme.ramp_1_violet());
        let terminal_themes = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(terminal_theme_row(
                "steg dark",
                &dark_terminal,
                muted,
                border,
            ))
            .child(terminal_theme_row(
                "steg light",
                &light_terminal,
                muted,
                border,
            ));

        // Five columns, not one: this screen's own height is the binding constraint
        // on the gallery window (observed clamped well short of the room a single
        // column of this section's rows would need), and this section has spare width
        // to spend instead. Each column has its own fixed width, so a long value
        // wraps onto a second line within its column rather than running off the
        // window's edge (team lead's review of the first capture found exactly that,
        // with three unconstrained columns) - this makes it a layout invariant, not
        // something only re-checked by eye each time.
        let other_values = other_named_values(theme);
        let column_size = other_values.len().div_ceil(5);
        let other_column_width = px(220.0);
        let other = div()
            .flex()
            .gap(px(16.0))
            .children(other_values.chunks(column_size).map(|rows| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .w(other_column_width)
                    .children(rows.iter().map(|(name, value)| {
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_size(px(10.0)).text_color(muted).child(*name))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(text)
                                    .child(value.clone()),
                            )
                    }))
            }));

        div()
            .id("gallery-root")
            .track_focus(&self.focus_handle)
            .on_key_down(_cx.listener(Self::on_key_down))
            .size_full()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(24.0))
            .bg(theme.surface_ground())
            .text_color(text)
            .child(format!(
                "steg-ui gallery ({}; press t to switch)",
                if matches!(theme.appearance(), Appearance::Dark) {
                    "dark"
                } else {
                    "light"
                },
            ))
            .child(section("Colours", muted, colours))
            .child(section("Ramps", muted, ramps))
            .child(section("Spacing", muted, spacing))
            .child(section("Corners", muted, corners))
            .child(section("Type", muted, type_sizes))
            .child(section(
                "Terminal themes (dark, light)",
                muted,
                terminal_themes,
            ))
            .child(section("Other named values", muted, other))
    }
}

fn main() {
    let appearance = appearance_from_env(std::env::var("STEG_GALLERY_THEME").ok().as_deref());
    Application::new().run(move |cx: &mut App| {
        let bounds = WindowBounds::centered(size(px(1240.0), px(1078.0)), cx);
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
            Theme::with_default_session(Appearance::Dark).appearance(),
            Appearance::Dark
        );
        assert_eq!(
            Theme::with_default_session(Appearance::Light).appearance(),
            Appearance::Light
        );
    }
}
