//! One terminal pane drawn in GPUI, written once and shared by both candidates so that the two
//! are drawn by identical code.
//!
//! Written from gpui 0.2.2's own `examples/` and its rustdoc. Zed's `terminal` and
//! `terminal_view` crates were not opened: they are GPL and steg is MIT (R59,
//! `.claude/rules/licence.md`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, Entity, Font, FontFeatures, FontStyle, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Pixels, Render, ShapedLine, SharedString,
    Styled as _, TextRun, Window, canvas, div, fill, point, px, rgb, rgba, size,
};

use crate::backend::{Rgb, Snapshot, TerminalBackend};
use crate::latency::Probe;
use crate::pty::Pty;

/// How often the pane looks for new output from the pseudo-terminal.
///
/// This polling is the one piece of added latency the measurement cannot avoid: gpui's own
/// contexts may only be touched from the main thread, so a background reader cannot wake the
/// view directly. It adds a uniform 0 to 200 microseconds, the same for both candidates, against
/// a frame interval of 8.3 ms. Every report says so.
pub const POLL: Duration = Duration::from_micros(200);

/// Font metrics, measured once from the text system rather than assumed.
pub struct Metrics {
    pub font: Font,
    pub font_size: Pixels,
    pub line_height: Pixels,
    cell_width: RefCell<Option<Pixels>>,
}

impl Metrics {
    pub fn new(family: &str, font_size: f32) -> Self {
        Self {
            font: Font {
                family: family.to_owned().into(),
                features: FontFeatures::default(),
                fallbacks: None,
                weight: FontWeight::NORMAL,
                style: FontStyle::Normal,
            },
            font_size: px(font_size),
            line_height: px((font_size * 1.3).round()),
            cell_width: RefCell::new(None),
        }
    }

