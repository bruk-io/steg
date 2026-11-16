---
id: TASK-1.114
title: 'Server: input and a declared size from an attached client'
status: In Progress
assignee:
  - '@bruk.w7'
created_date: '2026-11-16T08:39:00Z'
updated_date: '2026-11-16T15:04:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.47
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.114-server-input-and-a-declared-size-from-an
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.47 by its plan (2026-11-16). doc-15 R91, doc-9, decision-2 sections 3 and 6. Covers TASK-1.47's fourth acceptance criterion. Not pane send (TASK-1.90, a one-shot request) and not pane resize (TASK-1.110, moving a layout border). The kernel's side exists (Tree::declare_size, view_pane, note_input; Runtime::apply_size_changed, from TASK-1.42 and TASK-1.76); this task calls them from the attachment's paths.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Bytes a client sends on the stream it attached with reach the pane's program, in order, and count as that client's input for the size rule (R91)
- [ ] #2 An attachment declares a size; the terminal takes the size the kernel's rule gives, and an update on every attachment's stream reports the terminal's size when it changes
- [ ] #3 Two clients attached with different sizes: the result is the kernel's rule and each is told the size in force; a client that detaches or dies releases its claim
- [ ] #4 Offered by the steg library, and the tests go through it over a real socket with a real pty (R101)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer

