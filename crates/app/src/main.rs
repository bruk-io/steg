//! Steg.app: a GPUI window with one terminal (TASK-1.117's bare, read-only window; typing,
//! resizing and the IME, TASK-1.121; the latency budget, TASK-1.122). A client of the
//! server like any other, through the sdk and the public API, with no private channel.

mod client;
mod ime;
mod keys;
mod metrics;
mod model;
mod size;
mod startup;
mod terminal_element;

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::thread::JoinHandle;

use futures::StreamExt as _;
use gpui::{
    App, Application, AsyncWindowContext, Bounds, Context, EntityInputHandler, FocusHandle,
    Focusable, IntoElement, KeyDownEvent, ParentElement as _, Pixels, Render, Styled as _,
    Subscription, Task, TitlebarOptions, UTF16Selection, Window, WindowAppearance, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};
use steg_ui::foundations::{self, Appearance, Theme};

use crate::client::{Client, ClientEvent, Writer};
use crate::ime::ImeState;
use crate::metrics::CellMetrics;
use crate::model::Model;
use crate::size::GridSize;
use crate::startup::{Attached, ShutdownSlot, StartupError};
use crate::terminal_element::RowCache;

/// Everything this window holds once attached: the model a background thread feeds, the
/// metrics and row cache painting needs, the client whose own `stop` ends the reader
/// thread, and TASK-1.121's own addition - the writer, the tab this attachment's size is
/// declared for, the size last actually declared (the coalescer's own state, `size.rs`),
/// and the terminal element's own last-painted bounds (read back by `bounds_for_range`).
struct AttachedState {
    model: Rc<RefCell<Model>>,
    metrics: CellMetrics,
    row_cache: Rc<RefCell<RowCache>>,
    client: Client,
    _receive_task: Task<()>,
    writer: Writer,
    declared_size: Cell<GridSize>,
    last_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    _resize_subscription: Subscription,
}

/// What the window shows right now (TASK-1.117's plan §3/§6).
enum Phase {
    Connecting,
    Attached(Box<AttachedState>),
    Disconnected(String),
}

pub struct AppWindow {
    phase: Phase,
    focus_handle: FocusHandle,
    appearance: Appearance,
    /// Joined as soon as [`startup::spawn`]'s own oneshot resolves, or by
    /// `on_window_should_close` if the window closes first - either way, before the
    /// window's own lifetime ends (TASK-1.117's plan: every thread started is joined on
    /// every path). `stop_startup` is the one place that takes it.
    startup_thread: Option<JoinHandle<()>>,
    /// What `stop_startup` passes to [`startup::interrupt`] to interrupt a still-running
    /// startup thread before joining it (`startup`'s own module doc explains the
    /// mechanism, the two races its sticky state resolves, and its one unavoidable gap -
    /// the bounded wait inside `connect()`'s own retry loop).
    startup_shutdown: ShutdownSlot,
    _startup_task: Task<()>,
    _appearance_subscription: Subscription,
    /// The IME's own composition state (TASK-1.121 plan §2) - meaningless before
    /// `Phase::Attached`, harmless either way (nothing reads it until then).
    ime: ImeState,
}

impl AppWindow {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        let weak = cx.weak_entity();
        let appearance_subscription = window.observe_window_appearance({
            let weak = weak.clone();
            move |window, cx| {
                let _ = weak.update(cx, |this: &mut Self, cx| {
                    this.appearance = appearance_from(window.appearance());
                    cx.notify();
                });
            }
        });

        let (startup_thread, startup_receiver, startup_shutdown) = startup::spawn();
        let startup_task = window.spawn(cx, async move |cx: &mut AsyncWindowContext| {
            let result = startup_receiver.await;
            let _ = weak.update_in(cx, move |this, window, cx| {
                this.apply_startup_result(result, window, cx);
            });
        });

