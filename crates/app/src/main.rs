//! Steg.app: a GPUI window with one terminal, read-only (TASK-1.117, part 1 of three -
//! typing, resizing and the latency budget are later tasks). A client of the server like
//! any other, through the sdk and the public API, with no private channel.

mod client;
mod metrics;
mod model;
mod startup;
mod terminal_element;

use std::cell::RefCell;
use std::net::Shutdown;
use std::rc::Rc;
use std::sync::PoisonError;
use std::thread::JoinHandle;

use futures::StreamExt as _;
use gpui::{
    App, Application, AsyncWindowContext, Context, FocusHandle, Focusable, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Task, TitlebarOptions, Window,
    WindowAppearance, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use steg_ui::foundations::{self, Appearance, Theme};

use crate::client::{Client, ClientEvent};
use crate::metrics::CellMetrics;
use crate::model::Model;
use crate::startup::{Attached, ShutdownSlot, StartupError};
use crate::terminal_element::RowCache;

/// Everything this window holds once attached: the model a background thread feeds, the
/// metrics and row cache painting needs, and the client whose own `stop` ends the reader
/// thread.
struct AttachedState {
    model: Rc<RefCell<Model>>,
    metrics: CellMetrics,
    row_cache: Rc<RefCell<RowCache>>,
    client: Client,
    _receive_task: Task<()>,
}

/// What the window shows right now (TASK-1.117's plan §3/§6).
enum Phase {
    Connecting,
    Attached(AttachedState),
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
    /// What `stop_startup` locks to interrupt a still-running startup thread before
    /// joining it (review finding: `startup`'s own module doc explains the mechanism and
    /// its one unavoidable gap, the bounded wait inside `connect()`'s own retry loop).
    startup_shutdown: ShutdownSlot,
    _startup_task: Task<()>,
    _appearance_subscription: Subscription,
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
        }
    }

    /// Interrupts a still-running startup thread before joining it, and is a cheap no-op
    /// once it has already finished (`startup_thread` already `None`) - the one place
    /// `startup_thread`/`startup_shutdown` are touched, called from both
    /// `apply_startup_result` (the thread has already finished by then; this is just the
    /// join) and `teardown` (the thread may still be running - this is the fix the review
    /// asked for). Locks `startup_shutdown` *before* joining, never after: the whole point
    /// is to unblock a read the join would otherwise wait on forever.
    fn stop_startup(&mut self) {
        let Some(thread) = self.startup_thread.take() else {
            return;
        };
        if let Some(handle) = self
            .startup_shutdown
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = handle.shutdown(Shutdown::Both);
        }
        let _ = thread.join();
    }

    /// Joins the startup thread (it has already sent its one result, or the channel was
    /// dropped - either way it is finishing or finished) and moves the window into
    /// [`Phase::Attached`] or [`Phase::Disconnected`].
    fn apply_startup_result(
        &mut self,
        result: Result<Result<Attached, StartupError>, futures::channel::oneshot::Canceled>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_startup();
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

        self.phase = Phase::Attached(AttachedState {
            model,
            metrics,
            row_cache,
            client,
            _receive_task: receive_task,
        });
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

    /// Stops the client (if attached) and interrupts-then-joins the startup thread (if the
    /// window closes before it ever resolved) - called from `on_window_should_close`, so
    /// no thread or socket outlives the window on any path (TASK-1.117's plan).
    fn teardown(&mut self) {
        self.stop_startup();
        if let Phase::Attached(state) = &mut self.phase {
            state.client.stop();
        }
    }
}

impl Focusable for AppWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
                div().size_full().child(terminal_element::terminal(
                    Rc::clone(&state.model),
                    state.metrics,
                    terminal_theme,
                    Rc::clone(&state.row_cache),
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

// TASK-1.121: these modules are pure functions and state with their own tests, built ahead
// of the window plumbing (`terminal_element.rs`/`client.rs`, TASK-1.117's own branch, not yet
// merged here) that will call them. `#[cfg(test)]` while that wiring doesn't exist yet, so
// `cargo build`'s own real binary target carries no dead code to warn about; dropped once
// `on_key_event`/`paint`/the resize observer call them for real.
#[cfg(test)]
mod ime;
#[cfg(test)]
mod keys;
#[cfg(test)]
mod size;

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