1. **The brief says "`pane.attach`'s own request already carries a size, read it" — it does not, and TASK-1.47's own approved plan says so explicitly.** Its "For the engineer" point 1: "`pane.attach` itself takes no size parameter: declaring one is `Tree::declare_size`, a separate kernel call TASK-1.114 wires in, not part of opening the stream." Confirmed in code: `PaneCommand::Attach { stream, reply }` (`crates/server/src/pane.rs`, branch `task-1.47-server-attach-to-a-pane-stream-its` @ `1ce276d`) carries no size field, and `Runtime::attach_pane` never reads one. This plan is built on that ground truth, not the brief's assumption.
2. **A new method is needed to declare a size, and it targets a tab, not a pane.** `Tree::declare_size(client, tab, size)` (`crates/kernel/src/tree.rs:1386`) takes a `TabId`: "a client declares a size for the tab and is told each pane's" (TASK-1.47's plan, §"Scope"), matching `Layout::leaf_sizes` splitting one declared total across a tab's panes by their split ratios. Recommend: `tab.declare_size` — noun `tab` (already in doc-2's noun table), new verb for doc-2 §3's "verbs one noun owns" table, since none of the closed general verbs (list/show/new/close/move/set/unset/focus/start/stop/check/watch/wait) fit "a client asserts its own size." Proposed row, for the lead to add to doc-2 (same shape 1.47 proposed for `pane attach`/`pane detach`, not mine to edit): `tab declare-size | tab | "Declare the caller's own size for the tab (R91, decision-6)" | Nothing`. Params: `columns`, `rows` (both required integers). `returns: Returns::Nothing`, `follows: Follows::Nothing`.
3. **This method cannot be a plain `dispatch::dispatch` arm, for a sharper reason than "no `Outbound`."** `dispatch::dispatch`'s signature (`crates/server/src/dispatch/mod.rs:153`) takes `&mut Tree, &mut Settings, &Description, &Context, &Request, Moment` — **no `ClientId` at all**. `Context` (`dispatch/mod.rs:50`) holds only `default_session`/`default_tab`/`default_pane`. Every ordinary dispatch arm (session/tab/pane/api/setting) is therefore structurally unable to call `Tree::declare_size`/`Tree::note_input`, which both require a `ClientId` — not merely "no Outbound," the exact gap TASK-1.46/1.47 hit for `event.watch`/`pane.attach`, but one step further back. `tab.declare_size` must be intercepted in `connection.rs`'s `handle_request`, exactly where `pane.attach`/`pane.detach` are today, calling a new `Runtime::declare_size` that takes `ClientId` directly. Input (AC #1) needs the same `ClientId` for `Tree::note_input` and is not a request at all (it's `Incoming::Data` on an open stream) — handled in `run_post_handshake`'s own match, one level further out.
4. **`tab.declare_size` must be added to `dispatch/mod.rs`'s test-only `INTERCEPTED_BEFORE_DISPATCH` list** (`crates/server/src/dispatch/mod.rs:106`, currently `&[("pane", "attach"), ("pane", "detach")]`), the list TASK-1.47 built for exactly this shape ("described, fully built methods that never reach `dispatch` at all... because answering them needs a connection's own `Arc<Outbound>`... or its own per-connection attachment bookkeeping"). Without this, `every_described_method_has_exactly_one_handler` fails once `tab.declare_size` is added to `description()`.
5. **The doc-2 §8 completeness test's own count moves again, and the exact number must be re-checked at build time, not carried forward from this plan.** On `1ce276d` it is `the_method_set_is_exactly_doc_2_section_8s_44_pairs` asserting `len() == 44` (TASK-1.47's own bump from 42). TASK-1.47 is `In Progress`/`changes-requested` and may move again before this task builds on it. Before writing this test's own next number, run `git -C <worktree> grep -n "the_method_set_is_exactly" crates/sdk/src/description/methods.rs` against the actual branch head you are building against, not this plan's text.
6. **A size of zero is a narrower risk than R100's floor, and worth refusing even though R100 explicitly allows small.** `Tree::declare_size`'s own doc says its size is applied "with no floor check, ever (R100's last clause)" — R100 (doc-15) deliberately lets a window shrink below 20x4 ("a pane may go under the floor that way, and its terminal is cropped"), so this plan does not add a floor the kernel's own design just rejected. But `columns: 0` or `rows: 0` is a different, degenerate case R100 never addresses, and `ghostty_vt::Terminal::resize`'s behaviour at 0 is unverified. Recommend: `Runtime::declare_size` refuses `columns == 0 || rows == 0` with `ErrorCode::BadRequest`, before the kernel or the engine ever sees it — cheap, and the one place in this task close to an untested engine edge.
7. **A resize that changes the terminal's size without marking any cell dirty must still be reported — the engine's `Dirty` and "did the size change" are not the same question, and nothing already built checks this.** See §2 below for the exact mechanism and the fix; flagged here because it is the one piece of kernel-level behaviour this task adds that is not a mechanical field addition.

## 1. Input (AC #1)

**Routing.** `run_post_handshake`'s own match (`crates/server/src/connection.rs:362`) currently buckets `Incoming::Data { .. }` with `Hello`/`Reply`/`Event`/`UnknownEvent`/`StreamControl`/`UnknownStream`/`Malformed` into one arm that closes the connection — TASK-1.47's own plan named this as a real, temporary gap it was leaving to this task ("a client that attaches and then sends input before 1.114 merges gets its connection closed on the first such frame"). This task pulls `Incoming::Data { stream, bytes }` into its own arm:

```rust
Some(Incoming::Data { stream, bytes }) => {
    if let Some(&pane) = state.stream_panes.borrow().get(&stream) {
        runtime.send_input(client_id, pane, bytes);
    }
    // else: unreachable in practice — `Connection::open_stream` is only
    // ever called together with inserting into `stream_panes` (below),
    // so a `stream` this match sees as `Incoming::Data` (meaning
    // `Connection`'s own `open_streams` already contains it) always has
    // an entry here too. Not an error either way: nothing to route to.
}
```

`Incoming::UnknownStream { .. }` stays exactly where it is, in the closing bucket, unchanged — a frame for a stream this connection never opened (or already closed with `pane.detach`/disconnected) is still refused by closing the connection, which is the existing, already-decided behaviour (TASK-1.45 §3: "there is no id to answer most of them with, so closing is the only coherent, uniform response"), not something this task invents. This is the "refused or dropped" decision-2 describes for a frame outside an attachment's own stream.

**The reverse lookup.** `ConnectionState`/`ConnectionCleanup` (`connection.rs:262`, `:398`) currently hold `pane_streams: RefCell<HashMap<PaneId, u32>>` — the direction `pane.detach` needs (pane → stream). Input needs the other direction (stream → pane). Add a second field, `stream_panes: RefCell<HashMap<u32, PaneId>>`, to both `ConnectionState` and `ConnectionCleanup`, inserted and removed in the same three places `pane_streams` already is (`pane.attach`'s success arm, `pane.detach`'s `Some(stream)` arm, and nowhere else needs to touch it — `ConnectionCleanup::drop`'s sweep only reads `pane_streams`, unchanged, since it only needs pane→stream to call `forget_attachment`). Two `HashMap`s kept in step by construction (one insert/remove call site touches both) rather than one bK: `bimap`-shaped type, which would be the only new dependency this task could otherwise need — not worth it for two call sites.

**`Runtime::send_input`**, new, `pub(crate)`, same self-locking shape as `Runtime::arm_attachment`:

```rust
pub(crate) fn send_input(&self, client: ClientId, pane: PaneId, bytes: Vec<u8>) {
    let mut state = self.lock();
    if let Ok(Some(change)) = state.tree.note_input(client, pane) {
        apply_size_changed_locked(&mut state, &change);
    }
    // `Err` (ClientNotFound/ClientNotViewingPane) is never reached in
    // practice: `stream_panes` only ever names a pane this connection's
    // own `client_id` is currently viewing (inserted at the same point
    // `pane.attach` succeeded, removed at `pane.detach`/cleanup) — but if
    // it ever were, best-effort, the same tolerance `detach_pane`/
    // `forget_attachment` already give a stale pane: there is nothing
    // sane to do about a state inconsistency here except not crash the
    // connection over it.
    if let Some(handle) = state.panes.get(&pane) {
        let _ = handle.send_input(bytes);
    }
}
```

`Tree::note_input` (`crates/kernel/src/tree.rs:1442`) is R91's other half of "latest" ("the last one to send input to it or be resized") and already clears the pane's `Finished`/`Failed` status (doc-11 R83) and reapplies whatever size the now-latest client last declared, with no clock read anywhere — `Tree::tick()` is the kernel's own counter (`crates/kernel/src/tree.rs:154`), not wall time, which is what lets AC #1's "count as that client's input... without the kernel reading a clock" hold with no new code.

**Why the reader never blocks on a slow program.** `Runtime::send_input` only ever (a) takes the `Mutex` guarding `State` — the same lock every other request already takes, held for a kernel call plus two `HashMap` lookups, no I/O; and (b) calls `PaneHandle::send_input`, which is `self.commands.send(PaneCommand::Input(bytes))` on an **unbounded** `mpsc::Sender` (`crates/server/src/pane.rs:448`) — a non-blocking enqueue, never a write to the pty. The pty write itself happens later, inside the pane's own worker thread, in the `PaneCommand::Input` arm (`pane.rs:837-841`): `running.writer.write_all(&bytes)`. A pty nobody is reading can block that call for real, but it blocks only that one pane's own worker thread — a different OS thread per pane (module doc: "one OS thread per pane... created once and alive until shutdown") — never the connection's reader thread (which already returned once the mpsc `send` succeeded), never the connection's writer thread or its other streams (`Outbound`'s queues are untouched by this path), and never another pane's worker (each owns its own channel and its own thread). The one real cost is pane-local: more `PaneCommand::Input` piling up behind a stuck `write_all` on that pane's own queue, the identical, already-accepted shape `PtyBytes` already has on the same channel (`pane.rs`'s own module doc: "a flooding run's own backlog of `PtyBytes` on the worker's command queue"; `:430`: "a flooding run's own backlog of `PtyBytes` on that same queue"). This task adds no new bounding because none of this is new: it is the same channel, the same thread-per-pane isolation, TASK-1.40/1.47 already built and documented.

**Ordering.** `connection.rs`'s read loop is the sole sender of this connection's own `PaneCommand::Input` values for a given pane (one reader thread per connection), and `mpsc` guarantees per-sender FIFO delivery — the identical argument `pane.rs`'s own `drain_command_marks`/`spawn_relay` comments already make for `PtyBytes`/`Exited` ordering, cited rather than re-derived. Bytes therefore reach the program in the order the client sent them, with nothing between the connection's `poll()` loop and the worker's `write_all` that could reorder two `Input` frames from the same stream.

## 2. Declared size (AC #2)

**The method.** `tab.declare_size`, added to `crates/sdk/src/description/methods.rs`'s `tab_methods()`, params `[target("The tab to declare a size for. Defaults to the current tab"), flag("columns", ValueKind::Integer, true, "The declared width, in whole cells"), flag("rows", ValueKind::Integer, true, "The declared height, in whole cells")]`, `returns: Returns::Nothing`, `follows: Follows::Nothing`, `errors: [BadRequest, NoSuchTarget, AmbiguousTarget]`.

**`connection.rs`'s `handle_request`** gains an arm reading `columns`/`rows` with the existing `dispatch::param_i64` helper (`dispatch/lookup.rs:25`) and range-checking into `u16`, same shape as `pane.detach`'s arm (builds and enqueues its own reply here, not inside `Runtime`, since there is no ordering hazard to protect — no stream is opened by this call):

```rust
"tab.declare_size" => {
    let response = match (param_i64(&resolved, "columns"), param_i64(&resolved, "rows")) {
        (Some(columns), Some(rows)) if (1..=i64::from(u16::MAX)).contains(&columns)
            && (1..=i64::from(u16::MAX)).contains(&rows) => {
            match runtime.declare_size(description, context, &resolved, client_id,
                                        columns as u16, rows as u16) {
                Ok(()) => Response::Ok(serde_json::Value::Null),
                Err(error) => Response::Error(error),
            }
        }
        _ => Response::Error(bad_request("columns and rows must each be between 1 and 65535")),
    };
    let bytes = Connection::reply(id, response);
    outbound.enqueue_control(bytes).is_ok()
}
```

**`Runtime::declare_size`**, new, `pub(crate)`, self-locking:

```rust
pub(crate) fn declare_size(
    &self, description: &Description, context: &Context, request: &Request,
    client: ClientId, columns: u16, rows: u16,
) -> Result<(), ApiError> {
    validate(description, request)?;
    let mut state = self.lock();
    let tab = dispatch::target_tab(&state.tree, context, request)?;
    let change = state.tree.declare_size(client, tab, Size::new(columns, rows))
        .map_err(dispatch::map_kernel_error)?;
    apply_size_changed_locked(&mut state, &change);
    Ok(())
}
```

`dispatch::target_tab` (`dispatch/lookup.rs:70`) is already `pub(crate)` — no widening needed, unlike `target_pane` in TASK-1.47. `apply_size_changed_locked` (`runtime.rs:1252`) already resizes every named pane's `PaneHandle` (terminal + pty together) and, via each pane's own `PaneCommand::Resize` arm, already arms that pane's own tick (`pane.rs:842-847`) — this is the entire mechanism that gets every attachment told; **`SizeChanged.viewers` is deliberately not read anywhere in this plan.** It exists so a caller could push to a per-client channel, but there is no `ClientId -> Outbound` map anywhere in `Runtime` (connections are anonymous past the kernel's own `ClientId`) and none is needed: every pane `SizeChanged.panes` names already resizes and re-arms through the pane's own worker, which already knows its own fan-out list of attachments — the tick, not a client address, is what tells them.

**Carrying the size on the wire — the gap against decision-2 this task closes, not a new shape.** `Screen` (`crates/sdk/src/screen.rs:146`) is built as `{ rows, cursor }` today; decision-2 §3 already specifies "Snapshot, as plain data... sequence number, size, the screen as rows... the cursor" — `size` is a decision-2 field 1.47 left out, not an invention. Add `pub size: Size` to `Screen`. Every struct literal that builds one needs updating — found by grep, not estimate: `crates/kernel/src/terminal.rs:211` (`screen()`) and `:280` (`changes_since`'s `Full` branch) are the only two in production code; `crates/sdk/src/screen.rs`'s own test module (`sample_screen()`) is the only other.

`terminal.rs:211`, `screen()`: `Ok(Screen { rows, cursor, size: Size::new(self.engine.cols().map_err(Error::Engine)?, self.engine.rows().map_err(Error::Engine)?) })` — a fresh read each call, correct for a point-in-time snapshot (`pane.attach`, `pane.capture`).

**`ScreenUpdate::Delta` gains `size: Option<Size>`** (doc-14: "an update gains an optional `size` for the same purpose") — populated by `Terminal::changes_since`, not by `pane.rs`'s `run_tick`, because only the kernel's `Terminal` has the one `seq`-like bookkeeping point to compare against. Add `last_reported_size: Size` to `Terminal` (init in `new()` from its own `cols`/`rows` args), compared in `changes_since` against a fresh `Size::new(self.engine.cols()?, self.engine.rows()?)` read on every call.

**The risk this closes, not a mechanical field add:** `Dirty` and "did the size change" are two different questions the engine answers independently, and nothing says they agree — a resize that only grows blank rows at the bottom, or only adds columns of default-styled cells, is exactly the kind of change the engine's own content-dirty tracking might report as `Dirty::Clean` (no cell's *content* changed) while the dimensions plainly did. Today's `changes_since` would answer `Update::Unchanged` in that case (`is_current && Dirty::Clean => None`) and the size change would never reach a single attachment — silently, with `run_tick`'s own `ScreenUpdate::Unchanged => return` (`pane.rs:994`) throwing it away before anything is sent. Fix: check the size first, independent of `Dirty`:

```rust
let current_size = Size::new(self.engine.cols().map_err(Error::Engine)?, self.engine.rows().map_err(Error::Engine)?);
let size_changed = current_size != self.last_reported_size;

let outcome = if is_current {
    match dirty {
        Dirty::Clean if !size_changed => None,
        Dirty::Clean => Some(Outcome::Delta(Vec::new())), // size-only: no rows changed
        Dirty::Partial => Some(Outcome::Delta(dirty_rows(&snapshot, &palette)?)),
        Dirty::Full => Some(Outcome::Full(all_rows(&snapshot, &palette)?)),
    }
} else {
    Some(Outcome::Full(all_rows(&snapshot, &palette)?))
};
// ... snapshot.clean()?; self.last_reported_size = current_size; ...
Ok(match outcome {
    None => Update::Unchanged,
    Some(Outcome::Delta(rows)) => { self.seq += 1; Update::Delta { rows, cursor, seq: self.seq, size: size_changed.then_some(current_size) } }
    Some(Outcome::Full(rows)) => { self.seq += 1; Update::Full { screen: Screen { rows, cursor, size: current_size }, seq: self.seq } }
})
```

A size-only change now reports as a `Delta` with empty `rows`, an advanced `seq`, and `size: Some(new_size)` — never silently swallowed as `Unchanged`, and never promoted to a full, scrollback-free resync it does not need (that stays reserved for `!is_current`/back-pressure, decision-2 §6). `run_tick`'s own "ready, shared payload" optimisation (`pane.rs:1004-1018`, one JSON blob built once and cloned for every caught-up attachment) is untouched: `size_changed` is computed once per tick, by the kernel, not per attachment, which is exactly why this is a pane-level, not an attachment-level, flag.

## 3. Two clients, different sizes, and release (AC #3)

**The kernel's rule, as built, unchanged by this task.** `Tree::declare_size` makes the declaring client latest unconditionally ("its tick is now the freshest of anyone's"); `Tree::note_input` makes the sender latest and reapplies *that client's own last declared size* for the tab if it differs from the tab's current size — "the path that makes an *old* declaration win again once its client becomes latest by a later input, not a fresh declare" (`tree.rs:1419`). This is decision-6/R91 exactly: "the latest client wins... meaning the last one to send input or be resized," already proven by the kernel's own tests (`latest_client_tracks_whoever_last_declared_or_sent_input`, `note_input_reapplies_the_now_latest_clients_last_declared_size`, `tree.rs:2973`/`:3286`). This task's §§1-2 are the only wiring it needed; no kernel change here.

**Release, already built by TASK-1.47, not by this task.** `Tree::stop_viewing` and `Tree::disconnect_client` both call `Tree::reapply_latest` and return `Vec<SizeChanged>`, already threaded into `Runtime::detach_pane` and `Runtime::disconnect_client` (`runtime.rs:539`, `:414`), which already call `apply_size_changed_locked` for each. A client that calls `pane.detach`, or whose connection ends at all (`ConnectionCleanup::drop` → `runtime.disconnect_client`, `connection.rs:289`), already has its claim released and the remaining client's own declared size reapplied — this task adds a proof that exercises this with an actual declared-size difference (not merely that a stale attachment is swept), not new server code for the release path itself.

## 4. The sdk's surface (R101)

**Input.** `Connection::send_data(&self, stream: u32, bytes: &[u8]) -> Result<Vec<u8>, FrameError>` (`crates/sdk/src/wire/connection.rs:409`) already exists, already refuses a `stream` this connection has not itself opened. Nothing new to add: TASK-1.117 calls `connection.send_data(stream, &key_bytes)` with the `stream` it read off `PaneAttached` and opened itself (see below) — this task's job is only to confirm this is what it calls, not to build it.

**Declaring a size.** Add `Connection::declare_size(&mut self, tab: Option<&str>, columns: u16, rows: u16) -> Result<(u64, Vec<u8>), ApiError>`, the same shape as `Connection::attach`/`Connection::detach` (`wire/connection.rs:302`/`:321`): builds a `params` map with `target`/`columns`/`rows` and calls `self.call("tab.declare_size", params)`.

**Reading an update.** No new sdk type or helper: `ScreenUpdate` already derives `Deserialize` (`crates/sdk/src/screen.rs`), so `serde_json::from_slice::<steg_sdk::ScreenUpdate>(&payload)` off an `Incoming::StreamControl { stream, payload }` is itself "offered by the sdk" in R101's sense (a public, typed shape; no protocol *logic* needed to decode it — the demuxing is already `Connection::poll`'s job). Recommend no wrapper method; flagging this as "For the engineer" only in case a bare `serde_json::from_slice` call in app code reads as "the app holds protocol code" to someone — it does not, by the same reasoning `Connection::call`'s own `Response::Ok` value is already read this way by every existing caller.

**What TASK-1.117 calls, named once, for its own plan to cite:** `Connection::attach` → read `stream`/`snapshot` off the reply → `Connection::open_stream(stream)` (the client's own half of decision-2 §3's ordering rule, confirmed by `wire/connection.rs:284`'s own doc: "the caller reads its own `stream` field and calls `Connection::open_stream` with it *after* that reply is in hand, never before") → `Connection::declare_size` once, with the window's own initial size → on every keystroke, `Connection::send_data(stream, bytes)` → on every `Incoming::StreamControl` on that stream, `serde_json::from_slice::<ScreenUpdate>(&payload)` → on resize, `Connection::declare_size` again.

## 5. Proof

Over a real socket (`crates/server/tests/`, the shape TASK-1.47's own `attach.rs` already establishes) with the sdk's own `Connection` as the client and a real pty — no mock terminal, no fake transport. Every item below gets a named control, recorded through `asdlc record-check` as a failing run (the mechanism reverted) and then a passing run, against the tests as they stand at the end — not a hand-run-and-reverted demonstration (TASK-1.47's own review was sent back for exactly this; this task is not repeating it).

| # | Proof | Control (mechanism removed) |
|---|---|---|
| 1 | A program that echoes its stdin (or records it, with a bounded wait on its own output) receives typed bytes in order, over a real pty | Remove the `Incoming::Data` arm (leave it in the closing bucket): the test fails because the connection closes on the first input frame |
| 2 | `note_input` makes the sender latest and clears a `Finished`/`Failed` status | Remove the `tree.note_input` call from `Runtime::send_input`: the two-client latest-wins test fails |
| 3 | A declared size reaches the real pty (a program reading `stty size`, or one that traps `SIGWINCH` and reports the window-size ioctl) and is reported on **every** attachment's own stream, including one that declared no size itself | Drop `size` from `ScreenUpdate::Delta`/its population in `changes_since`: the "every attachment is told" test fails |
| 4 | A size-only change (same content, new dimensions) still reaches an attachment that is otherwise caught up | Revert the `Dirty::Clean`-but-`size_changed` branch to `None`: this test specifically fails (distinct from #3, which a naïve implementation that only handles `Dirty::Full`/`Partial` could still pass) |
| 5 | Two clients, different declared sizes: the result is the kernel's rule (latest wins), and each client's own attachment is told the size in force | Covered by #3's own mechanism; this item's control is the kernel one already named in §3 above (`tree.rs`'s own tests), cited not re-proved, plus this task's own server-level assertion that both streams receive the post-reapply size |
| 6 | A client that calls `pane.detach` releases its claim; the survivor's terminal returns to *its own* declared size, and the survivor is told **on its own stream**, not merely that the kernel recomputed | Remove `forget_attachment`/the `detach_pane` call: the test fails to observe the survivor's stream reporting the reapplied size |
| 7 | A client whose connection is dropped hard (a real killed client process, or the socket half closed from under it — not a polite `pane.detach`) releases its claim the same way | Remove `ConnectionCleanup::drop`'s `disconnect_client`/`forget_attachment` calls: the test fails the same way as #6 |
| 8 | A program that never reads its stdin (or is `SIGSTOP`ped) does not stall: flood input to its pane, then prove a control call on the *same* connection still answers, and a second, unrelated pane's own attachment still receives updates, both on bounded waits on the event | No mechanism to remove here (nothing new bounds this — see §1's own reasoning); the control is a negative one: assert the second pane's update arrives within one tick's bound while the first pane's own `PaneCommand::Input` queue is still backed up, proving thread isolation rather than a limit this task added |
| 9 | A frame on a stream this connection never opened, or already detached, closes the connection (decision-2's "refused") | Already covered by TASK-1.47's own `Incoming::UnknownStream` behaviour — this task adds one test sending `Incoming::Data` *after* `pane.detach`, confirming the existing mechanism still applies once this task's new `Incoming::Data` arm exists beside it |
| 10 | A declared size of `0` is refused with `BadRequest`, never reaching `Tree::declare_size` or the engine | Remove the range check in connection.rs's `tab.declare_size` arm: the test fails |

Every item above gets a test or is named as dropped; none is.

## 6. Building against unmerged work

TASK-1.46 and TASK-1.47 are both `In Progress` (1.47 `changes-requested`); this task's three touched server files (`pane.rs`, `connection.rs`, `runtime.rs`) are the same three 1.47's own review fixes will land in. Build this task's worktree against 1.47's branch head at the time work starts (not `1ce276d` specifically — re-check), merged into this task's own branch; re-merge 1.47's branch (and main, once 1.47 lands there) before finishing, expecting conflicts in those three files and in `crates/sdk/src/description/methods.rs`/`crates/server/src/dispatch/mod.rs` if 1.47's own review moves the method count again. Merge order: TASK-1.46 → TASK-1.47 → this task, matching the dependency graph and the order the lead already named.

This stays one task: input, the declared size, and the size carried on every update are one coherent change — what an attached client sends, and what it is told back — and AC #2's own proof is unprovable without AC #1's routing already in place (the sdk's own `send_input`/`open_stream` sequencing is identical for both). No split proposed.
<!-- SECTION:PLAN:END -->
