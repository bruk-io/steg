//! The gallery (doc-17, R124): a program in this crate that shows every component this
//! package holds, in every state the design draws, light and dark. Nothing is built yet
//! (TASK-1.91), so it shows one placeholder - drawn directly with GPUI, no component of
//! this crate - that proves three things before any real component exists: the window
//! opens, the light/dark switch works, and `just capture-gallery` can get a picture of it
//! without a person at the screen.
//!
//! The only thing in `steg-ui` that calls `gpui::Application::run`; `src/` never opens a
//! window, and nothing in `cargo build`/`cargo test` executes this file's `main`.

use gpui::{
    App, Application, Context, FocusHandle, Focusable, KeyDownEvent, Render, TitlebarOptions,
    Window, WindowBounds, WindowOptions, div, prelude::*, rgb, size,
};

/// Which of the design's two forms the gallery shows.
///
/// A placeholder: there is no theme context yet, because foundations (TASK-1.93) hasn't
/// exported the design's light/dark pairs (R123). This enum, and `placeholder_colour`
/// below, are what TASK-1.93's real foundations replace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GalleryTheme {
    Light,
    Dark,
}

impl GalleryTheme {
    /// The placeholder's own colour for each theme - a literal, not a named value from
    /// foundations, because there is no foundations value yet (R120 binds a *component*
    /// of the four levels; this placeholder is the gallery's own chrome, not one).
    fn placeholder_colour(self) -> u32 {
        match self {
            GalleryTheme::Light => 0xfceee6,
            GalleryTheme::Dark => 0x1a1420,
        }
    }

    /// The placeholder's own text colour - legible against `placeholder_colour`, same
    /// caveat as that method.
    fn placeholder_text_colour(self) -> u32 {
        match self {
            GalleryTheme::Light => 0x1a1420,
            GalleryTheme::Dark => 0xfceee6,
        }
    }

    fn toggled(self) -> Self {
        match self {
            GalleryTheme::Light => GalleryTheme::Dark,
            GalleryTheme::Dark => GalleryTheme::Light,
        }
    }

    fn label(self) -> &'static str {
        match self {
            GalleryTheme::Light => "light",
            GalleryTheme::Dark => "dark",
        }
    }
}

/// Read the gallery's initial theme from `STEG_GALLERY_THEME` ("light" or "dark", any
/// case). Anything else, including unset, is `Light` - `just capture-gallery` sets this
/// explicitly for both of its runs, so it never relies on this default; a person running
/// `just run-gallery` by hand gets a predictable start instead of an error over a typo.
fn theme_from_env(value: Option<&str>) -> GalleryTheme {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("dark") => GalleryTheme::Dark,
        Some(_) | None => GalleryTheme::Light,
    }
}

struct Gallery {
    theme: GalleryTheme,
    focus_handle: FocusHandle,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>, theme: GalleryTheme) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);
        Self {
            theme,
            focus_handle,
        }
    }

    /// `t` toggles the placeholder's theme. Only a person looking at `just run-gallery`
    /// can press it; `just capture-gallery` instead launches the binary twice, once per
    /// `STEG_GALLERY_THEME` value, so the capture needs no synthetic keystroke (which
    /// would need its own, separate Accessibility permission grant this recipe does not
    /// take on).
    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "t" {
            self.theme = self.theme.toggled();
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("gallery-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(rgb(self.theme.placeholder_colour()))
            .text_color(rgb(self.theme.placeholder_text_colour()))
            .child(format!(
                "steg-ui gallery - nothing built yet ({}; press t to switch)",
                self.theme.label()
            ))
    }
}

fn main() {
    let theme = theme_from_env(std::env::var("STEG_GALLERY_THEME").ok().as_deref());
    Application::new().run(move |cx: &mut App| {
        let bounds = WindowBounds::centered(size(gpui::px(640.0), gpui::px(420.0)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                titlebar: Some(TitlebarOptions {
                    title: Some("steg-ui gallery".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| Gallery::new(window, cx, theme)),
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
        assert_eq!(theme_from_env(None), GalleryTheme::Light);
    }

    #[test]
    fn dark_is_recognised_in_any_case() {
        assert_eq!(theme_from_env(Some("dark")), GalleryTheme::Dark);
        assert_eq!(theme_from_env(Some("DARK")), GalleryTheme::Dark);
        assert_eq!(theme_from_env(Some("Dark")), GalleryTheme::Dark);
    }

    #[test]
    fn light_is_explicit_too() {
        assert_eq!(theme_from_env(Some("light")), GalleryTheme::Light);
    }

    #[test]
    fn an_unrecognised_value_falls_back_to_light_rather_than_failing() {
        assert_eq!(theme_from_env(Some("sepia")), GalleryTheme::Light);
    }

    #[test]
    fn toggling_is_its_own_inverse() {
        assert_eq!(GalleryTheme::Light.toggled(), GalleryTheme::Dark);
        assert_eq!(GalleryTheme::Dark.toggled(), GalleryTheme::Light);
    }
}