        Self {
            phase: Phase::Connecting,
            focus_handle,
            appearance: appearance_from(window.appearance()),
            startup_thread: Some(startup_thread),
            startup_shutdown,
            _startup_task: startup_task,
            _appearance_subscription: appearance_subscription,
            ime: ImeState::default(),
        }
    }

    /// Joins the startup thread (it has already sent its one result, or the channel was
    /// dropped - either way it is finishing or finished) and moves the window into
    /// [`Phase::Attached`] or [`Phase::Disconnected`]. Joining here is safe and bounded
    /// (startup.rs's module doc): the oneshot has already resolved, so the thread's own
    /// closure, `sender.send(result)` then return, has nothing left to do - unlike
    /// `teardown`, which must never join a thread that might still be running.
    fn apply_startup_result(
        &mut self,
        result: Result<Result<Attached, StartupError>, futures::channel::oneshot::Canceled>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(thread) = self.startup_thread.take() {
            let _ = thread.join();
        }
        let attached = match result {
            Ok(Ok(attached)) => attached,
            Ok(Err(error)) => {
                self.phase = Phase::Disconnected(error.to_string());
                cx.notify();
                return;
            }
            Err(_) => {
                self.phase = Phase::Disconnected("startup ended with no result".into());
                cx.notify();
                return;
            }
        };

        let metrics = CellMetrics::measure(window);
        let model = Rc::new(RefCell::new(Model::from_snapshot(attached.snapshot)));
        let row_cache = terminal_element::row_cache();
        let (client, mut receiver) = Client::start(
            attached.stream,
            attached.connection,
            attached.shutdown_handle,
        );

        // The writer: a second, encode-only `Connection`, seeded the same way the reader's
        // was (`Connection::open_stream` on the same id) - TASK-1.121 plan §3, TASK-1.117's
        // plan §1's own slot for this. Its own `tab.declare-size` reply lands on the
        // reader's connection instead (`client.rs`'s own `drain`, which tolerates it).
        let mut writer_connection =
            steg_sdk::Connection::new(steg_sdk::WireRole::Client, steg_sdk::description());
        writer_connection.open_stream(attached.stream_id);
        let mut writer = Writer::new(
            attached.write_handle,
            writer_connection,
            attached.stream_id,
            attached.tab_target,
        );

        // Declared once on open (TASK-1.121 plan §4), from whatever the window's own
        // viewport already is - before any resize has happened, so the grid starts
        // filling the window rather than waiting for the first one.
        let declared_size = size::grid_size(
            f32::from(window.viewport_size().width),
            f32::from(window.viewport_size().height),
            f32::from(metrics.cell_width),
            f32::from(metrics.line_height),
        );
        writer.declare_size(declared_size.columns, declared_size.rows);

        let resize_subscription = cx.observe_window_bounds(window, |this, window, _cx| {
            this.on_resize(window);
        });

        let weak = cx.weak_entity();
        let receive_task = window.spawn(cx, async move |cx: &mut AsyncWindowContext| {
            while let Some(event) = receiver.next().await {
                let stop = weak
                    .update(cx, |this: &mut Self, cx| {
                        this.handle_client_event(event, cx)
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }
            }
        });

        self.phase = Phase::Attached(Box::new(AttachedState {
            model,
            metrics,
            row_cache,
            client,
            writer,
            declared_size: Cell::new(declared_size),
            last_bounds: Rc::new(Cell::new(None)),
            _resize_subscription: resize_subscription,
            _receive_task: receive_task,
        }));
        cx.notify();
    }

    /// Applies one event from the reader thread to the model, or moves the window into
    /// [`Phase::Disconnected`] - returns whether the receive loop should stop (TASK-1.117's
    /// plan §3: "the connection dropping on its own"). An `Update` while not `Attached`
    /// cannot happen today (the receive task is only ever started from
    /// `apply_startup_result`, once `self.phase` already is `Attached`, and nothing moves
    /// it back) - handled as a no-op rather than assumed away, so a later change that did
    /// break that invariant would silently drop a frame instead of panicking.
    fn handle_client_event(&mut self, event: ClientEvent, cx: &mut Context<Self>) -> bool {
        match event {
            ClientEvent::Update(update) => {
                if let Phase::Attached(state) = &self.phase {
                    state.model.borrow_mut().apply(update);
                    cx.notify();
                }
                false
            }
            ClientEvent::Disconnected => {
                self.phase = Phase::Disconnected("the server closed the connection".into());
                cx.notify();
                true
            }
        }
    }

    /// Stops the client (if attached) and, if the startup thread hasn't resolved yet,
    /// marks it cancelled and lets go of it - called from `on_window_should_close`, so
    /// closing the window never blocks the main thread on a thread that might still be
    /// running (startup.rs's module doc: a thread this side cannot be sure of unblocking
    /// must not be joined here). `interrupt` ends it sooner if it already has a socket to
    /// shut down, but either way the thread finishes on its own - bounded by its own read
    /// timeout once connected, or by `connect_timeout` while still inside `connect()`'s own
    /// retry loop - never by this call waiting for it. If `apply_startup_result` already
    /// ran, `startup_thread` is already `None` and this is a no-op for it.
    fn teardown(&mut self) {
        startup::interrupt(&self.startup_shutdown);
        drop(self.startup_thread.take());
        if let Phase::Attached(state) = &mut self.phase {
            state.client.stop();
        }
    }

    /// The resize coalescer's own window-level half (TASK-1.121 plan §4): computes the
    /// viewport's own grid size and, if it differs from what was last actually declared,
    /// declares it. A no-op before attachment (nothing to declare for yet) - registered as
    /// a `Subscription` only once attached (`apply_startup_result`), so this is reachable
    /// only while `Phase::Attached` anyway; the `let else` is defensive, not expected.
    fn on_resize(&mut self, window: &mut Window) {
        let Phase::Attached(state) = &mut self.phase else {
            return;
        };
        let viewport = window.viewport_size();
        let computed = size::grid_size(
            f32::from(viewport.width),
            f32::from(viewport.height),
            f32::from(state.metrics.cell_width),
            f32::from(state.metrics.line_height),
        );
        if let Some(next) = size::declare_if_changed(state.declared_size.get(), computed) {
            state.declared_size.set(next);
            state.writer.declare_size(next.columns, next.rows);
        }
    }

    /// Sends `bytes` as input, if attached - a no-op otherwise (there is no writer before
    /// `Phase::Attached`, and nowhere for a keystroke typed before then to go).
    fn send_bytes(&mut self, bytes: Vec<u8>) {
        if let Phase::Attached(state) = &mut self.phase {
            state.writer.send_input(bytes);
        }
    }

    /// The raw key path (TASK-1.121 plan §1): `keys::key_to_bytes` over the keystroke's own
    /// three public fields, sent if it maps to anything. Stops propagation exactly when it
    /// does - the one invariant that guarantees a keystroke this maps never also reaches
    /// `replace_text_in_range` (`keys.rs`'s own doc comment on the dedup rule GPUI's own
    /// macOS backend already enforces).
    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if let Some(bytes) = keys::key_to_bytes(
            &keystroke.key,
            keystroke.key_char.as_deref(),
            keystroke.modifiers,
        ) {
            self.send_bytes(bytes);
            cx.stop_propagation();
        }
    }
}

