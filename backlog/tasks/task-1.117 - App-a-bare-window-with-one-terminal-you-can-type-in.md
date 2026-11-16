---
id: TASK-1.117
title: 'App: a bare window with one terminal you can type in'
status: In Progress
assignee:
  - '@bruk.w8'
created_date: '2026-11-16T12:54:00Z'
updated_date: '2026-11-16T17:35:00Z'
labels:
  - size-5
milestone: m-4
dependencies:
  - TASK-1.47
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.117-app-a-bare-window-with-one-terminal-you
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '8551f6bb93b8'
asdlc_plan_approved_size: size-5
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A thin slice cut on 2026-11-16 at the engineer's request, to have something visible and usable early and to learn early whether GPUI can draw a terminal within the latency budget (doc-10's open question), before the components, the bar and the pane layout exist. Deliberately bare: one window, one session, one pane; no bar, no tabs, no splits, no palette, no settings, no selection or copy, no scrollback view, no events subscription (it makes its own session and attaches, so it needs TASK-1.47 and TASK-1.114 and not TASK-1.46). doc-9 R19; doc-10 R20, R22, R24; doc-13 R59 (the view is written fresh, nothing taken from GPL code); decision-3 section 5; decision-11 (GPUI directly, pinned at 0.2.2; read its README and the examples text.rs, text_layout.rs, input.rs and painting.rs against the pinned source). The terminal's cell drawing is the app's own and uses the foundations' terminal themes and fonts through the theme; it is not one of the component crate's four levels unless the plan finds it should be (doc-17, out of scope). What this leaves for TASK-1.56: the latency budget as a pass or fail on a 120 Hz display, the 100 MB flood with a second pane, font settings, a size set by another client, and fitting the view into the pane layout. What it leaves for TASK-1.52: the app holding what the server reports through events, a second window, and reconnecting.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Running the app opens one window showing one terminal: it starts a server if none is running, makes a session, attaches to its pane, and draws the pane's rows, colours and cursor from the attachment's snapshot and updates, in the terminal theme the foundations give for the system's light or dark setting
- [ ] #2 Closing the window shuts the connection down, joins its reader, and leaves the session running on the server; running the app again attaches to the same session and shows its screen
- [ ] #3 The pull request carries an image of the window with a shell in it; a recipe starts the app; nothing in just check or CI opens a window; what can be tested without a window has tests with recorded controls
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer

This plan now covers **part 1 only** — "a bare window with a read-only live terminal." Parts 2 ("typing and resizing") and 3 ("measure key to paint, and bound the work per frame") are the team lead's own new tasks; what this plan said about them is kept as an appendix below, unchanged in substance, marked as moved, so a reader doesn't lose the reasoning that produced the split.