    /// The advance of one cell, measured by shaping a run of one character and dividing.
    fn cell_width(&self, window: &mut Window) -> Pixels {
        if let Some(width) = *self.cell_width.borrow() {
            return width;
        }
        let probe: SharedString = "MMMMMMMMMM".into();
        let run = TextRun {
            len: probe.len(),
            font: self.font.clone(),
            color: Hsla::white(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window
            .text_system()
            .shape_line(probe, self.font_size, &[run], None);
        let width = line.width / 10.0;
        *self.cell_width.borrow_mut() = Some(width);
        width
    }
}

/// Shaped rows kept between frames, so a screen that did not change costs no shaping. Shared
/// through `Rc<RefCell<_>>` because gpui's paint callback is `'static` and cannot borrow the
/// view, and everything here lives on the main thread.
#[derive(Default)]
pub struct RowCache {
    lines: HashMap<u64, ShapedLine>,
    pub hits: u64,
    pub misses: u64,
}

impl RowCache {
    fn shaped(
        &mut self,
        key: u64,
        build: impl FnOnce() -> ShapedLine,
    ) -> ShapedLine {
        if let Some(line) = self.lines.get(&key) {
            self.hits += 1;
            return line.clone();
        }
        self.misses += 1;
        if self.lines.len() > 8192 {
            self.lines.clear();
        }
        let line = build();
        self.lines.insert(key, line.clone());
        line
    }
}

/// One terminal pane: a program on a pseudo-terminal, an emulator, and the drawing of it.
pub struct TerminalView<B: TerminalBackend + 'static> {
    backend: B,
    pty: Pty,
    metrics: Rc<Metrics>,
    cache: Rc<RefCell<RowCache>>,
    probe: Rc<RefCell<Probe>>,
    background: Rgb,
    foreground: Rgb,
    /// How much output this pane has fed to its emulator, and the cost of the worst single
    /// pump. A pump that hands over everything waiting blocks the main thread for as long as it
    /// takes, so this is where "printing a file freezes the app" would show up (R19).
    fed: Fed,
}

#[derive(Default)]
pub struct Fed {
    pub bytes: u64,
    pub pumps: u64,
    pub slowest_pump: Duration,
    pub largest_pump_bytes: usize,
}

impl<B: TerminalBackend + 'static> TerminalView<B> {
    /// Open a pane of the given size running `pty`.
    ///
    /// # Errors
    /// If the emulator cannot be created.
    pub fn new(
        pty: Pty,
        cols: u16,
        rows: u16,
        scrollback: usize,
        probe: Rc<RefCell<Probe>>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            backend: B::open(cols, rows, scrollback)?,
            pty,
            metrics: Rc::new(Metrics::new("SF Mono", 13.0)),
            cache: Rc::new(RefCell::new(RowCache::default())),
            probe,
            background: Rgb::new(0x0d, 0x0d, 0x12),
            foreground: Rgb::new(0xd4, 0xd4, 0xdc),
            fed: Fed::default(),
        })
    }

    pub fn probe(&self) -> Rc<RefCell<Probe>> {
        Rc::clone(&self.probe)
    }

    pub fn cache(&self) -> Rc<RefCell<RowCache>> {
        Rc::clone(&self.cache)
    }

    /// Send bytes to the program, as typing does.
    pub fn send(&mut self, bytes: &[u8]) {
        if let Err(error) = self.pty.send(bytes) {
            eprintln!("pty write failed: {error}");
        }
    }

    pub fn finished(&mut self) -> bool {
        self.pty.finished()
    }

    pub fn screen(&mut self) -> Snapshot {
        self.backend.snapshot()
    }

    /// What this pane has fed, and the worst single pump.
    pub fn fed(&self) -> &Fed {
        &self.fed
    }

    /// Start the loop that moves output from the pseudo-terminal into the emulator. Nothing is
    /// drawn until this runs, and it is the only thing that asks for a repaint.
    pub fn start(view: &Entity<Self>, window: &mut Window, cx: &mut App) {
        let weak = view.downgrade();
        window
            .spawn(cx, async move |cx| {
                loop {
                    cx.background_executor().timer(POLL).await;
                    let alive = weak
                        .update_in(cx, |pane, _window, cx| pane.pump(cx))
                        .is_ok();
                    if !alive {
                        break;
                    }
                }
            })
            .detach();
    }

    /// Take whatever the program has written and give it to the emulator.
    fn pump(&mut self, cx: &mut Context<Self>) {
        let chunks = self.pty.drain();
        if chunks.is_empty() {
            return;
        }
        let bytes: usize = chunks.iter().map(Vec::len).sum();
        let started = Instant::now();
        for chunk in &chunks {
            self.backend.feed(chunk);
        }
        let took = started.elapsed();
        self.fed.bytes += bytes as u64;
        self.fed.pumps += 1;
        self.fed.slowest_pump = self.fed.slowest_pump.max(took);
        self.fed.largest_pump_bytes = self.fed.largest_pump_bytes.max(bytes);
        if self.probe.borrow().waiting_for().is_some() {
            let screen = self.backend.snapshot();
            self.probe
                .borrow_mut()
                .note_arrival((screen.cursor_col, screen.cursor_row));
        }
        cx.notify();
    }

    fn paint_grid(
        screen: &Snapshot,
        metrics: &Metrics,
        cache: &RefCell<RowCache>,
        foreground: Rgb,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let cell_width = metrics.cell_width(window);
        let line_height = metrics.line_height;
        for (index, row) in screen.rows.iter().enumerate() {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a row index on a terminal screen is a small integer"
            )]
            let y = bounds.origin.y + line_height * (index as f32);
            if row.text.trim_end().is_empty() {
                continue;
            }
            let text: SharedString = row.text.trim_end().to_owned().into();
            let runs = style_runs(row, &text, metrics, foreground);
            let key = row_key(index, &text, &runs);
            let shaped = cache.borrow_mut().shaped(key, || {
                window
                    .text_system()
                    .shape_line(text.clone(), metrics.font_size, &runs, None)
            });
            let origin = point(bounds.origin.x, y);
            if let Err(error) = shaped.paint_background(origin, line_height, window, cx) {
                eprintln!("painting a row's background failed: {error}");
            }
            if let Err(error) = shaped.paint(origin, line_height, window, cx) {
                eprintln!("painting a row failed: {error}");
            }
        }
        if screen.cursor_visible {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a cursor position on a terminal screen is a small integer"
            )]
            let origin = point(
                bounds.origin.x + cell_width * (f32::from(screen.cursor_col)),
                bounds.origin.y + line_height * (f32::from(screen.cursor_row)),
            );
            window.paint_quad(fill(
                Bounds::new(origin, size(cell_width, line_height)),
                rgba(0x66_80_ff_8c),
            ));
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let bytes: Vec<u8> = match keystroke.key.as_str() {
            "enter" => b"\r".to_vec(),
            "backspace" => b"\x7f".to_vec(),
            "tab" => b"\t".to_vec(),
            "escape" => b"\x1b".to_vec(),
            "up" => b"\x1b[A".to_vec(),
            "down" => b"\x1b[B".to_vec(),
            "right" => b"\x1b[C".to_vec(),
            "left" => b"\x1b[D".to_vec(),
            _ => match &keystroke.key_char {
                Some(text) if !keystroke.modifiers.control => text.as_bytes().to_vec(),
                _ if keystroke.modifiers.control && keystroke.key.len() == 1 => {
                    let letter = keystroke.key.as_bytes()[0].to_ascii_lowercase();
                    vec![letter.wrapping_sub(b'a').wrapping_add(1)]
                }
                _ => Vec::new(),
            },
        };
        if !bytes.is_empty() {
            self.send(&bytes);
        }
    }
}