impl EntityInputHandler for AppWindow {
    /// No editable text buffer exists here (`ime.rs`'s own module doc) - an honest "nothing
    /// selectable" the IME already tolerates.
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        None
    }

    /// Real, not hard-coded `None` - this is what lets GPUI's own macOS backend ever learn
    /// a composition is open (`keys.rs`'s own doc comment on the dedup rule).
    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        if self.ime.is_composing() {
            Some(0..0)
        } else {
            None
        }
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.ime.unmark();
    }

    /// Committed text - the one path a resolved composition or a plain, non-composing
    /// insert ever takes (`ime.rs`'s own doc on `ImeState::commit`).
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bytes = self.ime.commit(text);
        self.send_bytes(bytes);
        cx.notify();
    }

    /// Marked (uncommitted) text: tracked so `is_composing` is real, sent nowhere (`ime.rs`'s
    /// own stated gap - no preview is drawn for it yet either).
    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ime.mark(new_text);
        cx.notify();
    }

    /// The terminal's own current cursor cell, so the IME's candidate window anchors there
    /// rather than the window's origin - `None` only before the first paint (`terminal_
    /// element::terminal`'s own doc on `last_bounds`).
    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let Phase::Attached(state) = &self.phase else {
            return None;
        };
        let bounds = state.last_bounds.get()?;
        let cursor = state.model.borrow().cursor();
        let metrics = state.metrics;
        Some(Bounds::new(
            gpui::point(
                bounds.origin.x + metrics.cell_width * f32::from(cursor.x),
                bounds.origin.y + metrics.line_height * f32::from(cursor.y),
            ),
            gpui::size(metrics.cell_width, metrics.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl Focusable for AppWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::with_default_session(self.appearance);
        match &self.phase {
            Phase::Connecting => placeholder(&theme, "connecting…"),
            Phase::Disconnected(message) => {
                placeholder(&theme, &format!("disconnected: {message}"))
            }
            Phase::Attached(state) => {
                let terminal_theme = match theme.appearance() {
                    Appearance::Dark => foundations::steg_dark(theme.session()),
                    Appearance::Light => foundations::steg_light(theme.session()),
                };
                div()
                    .size_full()
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(Self::on_key_down))
                    .child(terminal_element::terminal(
                        Rc::clone(&state.model),
                        state.metrics,
                        terminal_theme,
                        Rc::clone(&state.row_cache),
                        self.focus_handle.clone(),
                        cx.entity(),
                        Rc::clone(&state.last_bounds),
                    ))
            }
        }
    }
}

fn placeholder(theme: &Theme, message: &str) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.surface_ground())
        .text_color(theme.text())
        .child(message.to_owned())
}

fn appearance_from(appearance: WindowAppearance) -> Appearance {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = WindowBounds::centered(size(px(900.0), px(600.0)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                titlebar: Some(TitlebarOptions {
                    title: Some("steg".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| AppWindow::new(window, cx));
                window.on_window_should_close(cx, {
                    let view = view.clone();
                    move |_window, cx| {
                        view.update(cx, |view, _cx| view.teardown());
                        true
                    }
                });
                view
            },
        );
        if let Err(error) = result {
            eprintln!("steg-app: failed to open window: {error}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