1. **The socket is read by a thread that blocks on it, never by anything that wakes on a timer.** Reworked below (§1) after the team lead's finding: no periodic wake-up anywhere in this design now. The short version: a dedicated `std::thread::spawn` blocks on a genuine `read()` with no timeout; a channel hands each decoded `ScreenUpdate` to a `window.spawn`-scheduled task, which `.await`s the matching receiver — genuinely parked, woken only by the channel's own waker, never polled. I read `gpui::executor`'s source to confirm this is sound: a `ForegroundExecutor`'s task is scheduled via `dispatcher.dispatch_on_main_thread(runnable)`, and that schedule function is required to be `Send + Sync` (`executor.rs`'s own `spawn_local_with_source_location` bound) — so the task's waker can be invoked from any thread, including this app's own reader thread, and it will correctly re-queue the task onto the main thread. This is not a guess; it's what the bound on that function means.

   **The channel is `futures::channel::mpsc`, and it is a new, direct dependency after all.** I checked what gpui re-exports, as asked, rather than assuming "already in `Cargo.lock`" settled it: `gpui.rs`'s own `pub use` lines re-export exactly `smol::Timer` and `util::smol_timeout`, nothing from `futures`, nothing channel-shaped. So `steg-app` cannot reach `futures::channel::mpsc`/`oneshot` through `gpui`'s own public surface and must declare its own dependency — still zero new *crates* in the licence graph (0.3.34 is already resolved in `Cargo.lock`, via `gpui`'s own `futures.workspace = true`), but a new line in both `crates/app/Cargo.toml` and `boundaries.toml`'s `[steg-app]`, named as such in §1 below.

   **Why a dedicated thread, not `cx.background_executor()`, and why that applies to startup too.** `BackgroundExecutor::num_cpus` (`executor.rs`) reports `num_cpus::get()` outside tests — a background task runs on a dispatcher backed by a small, fixed-size pool, not an unbounded one. A task that blocks on it forever (the reader) would permanently occupy one of very few worker threads for the whole life of the window, competing with anything else gpui's own background executor needs to run. That is the reason the reader thread is its own `std::thread::spawn`, outside gpui's pool entirely — and the same reasoning applies to `local_socket::connect`'s own retry loop during startup, which blocks with `thread::sleep` for up to `connect_timeout` (seconds, while a cold server boots): that is also a pool thread blocked for a real, if bounded, amount of time, for the identical reason. Startup therefore runs on its *own* dedicated thread too, not the background executor — stated in §1 below, so the design is symmetric rather than quietly exempting the one-time case.

2. **Two `Connection` instances, not one shared behind a lock — reads and writes don't actually need to share state for this task.** `Connection::poll`/`receive` (decode) never touch `next_id`, and `Connection::call`/`attach`/`detach`/`declare_size`/`send_data` (encode) never touch the decoder. The only field decode reads that encode writes is `open_streams`, and for this bare, one-attachment app that set is `{}` then `{stream}` for the rest of the process's life, set once, identically, on both sides, right after the one `pane.attach` reply — never renegotiated again while part 1 runs. So: the reader thread owns one `Connection` (decode-only, plus the one `open_stream` call made once during startup); and — because **part 1 makes no call at all after startup** (no `declare_size`, no further requests; closing the window simply drops or shuts down the socket, which TASK-1.114's own plan already proves the server treats the same as a clean detach, item #7) — no second `Connection` is even needed yet. I'm still answering "how reads and writes share it" concretely, because part 2 needs it: the main thread would own its *own* second `Connection` (encode-only, seeded the same way, `open_stream(stream)` called once from the same startup result), and send bytes directly over its own cloned write-handle. No shared mutable state, no `Arc<Mutex<_>>` (rust.md's own "last resort") needed in either part.

3. **Shutdown.** The reader thread's blocking `read()` is unblocked by calling `.shutdown(std::net::Shutdown::Both)` on a second `try_clone()`'d handle kept for exactly this (`shutdown()` on one fd ends a blocking `read()` on another fd of the same socket — standard POSIX behaviour, not gpui- or app-specific). On window close, the entity's teardown calls `.shutdown()` then `JoinHandle::join()`s the reader thread — plainly, `join()` itself has no timeout, so this is "join, which is now expected to return promptly" in production, not a bounded wait by construction. The *test* is what actually proves promptness, and it does so without needing `join()` to be bounded: poll `JoinHandle::is_finished()` in a short bounded loop (e.g. up to 1 s, a few ms between checks — an ordinary "eventually true" test idiom, not a periodic-wakeup design choice in the app itself) and only then call the real, now-instant `.join()`. §4 states this precisely.

4. **The fixed grid's own visible shape, so the engineer isn't surprised by the image.** §2 below states precisely what paints where when the window is bigger or smaller than the attached pane's own fixed size.

5. **The stale-pane case.** §6 below: `pane.list`'s own `program_status` field (already fetched during startup, no extra round trip) tells the app whether the found session's first tab's first pane is still running something; if not, it calls `tab.new` in the *same* session rather than attaching to a dead pane or minting a second same-labelled session.

6. **What's needed from the engineer for AC #6's image:** part 1 only needs a still image now (the recording moved to part 2, with typing); the same one-time, interactive Screen Recording permission `crates/ui/CLAUDE.md` already documents for `capture-gallery` is what a screenshot of this window needs too.

7. **Every proof item below is recorded through `asdlc record-check` as a failing run (the mechanism reverted) and then a passing run, against the tests as they stand at the end** — not a hand-run-and-reverted demonstration. Stated once here because it applies to every row in §7, not repeated per row.

## 1. Structure

**Where the code lives**, unchanged from before: `crates/app`, not `crates/ui` (doc-17's own carve-out for the terminal's cell drawing). `steg-ui` supplies only values through `Theme`: `foundations::steg_dark`/`steg_light`, `FONT_TERMINAL`/`TERMINAL`.

```
crates/app/src/
  main.rs              entry point: Application::run; shows a "connecting…" placeholder until
                       startup resolves, then builds the terminal entity
  startup.rs           connect/autostart, hello, find-or-make-and-fix-up session (§6), attach — all
                       blocking sdk calls, run once on their own dedicated thread, off the main thread
                       and off gpui's own background-executor pool (§"For the engineer" point 1)
  client.rs            owns the decode-side Connection + the reader thread + the channel + the
                       shutdown handle, from the point startup hands them over
  model.rs             pure data — Vec<DrawRow> + Cursor + the fixed (columns, rows) read once from
                       the attach snapshot; `apply(&mut self, ScreenUpdate)`; no gpui::Window/Context
                       param anywhere in this file
  metrics.rs            pure data — CellMetrics (cell_width, line_height) from a shaped probe string
  terminal_element.rs   the Element impl — request_layout/prepaint/paint, read-only, the row-shaping
                        cache; no on_key_down, no input handler (moved to part 2)
```

Two manifest changes not visible from this file list, both needed before anything above compiles, and both `just check-boundaries` (part of `just check`) catches on the first `record-check` if missed:
- `crates/app/Cargo.toml`: `steg-sdk = { path = "../sdk", features = ["local-socket"] }` (`local_socket::connect` is behind that feature); `serde_json` (building `Map<String, Value>` params, reading `Response::Ok(Value)`, `serde_json::from_slice::<ScreenUpdate>`); `futures` (`channel::mpsc`/`channel::oneshot` — not reachable through `gpui`'s own public surface, confirmed by reading `gpui.rs`'s own `pub use` lines, §"For the engineer" point 1 — but still adding no new *crate* to the licence graph, since 0.3.34 is already resolved in the workspace's own `Cargo.lock` via `gpui`'s own dependency on it, the same reasoning `[steg-server]`'s own `nix` entry already states for a dependency "already resolved in the workspace through portable-pty's own dependency on it").
- `boundaries.toml`'s `[steg-app]`: `allow = ["steg-sdk", "steg-ui", "gpui", "serde_json", "futures"]`, with a reason beside each new entry (the same shape `[steg]`'s own `serde_json` entry already states).

**Layers, and what crosses between threads:**

- *Startup* (its own `std::thread::spawn`, not `cx.background_executor()` — §"For the engineer" point 1 says why: its own blocking retry loop can run for real seconds against a fresh, slow-starting server, and that is exactly the kind of block the reader thread already can't take on the background executor's own small, fixed pool): `steg_sdk::local_socket::connect` → `Connection::send_hello`/write → block-read until `Incoming::Hello`, `parse_server_hello` → `Connection::call("session.list", {})`, filter by `label == "steg-app"` → §6's own fix-up (make one if none; replace a stale one's tab if its pane isn't running) → `Connection::call("tab.list", {target: session_id})` → `Connection::call("pane.list", {target: "{session}/{tab}"})` → `Connection::attach(Some(&format!("{session}/{tab}/{pane}")))`, block-read until the matching reply, decode `PaneAttached` → `Connection::open_stream(stream)` (only after the reply is in hand — decision-2 §3's ordering rule, confirmed from `wire/connection.rs`'s own doc on `Connection::attach`). The thread's own result (the connected `UnixStream`, the `Connection` — decode-only from here on — `PaneAttached.snapshot`, and `stream`, or an error) goes out over a `futures::channel::oneshot::Sender`; a `window.spawn`-scheduled task `.await`s the matching `oneshot::Receiver` (woken by the same thread-safe waker mechanism as the reader's own channel, never polled) before building the real terminal entity.
- *The reader thread* (a second, separate `std::thread::spawn`, started once startup's own thread hands over its result): owns the `UnixStream` and the `Connection` outright. Loop: blocking `read()` (no timeout — a thread genuinely asleep until the kernel has bytes for it, zero wake-ups while idle) → `connection.receive(&buf[..n])` → `while let Some(incoming) = connection.poll() { ... }`, decoding each `Incoming::StreamControl`'s payload to `ScreenUpdate` **on this thread** (offloading JSON parsing from the main thread entirely — directly serves criterion 5's bound, which this task's own appendix inherits) and forwarding it with `sender.unbounded_send(ClientEvent::Update(update))` — a plain, synchronous, non-blocking call, legal from any thread. On `read()` returning `Ok(0)` or any error other than being interrupted, or on any `Incoming` this read-only app's own design doesn't expect (`Reply`/`Request`/`Event`/`UnknownStream`/`Malformed` — part 1 makes no call after startup, so none of these should ever arrive; if one does, that's treated as a protocol-level surprise, not specially recovered from), the thread sends `ClientEvent::Disconnected` once and exits.
- *The foreground tasks* (`window.spawn`, i.e. `ForegroundExecutor::spawn` — confirmed main-thread-only and confirmed wakeable from any thread, §"For the engineer" point 1): the startup-awaiting task above, and a second, longer-lived one that owns the `UnboundedReceiver<ClientEvent>` once the entity exists: `while let Some(event) = receiver.next().await { ... }` — genuinely suspended between events, not re-polled on any interval. `Update(update)` → `model.apply(update)` → `cx.notify()`. `Disconnected` → flips the entity into the "disconnected" placeholder state (§6) and ends the loop. **Both `Task<()>` values are stored as fields on the entity** (`gpui::Task` is `#[must_use]` and cancels on drop, `executor.rs`'s own doc on `Task::drop` — an untracked, un-detached task would simply stop running the first time nothing else held it, silently ending the update loop and freezing the window), so they live exactly as long as the entity does and end, deliberately, when it is dropped — dovetailing with the reader thread's own teardown below.
- *Shutdown*: the entity's teardown (its `Drop`, or `on_window_should_close`, `window.rs:4329`) calls `.shutdown(Shutdown::Both)` on the kept handle, then `JoinHandle::join()`s the reader thread (expected prompt, not bounded by the API itself — §"For the engineer" point 3, §4's test is what actually proves promptness).

## 2. Drawing

Unchanged from before except where the fixed grid (no declared size in part 1) determines what paints where — stated here precisely, per the team lead's own ask.

**Cell grid, from the font's metrics.** `metrics.rs`: `Font` from `foundations::FONT_TERMINAL` (first family, rest fallbacks) at `foundations::TERMINAL.size`; `line_height = foundations::TERMINAL.line_height` (both straight from foundations, not re-derived); `cell_width` measured once by shaping ten `'M'`s and dividing by ten (`spikes/.../element.rs`'s own `Metrics::cell_width`, steg's own prior MIT code, cited not copied verbatim — written fresh for this crate).

**The grid's own size is fixed, for all of part 1's life, to what `pane.attach`'s snapshot reports.** No `Screen.size` field exists on the TASK-1.47 branch alone (confirmed by reading it) — `model.rs` derives `(columns, rows)` once from the snapshot itself: `rows.len()` for rows, the first row's own total cell count (summing each `Run`'s cell span, same column bookkeeping as painting uses) for columns. This is the size the kernel already gave the pane at `session.new`/`tab.new` (TASK-1.76's own default), not anything this window asks for.

**What that looks like on screen, concretely.** The element always sizes itself `size_full()` (fills the window) and paints the **whole window's background** in the terminal theme's own `bg` first (`steg_dark`/`steg_light(theme.session())`, picked by `theme.appearance()`) — not the design's own `surface_ground`, since there is no chrome in this bare window for that colour to belong to, and one flat terminal-coloured field reads as one pane rather than two colours competing. The grid itself paints from the element's own top-left origin, at its natural pixel size (`columns * cell_width` × `rows * line_height`): if the window is **larger** than that, the extra space to the right and below is empty terminal background, nothing drawn into it — a window that doesn't fill with the terminal is the known, stated shape of part 1's own screenshot (part 2's declared/tracked size, once it exists, is what makes the grid fill the window). If the window is **smaller**, painting stops at whichever row/column no longer fits the element's own bounds (a bounds check in the paint loop, not a panic or an attempt to scale text) — part 1 never resizes the pane to match, so a small window simply crops what it shows.

**Runs, by column position, not byte offset; `CellStyle` → `TextRun`; the cursor** — unchanged from before:
- Column tracking: each run's own cell span (grapheme count, plus any `SpacerTail` immediately after a `Wide` run) advances a running column counter; every run paints at `x = origin.x + cell_width * column`, never accumulated `ShapedLine::width`.
- `fg`/`bg`: `Some(Rgb)` is the server's own resolved colour (`gpui::rgb(..).into()`, a data conversion, not a design value — R120's check is scoped to `crates/ui`); `None` resolves through the theme's own `TerminalTheme`.
- `inverse`: swap resolved fg/bg before building the run. `bold`/`italic`: `Font.weight`/`style`. `underline`: `UnderlineStyle{color, thickness: px(1.0), wavy: matches!(Underline::Curly)}` — gpui 0.2.2 only distinguishes solid/wavy (`text_system.rs:733`'s own field list), so `Single`/`Double`/`Dotted`/`Dashed` all paint solid, stated as an approximation doc-9's acceptance set doesn't depend on telling apart. `strikethrough`: the matching `StrikethroughStyle`. `overline`/`blink`: dropped, named as a deliberate, stated gap (gpui 0.2.2's `TextRun` has neither field nor an established technique for either). `invisible`: an empty-text run the width of what it hides. `faint`: reduced foreground alpha. `wide`: column bookkeeping only. `link`: draws nothing (doc-9 R116's address-on-demand lookup belongs to TASK-1.57).
- Cursor: a quad sized to its own cell (two if `Wide`), shape per `CursorShape`, drawn only when `visible`, coloured `theme.session()` via the terminal theme's own `cursor` field. `blinking` is the same stated gap as `CellStyle.blink`.

**What repaints, and the bound this task owns (the appendix's criterion 5 measures it later; this part builds it).** `model.apply` only ever touches the rows a `ScreenUpdate::Delta` names, or replaces the whole grid on `ScreenUpdate::Full` — O(rows changed), never O(screen), which the server already guarantees is at most one coalesced update per tick regardless of how fast a program prints (decision-2 §4.1). What this task's own code must not undo: the reader thread already drains every queued `Incoming` per wake before the foreground task applies them (so a backlog collapses into however many `cx.notify()` calls the events actually need — in this read-only part there is exactly one update per wake, since the channel itself is the queue now, not a polled buffer); and the row-shaping cache (`terminal_element.rs`, keyed on a hash of a row's content and style, so an unchanged row costs no reshaping even though the whole grid repaints each frame — gpui 0.2.2 has no partial-repaint primitive for a `canvas` element).

## 3. Lifecycle, and the stale-pane case

**Running the app the first time:** `startup.rs`'s `session.list` finds nothing labelled `"steg-app"` → `session.new(label: "steg-app", program: None)` (server decides the shell, doc-9 R13) → its one tab, its one pane → attach.

**Running it again, the pane still running:** the same label is found; its first tab's first pane's own `program_status` (`pane.list`'s own field, fetched for free, no extra round trip) reads `"running"` → attach to it directly. The same screen (and, once part 2 lands, the same typed history) is what AC #3's own evidence rests on — though part 1's own demonstration of this (a bare prompt, unchanged, twice) is admittedly weak on its own, which is why the team lead moved AC #3's own criterion to part 2, where `echo hello` surviving a restart is real evidence; this task still builds the mechanism AC #3 needs, just doesn't carry the criterion itself.

**Running it again, the pane's program has ended** (the person typed `exit`, or it crashed): `program_status` reads anything other than `"running"`. Attaching to a dead pane would show whatever it last displayed with no live program behind it, and minting a fresh `session.new` would leave the old, dead session around under the same label — the next restart's own `session.list` filter would then find **two** sessions labelled `"steg-app"`, and "take the first, in the server's own order" stops meaning anything useful. Instead: call `tab.new(target: session_id)` (no `program` named, so the server starts a fresh login shell the same way `session.new` would have) **in the same session**, and attach to its new pane. One label, one session, ever — a dead pane is replaced with a live one in place, not left to accumulate.

**Closing the window:** §1's own shutdown sequence (`.shutdown()`, join the reader thread) — no `pane.detach` call, since part 1 makes no further calls after startup at all and a hard-closed connection is already handled identically by the server (TASK-1.114's own proof item #7). The session and its pane carry on.

**The connection dropping on its own** (the server process ends, or the socket errors): the reader thread's `ClientEvent::Disconnected` flips the window to a plain "disconnected" placeholder (the same div used for "connecting…" before attach, not a second one) and nothing reconnects automatically — TASK-1.52's, as the task's own description already says.

## 4. Proof

Every item below is recorded through `asdlc record-check` as a failing run (the mechanism reverted) and a passing run, against the tests as they stand at the end (§"For the engineer" point 7).

**Pure data and functions, no window, no socket:**
- `model::apply` — a `ScreenUpdate::Full` establishes the fixed `(columns, rows)`; a following `Delta` that changes two rows asserts the result touches only those two rows and the model's own `(columns, rows)` is unchanged (there is no `size` field on this branch's `ScreenUpdate` to even read — the fixed-size claim is really "nothing in `apply` ever recomputes `(columns, rows)` from a `Delta`'s own row content"); control: make `apply` derive `(columns, rows)` from a `Delta`'s own rows and the fixed-size assertion fails.
- `metrics::columns_rows`-equivalent for the grid's own natural pixel size (used by `terminal_element.rs` to decide what fits, §2's own crop/letterbox rule) — a handful of `(columns, rows, cell metrics)` triples against the expected pixel size; control: swap a multiplication for an addition and the test fails.
- The run-to-column bookkeeping (§2: a `SpacerTail` contributes no text but still advances one column) — feed a row containing a `Wide` run followed by its `SpacerTail` and assert the next run's painted column, not byte offset; control: drop the `SpacerTail`'s own column contribution and the test fails.

**A real socket, no server, no window** (`UnixStream::pair()` — a real, local, in-process socket, not a double, same standing testing-rust.md already gives a real temporary directory or socket): feed bytes representing a length-prefixed control frame carrying a `ScreenUpdate` into one half, standing in for the server; assert the matching `ClientEvent::Update` arrives on the channel. A second test proves the reader thread actually stops: call the shutdown handle, then poll `JoinHandle::is_finished()` in a short bounded loop (e.g. up to 1 s total, a few ms between checks — `std::thread::JoinHandle::join` itself has no timeout, so this bounded poll is what the test uses to express "promptly," not a production mechanism) and assert it becomes `true` before the bound, only then calling the now-instant `.join()`; control for the first: stop the reader thread from decoding `StreamControl` payloads and the event never arrives; control for the second: remove the `.shutdown()` call and the bounded loop never observes `is_finished()`, failing the test rather than hanging it.

**What's shown by running it.** `just run` already exists (`justfile:208`); no new recipe needed — this task's own image isn't deterministic enough to automate the way the gallery's capture is (it needs a real server, session and login shell, not just a window opening). The engineer runs `just run` once and captures a screenshot (the same one-time Screen Recording permission `crates/ui/CLAUDE.md` already documents). Nothing in `just check`/CI opens a window: every file above compiles under `just check-packages` like any other crate's code, and `main.rs`'s own `Application::run` only ever executes when a person runs the binary.

---

## Appendix: parts 2 and 3, moved to their own tasks, kept here for the reasoning that produced them

(Unchanged from the original plan in substance; summarised rather than repeated in full, since it now lives on the team lead's own new task records.)

**Part 2 — typing and declared size.** Key-to-bytes as a pure function (`enter`→`\r`, `backspace`→`\x7f`, arrows in normal mode only, control+letter and the standard control-character table for `[`/`\`/`]`/`^`/`_`/space, nothing sent for anything else — R29 holds trivially since this app binds zero `KeyBinding`s). **Option key: the platform's composed character, not Meta** — decided, not left open, because a dead-key sequence only resolves through macOS's own input context (`platform/mac/window.rs:1693-1746`), which needs a minimal `EntityInputHandler` registered at all; Meta-as-Option is a competing convention that would need its own later setting, and the cost of choosing composed-character now (Alt+b/Alt+f-style readline word motions don't work out of the box) is named for Bruk to weigh. `tab.declare_size` on open and on resize (no explicit debounce — GPUI's own vsync-paced render cadence plus decision-2's 0.06-0.07 ms p95 control-call cost already bounds it), and the model switching from part 1's fixed size to drawing whatever size each update reports (`ScreenUpdate::Delta.size`/`Screen.size`, TASK-1.114's own additions) rather than the size last declared. Part 2's own writer is the second `Connection` instance §1 above already designed for: owned by the main thread, seeded from the same startup result, writing directly over its own cloned handle — no channel, no lock, because only the main thread ever builds a new request after startup in part 2 either (the reader thread still only ever decodes).

**Part 3 — the latency budget and the flood bound, measured.** Four stamps (key arrival, send, echo arrival by cursor-position match at an idle prompt — stated limit: only sound with one keystroke in flight and no concurrent program output, so the run is done at an idle prompt, not "while using the terminal" — and frame painted via `window.on_next_frame`), key-to-paint and key-to-frame printed as p50/p95 and recorded in the notes beside doc-9 R19, as a measurement, not a pass/fail. The honest floor stated up front: `DEFAULT_ATTACH_TICK` is 8 ms and not configurable from outside this process today (confirmed: `crates/steg/src/main.rs`'s `run_server_start` builds `ServerConfig::default()` with no override), so a single keystroke at an idle pane waits up to a full 8 ms before the server even computes its echo — before gpui's own paint path or the display's refresh wait are counted at all — so the expectation going in is that key-to-paint p95 lands above R19's revised 12 ms budget, and a second measurement at a lower tick isn't this task's to produce (nothing exposes the knob yet; a follow-up, not this task, should add one). The per-frame bound under a flood is proved by feeding `model::apply` a long run of single-row `Delta`s (the shape the server's own one-update-per-tick coalescing would actually deliver, not raw bytes — this app never parses raw bytes, unlike the TASK-1.1 spike) and asserting it never touches more than the rows named.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan (8551f6bb93b8) as rewritten for the read-only window alone, checked against doc-9, doc-10 (R20, R22, R24), doc-13 R59, doc-16, doc-17 and decision-11 by @bruk.o, 2026-11-16: ok. The split into three is taken (typing and resizing; measurement and the per-frame bound: two drafts). Revised at the orchestrator's request: the socket is not polled; a thread blocks on the read and hands each update through a channel to a task on the window's executor, woken by the channel. Needs only TASK-1.47. Adds futures as a direct dependency of the app (already in the lock file through gpui). Draws a fixed grid from the attach snapshot until the typing task brings the size.

Check passed: `just check` exited 0 at 2026-11-16T16:48:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at d1c726dbd094:

```text
ok      format check (0.2s)
ok      lint (89.8s)
ok      boundaries (0.1s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
skip    tests: this change touches no crate
ok      build (steg-sdk) (3.1s)
ok      test (steg-sdk) (1.8s)
ok      test (steg-sdk, local-socket) (2.2s)
ok      build (steg-kernel) (47.4s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (43.7s)
ok      test (steg-server) (13.7s)
ok      test (steg-server, race-fixture) (14.4s)
ok      build (steg) (2.2s)
ok      test (steg) (0.5s)
ok      test (steg, fixtures) (4.4s)
ok      build (steg-ui) (20.1s)
ok      test (steg-ui) (1.2s)
ok      build (steg-app) (1.8s)
ok      test (steg-app) (0.8s)
ok      tool tests (12.9s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `just check` exited 1 at 2026-11-16T17:06:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes, last 40 of 64 lines:

```text
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app/src/terminal_element.rs:107:
                     .borrow_mut()
                     .shaped(key, || shape_run(window, &run.text, text_run));
                 let origin = point(bounds.origin.x + metrics.cell_width * f32::from(column), y);
[31m-                if let Err(error) = shaped.paint_background(origin, metrics.line_height, window, cx) {
(B[m[32m+                if let Err(error) = shaped.paint_background(origin, metrics.line_height, window, cx)
(B[m[32m+                {
(B[m                     eprintln!("painting a run's background failed: {error}");
                 }
                 if let Err(error) = shaped.paint(origin, metrics.line_height, window, cx) {
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app/src/terminal_element.rs:194:
 fn shape_run(window: &mut Window, text: &str, run: TextRun) -> ShapedLine {
     let text: SharedString = text.to_owned().into();
     let font_size = steg_ui::foundations::TERMINAL.size;
[31m-    window.text_system().shape_line(text, font_size, &[run], None)
(B[m[32m+    window
(B[m[32m+        .text_system()
(B[m[32m+        .shape_line(text, font_size, &[run], None)
(B[m }
 
 fn run_key(row: usize, column: u16, run: &Run) -> u64 {
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app/src/terminal_element.rs:202:
     row.hash(&mut hasher);
     column.hash(&mut hasher);
     run.text.hash(&mut hasher);
[31m-    run.style.fg.map(|rgb| (rgb.r, rgb.g, rgb.b)).hash(&mut hasher);
(B[m[31m-    run.style.bg.map(|rgb| (rgb.r, rgb.g, rgb.b)).hash(&mut hasher);
(B[m[32m+    run.style
(B[m[32m+        .fg
(B[m[32m+        .map(|rgb| (rgb.r, rgb.g, rgb.b))
(B[m[32m+        .hash(&mut hasher);
(B[m[32m+    run.style
(B[m[32m+        .bg
(B[m[32m+        .map(|rgb| (rgb.r, rgb.g, rgb.b))
(B[m[32m+        .hash(&mut hasher);
(B[m     run.style.bold.hash(&mut hasher);
     run.style.italic.hash(&mut hasher);
     run.style.inverse.hash(&mut hasher);
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 142 with exit code 1
```

Check passed: `just check` exited 0 at 2026-11-16T17:08:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (1.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (13.5s)
ok      build (steg-sdk) (1.1s)
ok      test (steg-sdk) (2.3s)
ok      test (steg-sdk, local-socket) (2.6s)
ok      build (steg-kernel) (1.3s)
ok      test (steg-kernel) (1.3s)
ok      build (steg-server) (1.2s)
ok      test (steg-server) (15.1s)
ok      test (steg-server, race-fixture) (14.3s)
ok      build (steg) (2.1s)
ok      test (steg) (0.4s)
ok      test (steg, fixtures) (4.5s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.9s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.8s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-16T17:22:07+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (44.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.4s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (11.8s)
ok      test (steg-server, race-fixture) (12.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (3.0s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (11.9s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check FAILED: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app a_delta_touches_only_the_rows_it_names_and_never_resizes_the_grid'` exited 101 at 2026-11-16T17:22:40+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.71s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test model::tests::a_delta_touches_only_the_rows_it_names_and_never_resizes_the_grid ... FAILED

failures:

---- model::tests::a_delta_touches_only_the_rows_it_names_and_never_resizes_the_grid stdout ----

thread 'model::tests::a_delta_touches_only_the_rows_it_names_and_never_resizes_the_grid' (44428535) panicked at crates/app/src/model.rs:245:9:
assertion `left == right` failed
  left: 1
 right: 3
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    model::tests::a_delta_touches_only_the_rows_it_names_and_never_resizes_the_grid

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app a_delta_touches_only_the_rows_it_names_and_never_resizes_the_grid'` exited 0 at 2026-11-16T17:22:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.73s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test model::tests::a_delta_touches_only_the_rows_it_names_and_never_resizes_the_grid ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

Check FAILED: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app a_blank_first_row_does_not_change_the_row_count'` exited 101 at 2026-11-16T17:23:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.73s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test model::tests::a_blank_first_row_does_not_change_the_row_count ... FAILED

failures:

---- model::tests::a_blank_first_row_does_not_change_the_row_count stdout ----

thread 'model::tests::a_blank_first_row_does_not_change_the_row_count' (44432639) panicked at crates/app/src/model.rs:216:9:
assertion `left == right` failed
  left: 0
 right: 2
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    model::tests::a_blank_first_row_does_not_change_the_row_count

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app a_blank_first_row_does_not_change_the_row_count'` exited 0 at 2026-11-16T17:23:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.73s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test model::tests::a_blank_first_row_does_not_change_the_row_count ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

Check FAILED: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app a_pixel_size_that_does_not_divide_evenly_floors_not_rounds'` exited 101 at 2026-11-16T17:23:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.68s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test metrics::tests::a_pixel_size_that_does_not_divide_evenly_floors_not_rounds ... FAILED

failures:

---- metrics::tests::a_pixel_size_that_does_not_divide_evenly_floors_not_rounds stdout ----

thread 'metrics::tests::a_pixel_size_that_does_not_divide_evenly_floors_not_rounds' (44437452) panicked at crates/app/src/metrics.rs:105:9:
assertion `left == right` failed
  left: 11
 right: 10
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    metrics::tests::a_pixel_size_that_does_not_divide_evenly_floors_not_rounds

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app a_pixel_size_that_does_not_divide_evenly_floors_not_rounds'` exited 0 at 2026-11-16T17:23:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.70s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test metrics::tests::a_pixel_size_that_does_not_divide_evenly_floors_not_rounds ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

Check FAILED: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app a_wide_runs_columns_are_twice_its_own_grapheme_count'` exited 101 at 2026-11-16T17:23:56+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.68s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test model::tests::a_wide_runs_columns_are_twice_its_own_grapheme_count ... FAILED

failures:

---- model::tests::a_wide_runs_columns_are_twice_its_own_grapheme_count stdout ----

thread 'model::tests::a_wide_runs_columns_are_twice_its_own_grapheme_count' (44441731) panicked at crates/app/src/model.rs:194:9:
assertion `left == right` failed
  left: 2
 right: 4
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    model::tests::a_wide_runs_columns_are_twice_its_own_grapheme_count

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app a_wide_runs_columns_are_twice_its_own_grapheme_count'` exited 0 at 2026-11-16T17:24:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.69s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test model::tests::a_wide_runs_columns_are_twice_its_own_grapheme_count ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

Check FAILED: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app a_control_frame_on_the_open_stream_arrives_as_an_update'` exited 101 at 2026-11-16T17:24:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes, last 40 of 56 lines:

```text
    = note: `#[warn(unreachable_code)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `sender`
   --> crates/app/src/client.rs:100:39
    |
100 | fn drain(connection: &mut Connection, sender: &UnboundedSender<ClientEvent>) -> bool {
    |                                       ^^^^^^ help: if this is intentional, prefix it with an underscore: `_sender`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `update`
   --> crates/app/src/client.rs:102:13
    |
102 |         let update = match incoming {
    |             ^^^^^^ help: if this is intentional, prefix it with an underscore: `_update`

warning: `steg-app` (bin "steg-app" test) generated 3 warnings (run `cargo fix --bin "steg-app" -p steg-app --tests` to apply 2 suggestions)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.70s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::a_control_frame_on_the_open_stream_arrives_as_an_update ... FAILED

failures:

---- client::tests::a_control_frame_on_the_open_stream_arrives_as_an_update stdout ----

thread 'client::tests::a_control_frame_on_the_open_stream_arrives_as_an_update' (44445816) panicked at crates/app/src/client.rs:186:9:
assertion failed: matches!(event, Some(ClientEvent::Update(ScreenUpdate::Unchanged)))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    client::tests::a_control_frame_on_the_open_stream_arrives_as_an_update

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app a_control_frame_on_the_open_stream_arrives_as_an_update'` exited 0 at 2026-11-16T17:24:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::a_control_frame_on_the_open_stream_arrives_as_an_update ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

Check FAILED: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound'` exited 101 at 2026-11-16T17:24:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes, last 40 of 43 lines:

```text
  |
6 | use std::net::Shutdown;
  |     ^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: field `shutdown_handle` is never read
  --> crates/app/src/client.rs:29:5
   |
28 | pub struct Client {
   |            ------ field in this struct
29 |     shutdown_handle: UnixStream,
   |     ^^^^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-app` (bin "steg-app" test) generated 2 warnings (run `cargo fix --bin "steg-app" -p steg-app --tests` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.70s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound ... FAILED

failures:

---- client::tests::stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound stdout ----

thread 'client::tests::stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound' (44450864) panicked at crates/app/src/client.rs:179:14:
did not finish within the test's own bound: Timeout
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    client::tests::stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 2.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 && cargo test -p steg-app stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound'` exited 0 at 2026-11-16T17:25:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
```

Check passed: `just check` exited 0 at 2026-11-16T17:26:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at ec8d9f46e04c with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.1s)
ok      design pages (0.0s)
ok      licences (0.6s)
ok      tests (every crate) (12.8s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (11.5s)
ok      test (steg-server, race-fixture) (10.5s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (3.0s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.9s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.7s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check FAILED: `just check` exited 101 at 2026-11-16T17:27:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at 6a1b69b8f805, last 40 of 65 lines:

```text
2761 |         let handle = crate::pane::start(
 ...
2770 |             events_tx,
2771 ~             /* std::time::Duration */,
2772 ~         )
     |

error[E0061]: this function takes 5 arguments but 4 arguments were supplied
    --> crates/server/src/runtime.rs:3088:22
     |
3088 |           let handle = crate::pane::start(
     |  ______________________^^^^^^^^^^^^^^^^^^-
3089 | |             LaunchSpec {
3090 | |                 argv: vec![
3091 | |                     "sh".to_string(),
...    |
3101 | |             events_tx,
3102 | |         )
     | |_________- argument #5 of type `std::time::Duration` is missing
     |
note: function defined here
    --> crates/server/src/pane.rs:707:8
     |
 707 | pub fn start(
     |        ^^^^^
...
 712 |     attach_tick: Duration,
     |     ---------------------
help: provide the argument
     |
3088 |         let handle = crate::pane::start(
 ...
3101 |             events_tx,
3102 ~             /* std::time::Duration */,
3103 ~         )
     |

error: could not compile `steg-server` (lib test) due to 10 previous errors
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-16T17:32:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at 32a4b6b1945d, last 40 of 47 lines:

```text
   --> crates/server/src/connection.rs:840:28
    |
840 |             let _cleanup = ConnectionCleanup {
    |                            ^^^^^^^^^^^^^^^^^ missing `events`

error[E0061]: this function takes 4 arguments but 3 arguments were supplied
   --> crates/server/src/listen.rs:752:23
    |
752 |           let runtime = crate::runtime::Runtime::new(
    |  _______________________^^^^^^^^^^^^^^^^^^^^^^^^^^^^-
753 | |             std::sync::Arc::new(crate::pty::RealLauncher),
754 | |             crate::runtime::LoginShell::default(),
755 | |             Duration::from_millis(1),
756 | |         );
    | |_________- argument #4 of type `std::time::Duration` is missing
    |
note: associated function defined here
   --> crates/server/src/runtime.rs:269:12
    |
269 |     pub fn new(
    |            ^^^
...
273 |         attach_tick: Duration,
    |         ---------------------
help: provide the argument
    |
752 |         let runtime = crate::runtime::Runtime::new(
...
755 |             Duration::from_millis(1),
756 ~             /* std::time::Duration */,
757 ~         );
    |

    Checking steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117/crates/steg)
Some errors have detailed explanations: E0061, E0063.
For more information about an error, try `rustc --explain E0061`.
error: could not compile `steg-server` (lib test) due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-16T17:35:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.117 on task-1.117-app-a-bare-window-with-one-terminal-you at dd0dbb811b11:

```text
ok      format check (0.2s)
ok      lint (1.3s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.6s)
ok      tests (every crate) (53.2s)
ok      build (steg-sdk) (0.5s)
ok      test (steg-sdk) (1.7s)
ok      test (steg-sdk, local-socket) (2.1s)
ok      build (steg-kernel) (0.6s)
ok      test (steg-kernel) (0.9s)
ok      build (steg-server) (1.0s)
ok      test (steg-server) (21.7s)
ok      test (steg-server, race-fixture) (24.6s)
ok      test (steg-server, accept-backoff-fixture) (23.5s)
ok      build (steg) (1.6s)
ok      test (steg) (0.4s)
ok      test (steg, fixtures) (5.9s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.8s)
ok      test (steg-app) (0.9s)
ok      tool tests (11.7s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T16:43:00Z

---

Plan approved by @bruk (8551f6bb93b8, size-5): its worker may build it.

---

created: 2026-11-16T16:47:00Z

---

2026-11-16, @bruk.o: criteria narrowed to the split the engineer accepted: this task is the read-only window. TASK-1.121 has typing and resizing (and the recording); TASK-1.122 has the latency measurement and the per-frame bound. The approved plan calls them parts 2 and 3: cite TASK-1.121 and TASK-1.122 in code and notes. It no longer depends on TASK-1.114: it needs only attach.

---
<!-- COMMENTS:END -->