impl<B: TerminalBackend + 'static> Render for TerminalView<B> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Reading the grid back is part of every frame, and the renderer pays it: measured here
        // rather than hidden in the backend.
        let screen = self.backend.snapshot();
        let metrics = Rc::clone(&self.metrics);
        let cache = Rc::clone(&self.cache);
        let probe = Rc::clone(&self.probe);
        let foreground = self.foreground;
        div()
            .size_full()
            .bg(rgb(self.background.hex()))
            .on_key_down(cx.listener(Self::on_key))
            .child(canvas(
                |_bounds, _window, _cx| (),
                move |bounds, (), window, cx| {
                    Self::paint_grid(
                        &screen, &metrics, &cache, foreground, bounds, window, cx,
                    );
                    Probe::painted(&probe, window);
                },
            ))
    }
}

/// One text run per stretch of cells that share a colour, which is what a terminal row is.
fn style_runs(row: &crate::backend::Row, text: &str, metrics: &Metrics, default: Rgb) -> Vec<TextRun> {
    let mut runs: Vec<TextRun> = Vec::new();
    for (cell, ch) in row.cells.iter().zip(text.chars()) {
        let len = ch.len_utf8();
        let colour = if cell.ch == ' ' { default } else { cell.fg };
        let same = runs.last().is_some_and(|last| {
            last.color == hsla(colour) && last.background_color == background(cell.bg, default)
        });
        if same {
            if let Some(last) = runs.last_mut() {
                last.len += len;
                continue;
            }
        }
        runs.push(TextRun {
            len,
            font: if cell.bold {
                Font {
                    weight: FontWeight::BOLD,
                    ..metrics.font.clone()
                }
            } else {
                metrics.font.clone()
            },
            color: hsla(colour),
            background_color: background(cell.bg, default),
            underline: None,
            strikethrough: None,
        });
    }
    let shaped_len: usize = runs.iter().map(|run| run.len).sum();
    if shaped_len < text.len() {
        runs.push(TextRun {
            len: text.len() - shaped_len,
            font: metrics.font.clone(),
            color: hsla(default),
            background_color: None,
            underline: None,
            strikethrough: None,
        });
    }
    runs
}

fn background(colour: Rgb, _default: Rgb) -> Option<Hsla> {
    // The pane's own background already covers the default, so only a set background is painted.
    if colour.hex() == 0 {
        None
    } else {
        Some(hsla(colour))
    }
}

fn hsla(colour: Rgb) -> Hsla {
    rgb(colour.hex()).into()
}

fn row_key(index: usize, text: &str, runs: &[TextRun]) -> u64 {
    let mut hasher = DefaultHasher::new();
    index.hash(&mut hasher);
    text.hash(&mut hasher);
    for run in runs {
        run.len.hash(&mut hasher);
        run.color.h.to_bits().hash(&mut hasher);
        run.color.s.to_bits().hash(&mut hasher);
        run.color.l.to_bits().hash(&mut hasher);
        run.font.weight.0.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}
