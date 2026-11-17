---
id: TASK-1.114
title: 'Server: input and a declared size from an attached client'
status: In Progress
assignee:
  - '@bruk.w7'
created_date: '2026-11-16T08:39:00Z'
updated_date: '2026-11-16T18:04:00Z'
labels:
  - size-3
milestone: m-3
dependencies:
  - TASK-1.47
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.114-server-input-and-a-declared-size-from-an
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'ec385ef4d563'
asdlc_plan_approved_size: size-3
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
8. **A real, named limit this task accepts rather than fixes: one pane's screen freezes while its own program refuses to read its input, until that program reads, ends on its own, or is force-closed.** §1 below traces this precisely — every `PaneCommand` for that pane (not only more input, but the relay thread's own `PtyBytes`, and any `Attach`/`Detach`/`Resize` for that same pane) queues up behind the one stuck `write_all`, so that pane's own screen stops updating and its own attach/detach/resize all wait, for as long as the program refuses to read. §5's new proof item below confirms this never violates a stated bound elsewhere (the connection stays responsive, other panes keep updating, and a forced close or a full server stop both still return within their own documented bound) — the limit is scoped to this one pane's own responsiveness, not to anything this task's acceptance criteria promise. Accepted here, not fixed, because fixing it properly is a change to how every pane's worker handles input, not only how this task's new callers reach it: giving each pane's own pty write a thread of its own (so `PaneCommand::Input`'s handling in `worker_loop` hands bytes to a second, per-run thread that blocks on `write_all` alone, leaving the worker free to keep ticking and keep serving `Attach`/`Resize`/`Snapshot`), or making the write non-blocking with its own small bounded buffer and the same drop-and-signal shape decision-2 §6 already uses for a slow *reader*. My own estimate: small on its own (a few hours against this one function, `PaneCommand::Input`'s arm and `Running`'s own fields), but it touches `pane.rs`'s own concurrency shape precisely enough that it deserves its own plan and its own review, not a late addition to this one — proposed as a follow-up task, for the engineer to size and place.

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

**Why the reader never blocks on a slow program.** `Runtime::send_input` only ever (a) takes the `Mutex` guarding `State` — the same lock every other request already takes, held for a kernel call plus two `HashMap` lookups, no I/O; and (b) calls `PaneHandle::send_input`, which is `self.commands.send(PaneCommand::Input(bytes))` on an **unbounded** `mpsc::Sender` (`crates/server/src/pane.rs:448`) — a non-blocking enqueue, never a write to the pty. The pty write itself happens later, inside the pane's own worker thread, in the `PaneCommand::Input` arm (`pane.rs:837-841`): `running.writer.write_all(&bytes)`. A pty nobody is reading can block that call for real, but it blocks only that one pane's own worker thread — a different OS thread per pane (module doc: "one OS thread per pane... created once and alive until shutdown") — never the connection's reader thread (which already returned once the mpsc `send` succeeded), never the connection's writer thread or its other streams (`Outbound`'s queues are untouched by this path), and never another pane's worker (each owns its own channel and its own thread).

**Say what that costs this one pane, not only that it is contained.** Every `PaneCommand` for this pane — not only more `Input`, but `PtyBytes` (the relay thread keeps reading the program's own output and keeps enqueuing it; nothing stops that), `Attach`, `Arm`, `Detach`, `Resize`, `Snapshot`, `Cwd` — goes through this same one `mpsc::Sender` (`worker_loop`'s single `commands_rx.recv()`/`recv_timeout()` match, `pane.rs:816-834`), so all of them queue up behind the stuck `write_all` too. Concretely, while this pane's worker is blocked: its screen stops updating (the bytes it already received sit in the queue, unread by `terminal.write`, so no tick ever sees them as dirty); a client trying to attach, detach or declare a size *for this pane* waits behind the same backlog; and the queue itself grows without any bound of its own for as long as this lasts. This is the identical, already-accepted shape `PtyBytes` alone already had on this channel before this task (`pane.rs`'s own module doc: "a flooding run's own backlog of `PtyBytes` on the worker's command queue"; `:430`: "a flooding run's own backlog of `PtyBytes` on that same queue") — this task does not add a new kind of jam, it adds a second way to cause the one that was already possible.

**What ends it.** Either the program reads (the ordinary case: a pty's own kernel input buffer is finite, but a program that reads its stdin at all drains it, and `write_all` returns), or the program ends — naturally, or forced. Forcing bypasses this pane's own stuck queue entirely: `PaneHandle::signal_hangup`/`signal_kill` (`pane.rs:639-647`) go straight to the OS through `RunGate`, "never through `commands`... so a backlog of `PtyBytes` the worker has not drained yet... cannot delay this" (the method's own doc, already written for exactly this reason by TASK-1.78) — the stuck worker thread is never on the path a forced signal takes. Read `Runtime::force_end` (`runtime.rs:1160-1248`, used by both `Runtime::stop` and `pane.close --force`'s own wrapper) pass by pass against this: pass 1 (`signal_hangup`) and pass 3 (`signal_kill`, if pass 2 times out) are `RunGate`-only, unaffected by the stuck worker; pass 2/4 (`await_exit`, waiting for the process to actually die) are also `RunGate`-only (a `Condvar`, not `commands`) — the relay thread, a *separate* thread from the stuck worker, is the one that reaps the child and clears the gate, and reading the pty's output is not blocked by the worker being stuck writing to it, so this still completes once the process is actually dead; only pass 5 (`settle_worker`, telling the worker its run ended) and pass 6 (`join_worker`) go back through `commands`/the worker thread itself, and both are independently bounded (`FORCE_JOIN_TIMEOUT`, run via `thread::scope` in parallel across every pane in `handles`) — so **`force_end`, and therefore both `Runtime::stop` and a forced `pane.close`, return within their already-documented bound (`grace + ESCALATE_CONFIRM_TIMEOUT + FORCE_JOIN_TIMEOUT`) regardless of whether this pane's worker ever actually unblocks** — confirmed by reading the code, not assumed: nothing in any of `force_end`'s six passes waits on the stuck worker thread past a stated timeout.

What is expected to make `write_all` itself return, once the signaled process (and, per TASK-1.78, its whole process group) is dead: every file descriptor onto the pty's slave side closes with it, and a pending write to the master side of a pty with no open reader on the slave returns an error (`EIO`) rather than blocking forever — the write-side half of the identical kernel guarantee this codebase already relies on for the *read* side (`crates/server/src/pty.rs:140-141`: "dropping the slave is what lets the program's own exit reach the reader as end-of-file... the pty's slave side only closes once [the program and everything it started are gone]"). This is inferred by symmetry with an already-relied-upon guarantee, not independently verified elsewhere in this codebase — §5's new proof item below is what actually checks it, empirically, rather than leaving it assumed.

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
| 9 | **A program that never reads its stdin at all** (e.g. `sleep`, which never touches it), flooded with more bytes than the pty's own kernel input buffer holds, so this pane's own worker is genuinely, verifiably stuck in `write_all`: prove, all on bounded waits, that (a) another request on the *same* connection still answers, (b) a second pane on the same connection still receives updates, (c) `pane.close --force` on the *stuck* pane returns within `force_grace + ESCALATE_CONFIRM_TIMEOUT + FORCE_JOIN_TIMEOUT`, and (d) stopping the server with this pane still stuck returns within its own documented bound too | If (a)/(b) fail: the stuck worker is somehow blocking the connection or another pane — a real regression, not named as a limit anywhere in this plan, and must be fixed before this task is done, not merely written down. If (c)/(d) time out: `force_end` is waiting on the stuck worker somewhere past a stated bound — contrary to this plan's own reading of its six passes (§1 above) — also a bug to fix here, not a limit to accept. Only the pane's own screen freezing while stuck (named in "For the engineer" point 8) is the accepted limit; this test's job is to confirm nothing *else* rides along with it |
| 10 | A frame on a stream this connection never opened, or already detached, closes the connection (decision-2's "refused") | Already covered by TASK-1.47's own `Incoming::UnknownStream` behaviour — this task adds one test sending `Incoming::Data` *after* `pane.detach`, confirming the existing mechanism still applies once this task's new `Incoming::Data` arm exists beside it |
| 11 | A declared size of `0` is refused with `BadRequest`, never reaching `Tree::declare_size` or the engine | Remove the range check in connection.rs's `tab.declare_size` arm: the test fails |

Every item above gets a test or is named as dropped; none is.

## 6. Building against unmerged work

TASK-1.46 and TASK-1.47 are both `In Progress` (1.47 `changes-requested`); this task's three touched server files (`pane.rs`, `connection.rs`, `runtime.rs`) are the same three 1.47's own review fixes will land in. Build this task's worktree against 1.47's branch head at the time work starts (not `1ce276d` specifically — re-check), merged into this task's own branch; re-merge 1.47's branch (and main, once 1.47 lands there) before finishing, expecting conflicts in those three files and in `crates/sdk/src/description/methods.rs`/`crates/server/src/dispatch/mod.rs` if 1.47's own review moves the method count again. Merge order: TASK-1.46 → TASK-1.47 → this task, matching the dependency graph and the order the lead already named.

This stays one task: input, the declared size, and the size carried on every update are one coherent change — what an attached client sends, and what it is told back — and AC #2's own proof is unprovable without AC #1's routing already in place (the sdk's own `send_input`/`open_stream` sequencing is identical for both). No split proposed.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan (ec385ef4d563) checked against the four acceptance criteria, decision-2 section 3, doc-14 and doc-15 R91, doc-16 R101 and the team's testing rules by @bruk.o, 2026-11-16: ok. Built on TASK-1.47's branch (not merged) and to merge after it. Revised once at the orchestrator's request: the full cost of a program that does not read its input is stated (that pane's worker is blocked, so its screen stops updating and its commands wait), with a proof item on a real program that never reads (the connection stays responsive, another pane keeps updating, a forced close and a server stop return within their bounds), and that a failure there is a bug to fix in this task. For the engineer: the new method tab.declare_size (a row in doc-2; the method count moves to 45); a size with zero columns or rows is refused; and the accepted limit that one pane's own screen freezes while its program refuses input, with a follow-up proposed.

Check passed: `just check` exited 0 at 2026-11-16T16:49:37+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c:

```text
ok      format check (0.3s)
ok      lint (86.9s)
ok      boundaries (0.2s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.2s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (62.2s)
ok      build (steg-sdk) (3.5s)
ok      test (steg-sdk) (2.3s)
ok      test (steg-sdk, local-socket) (2.6s)
ok      build (steg-kernel) (43.2s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (46.6s)
ok      test (steg-server) (14.7s)
ok      test (steg-server, race-fixture) (14.4s)
ok      build (steg) (2.4s)
ok      test (steg) (0.4s)
ok      test (steg, fixtures) (4.2s)
ok      build (steg-ui) (14.5s)
ok      test (steg-ui) (1.4s)
ok      build (steg-app) (1.9s)
ok      test (steg-app) (0.8s)
ok      tool tests (12.7s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `sh -c 'cargo test -p steg-sdk -p steg-kernel -p steg-server'` exited 0 at 2026-11-16T17:39:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes, last 40 of 960 lines:

```text
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.06s

     Running tests/shutdown.rs (target/debug/deps/shutdown-641947b6111d1ffd)

running 8 tests
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-e7b594c955bacef5)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_kernel

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_sdk

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check passed: `sh -c 'cargo test -p steg-sdk -p steg-kernel -p steg-server'` exited 0 at 2026-11-16T17:48:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes, last 40 of 948 lines:

```text
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.06s

     Running tests/shutdown.rs (target/debug/deps/shutdown-641947b6111d1ffd)

running 8 tests
test a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop ... ok
test stopping_closes_the_listener_removes_the_socket_and_releases_the_lock ... ok
test stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound ... ok
test stop_called_twice_concurrently_on_an_idle_server_is_harmless ... ok
test a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop ... ok
test many_short_connections_do_not_grow_the_tracked_set ... ok
test stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound ... ok
test stop_racing_a_client_connecting_still_returns_within_a_bound ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/socket_mode.rs (target/debug/deps/socket_mode-e7b594c955bacef5)

running 1 test
test the_socket_is_0600_even_under_a_permissive_umask ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_kernel

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_sdk

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests steg_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-server --test attach typed_bytes_reach_a_real_program_in_order` exited 101 at 2026-11-16T17:52:55+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
warning: method `send_input` is never used
   --> crates/server/src/runtime.rs:590:19
    |
244 | impl Runtime {
    | ------------ method in this implementation
...
590 |     pub(crate) fn send_input(&self, client: ClientId, pane: PaneId, bytes: Vec<u8>) {
    |                   ^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test typed_bytes_reach_a_real_program_in_order ... FAILED

failures:

---- typed_bytes_reach_a_real_program_in_order stdout ----

thread 'typed_bytes_reach_a_real_program_in_order' (44720435) panicked at crates/server/tests/attach.rs:445:30:
the server closed before an update arrived
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    typed_bytes_reach_a_real_program_in_order

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.02s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check passed: `cargo test -p steg-server --test attach typed_bytes_reach_a_real_program_in_order` exited 0 at 2026-11-16T17:53:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.75s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test typed_bytes_reach_a_real_program_in_order ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.04s
```

Check FAILED: `cargo test -p steg-server --test attach input_clears_a_finished_status` exited 101 at 2026-11-16T17:53:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
warning: variable does not need to be mutable
   --> crates/server/src/runtime.rs:591:13
    |
591 |         let mut state = self.lock();
    |             ----^^^^^
    |             |
    |             help: remove this `mut`
    |
    = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default

warning: `steg-server` (lib) generated 1 warning (run `cargo fix --lib -p steg-server` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.75s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test input_clears_a_finished_status ... FAILED

failures:

---- input_clears_a_finished_status stdout ----

thread 'input_clears_a_finished_status' (44725105) panicked at crates/server/tests/attach.rs:794:9:
input never cleared the Finished status: Object {"cwd": String("/var/folders/rf/f34xmczn4659gm98hrsprydc0000gn/T/"), "exit_code": Number(0), "id": String("00000003"), "kind": String("terminal"), "label": Null, "program": String("sh -c exit 0"), "program_status": String("ended"), "size": Null, "status": String("finished"), "title": Null}
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    input_clears_a_finished_status

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 5.06s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check passed: `cargo test -p steg-server --test attach input_clears_a_finished_status` exited 0 at 2026-11-16T17:53:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.74s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test input_clears_a_finished_status ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.04s
```

Check FAILED: `cargo test -p steg-server --test attach a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream` exited 101 at 2026-11-16T17:54:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.69s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... FAILED

failures:

---- a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream stdout ----

thread 'a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream' (44729633) panicked at crates/server/tests/attach.rs:436:17:
no update arrived on stream 1 within 5s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 5.05s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check passed: `cargo test -p steg-server --test attach a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream` exited 0 at 2026-11-16T17:54:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.69s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.06s
```

Check FAILED: `cargo test -p steg-server --lib a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report` exited 101 at 2026-11-16T17:54:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::attach_tests::a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report ... FAILED

failures:

---- pane::attach_tests::a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report stdout ----

thread 'pane::attach_tests::a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report' (44733748) panicked at crates/server/src/pane.rs:1997:25:
no update arrived within 2s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane::attach_tests::a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 157 filtered out; finished in 2.01s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report` exited 0 at 2026-11-16T17:54:38+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.63s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::attach_tests::a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 157 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-server --test attach detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size` exited 101 at 2026-11-16T17:54:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.75s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... FAILED

failures:

---- detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size stdout ----

thread 'detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size' (44737976) panicked at crates/server/tests/attach.rs:436:17:
no update arrived on stream 1 within 5s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 5.06s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check passed: `cargo test -p steg-server --test attach detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size` exited 0 at 2026-11-16T17:55:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.05s
```

Check FAILED: `cargo test -p steg-server --test attach a_dropped_connection_releases_its_claim_the_same_way` exited 101 at 2026-11-16T17:55:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes, last 40 of 48 lines:

```text
    |     ^^^^^^^
267 |     client_id: ClientId,
    |     ^^^^^^^^^
...
280 |     pane_streams: &'a RefCell<HashMap<PaneId, (crate::pane::AttachmentId, u32)>>,
    |     ^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: method `forget_attachment` is never used
   --> crates/server/src/runtime.rs:566:19
    |
244 | impl Runtime {
    | ------------ method in this implementation
...
566 |     pub(crate) fn forget_attachment(&self, pane: PaneId, attachment_id: pane::AttachmentId) {
    |                   ^^^^^^^^^^^^^^^^^

warning: `steg-server` (lib) generated 2 warnings
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.73s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_dropped_connection_releases_its_claim_the_same_way ... FAILED

failures:

---- a_dropped_connection_releases_its_claim_the_same_way stdout ----

thread 'a_dropped_connection_releases_its_claim_the_same_way' (44742468) panicked at crates/server/tests/attach.rs:436:17:
no update arrived on stream 1 within 5s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_dropped_connection_releases_its_claim_the_same_way

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 5.06s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check passed: `cargo test -p steg-server --test attach a_dropped_connection_releases_its_claim_the_same_way` exited 0 at 2026-11-16T17:55:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.73s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_dropped_connection_releases_its_claim_the_same_way ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.05s
```

Check passed: `cargo test -p steg-server --test attach declaring_a_size_of_zero_is_refused` exited 0 at 2026-11-16T17:55:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.70s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test declaring_a_size_of_zero_is_refused ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.02s
```

Check FAILED: `cargo test -p steg-server --test attach declaring_a_size_of_zero_is_refused` exited 101 at 2026-11-16T17:57:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test declaring_a_size_of_zero_is_refused ... FAILED

failures:

---- declaring_a_size_of_zero_is_refused stdout ----

thread 'declaring_a_size_of_zero_is_refused' (44751947) panicked at crates/server/tests/attach.rs:1066:32:
declaring 0x24 must not succeed
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    declaring_a_size_of_zero_is_refused

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.01s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check passed: `cargo test -p steg-server --test attach declaring_a_size_of_zero_is_refused` exited 0 at 2026-11-16T17:57:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.69s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test declaring_a_size_of_zero_is_refused ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.02s
```

Check FAILED: `just check` exited 1 at 2026-11-16T17:58:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes, last 40 of 64 lines:

```text
(B[m[32m+        &mut connection,
(B[m[32m+        vec!["sh", "-c", "exec sleep 100"],
(B[m[32m+    );
(B[m     let attached = attach(&mut stream, &mut connection, &pane);
     flood_stdin(&mut stream, &connection, attached.stream);
 
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server/tests/attach.rs:1238:
     // reset to a bound comfortably past `force_end`'s own worst case
     // before a call whose reply can legitimately take that long, so
     // `call_and_read_reply`'s own blocking read does not time out first.
[31m-    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
(B[m[32m+    stream
(B[m[32m+        .set_read_timeout(Some(Duration::from_secs(10)))
(B[m[32m+        .unwrap();
(B[m 
     let mut params = Map::new();
     params.insert("target".to_string(), json!(pane));
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server/tests/attach.rs:1263:
 /// stop`).
 #[test]
 fn stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck() {
[31m-    let _flood_guard = FLOOD_TEST_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
(B[m[32m+    let _flood_guard = FLOOD_TEST_LOCK
(B[m[32m+        .lock()
(B[m[32m+        .unwrap_or_else(std::sync::PoisonError::into_inner);
(B[m     let (socket_path, stop, handle) = start_server();
     let (mut stream, mut connection) = connect_and_handshake(&socket_path);
 
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server/tests/attach.rs:1270:
[31m-    let pane = new_pane_running(&mut stream, &mut connection, vec!["sh", "-c", "exec sleep 100"]);
(B[m[32m+    let pane = new_pane_running(
(B[m[32m+        &mut stream,
(B[m[32m+        &mut connection,
(B[m[32m+        vec!["sh", "-c", "exec sleep 100"],
(B[m[32m+    );
(B[m     let attached = attach(&mut stream, &mut connection, &pane);
     flood_stdin(&mut stream, &connection, attached.stream);
 
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 142 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-16T17:59:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes, last 40 of 65 lines:

```text
    |     ^
    |
    = help: if this is supposed to be its own paragraph, add a blank line
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#doc_lazy_continuation
help: indent this line
    |
162 | ///   like one that needs a resync: a fresh `Full`, which already carries
    |     ++

error: doc list item without indentation
   --> crates/sdk/src/screen.rs:163:5
    |
163 | /// the current size by construction, no extra field needed on `Delta` at
    |     ^
    |
    = help: if this is supposed to be its own paragraph, add a blank line
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#doc_lazy_continuation
help: indent this line
    |
163 | ///   the current size by construction, no extra field needed on `Delta` at
    |     ++

error: doc list item without indentation
   --> crates/sdk/src/screen.rs:164:5
    |
164 | /// all.
    |     ^
    |
    = help: if this is supposed to be its own paragraph, add a blank line
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#doc_lazy_continuation
help: indent this line
    |
164 | ///   all.
    |     ++

error: could not compile `steg-sdk` (lib) due to 7 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg-sdk` (lib test) due to 7 previous errors
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-16T18:00:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes, last 40 of 42 lines:

```text
    Checking steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/sdk)
   Compiling ghostty-vt-sys v0.8.0+ghostty.1.4.0-main.83fb956
    Checking steg-ui v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/ui)
    Checking steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/app)
   Compiling ghostty-vt v0.8.0+ghostty.1.4.0-main.83fb956 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/third_party/ghostty-vt-patched)
    Checking steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/kernel)
    Checking steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Checking steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/steg)
error: doc list item without indentation
    --> crates/server/tests/attach.rs:1273:5
     |
1273 | /// wait on the worker thread past a stated timeout; only the final two
     |     ^
     |
     = help: if this is supposed to be its own paragraph, add a blank line
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#doc_lazy_continuation
     = note: `-D clippy::doc-lazy-continuation` implied by `-D warnings`
     = help: to override `-D warnings` add `#[allow(clippy::doc_lazy_continuation)]`
help: indent this line
     |
1273 | ///   wait on the worker thread past a stated timeout; only the final two
     |     ++

error: doc list item without indentation
    --> crates/server/tests/attach.rs:1274:5
     |
1274 | /// (`settle_worker`/`join_worker`) touch it at all, and both are bounded.
     |     ^
     |
     = help: if this is supposed to be its own paragraph, add a blank line
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#doc_lazy_continuation
help: indent this line
     |
1274 | ///   (`settle_worker`/`join_worker`) touch it at all, and both are bounded.
     |     ++

error: could not compile `steg-server` (test "attach") due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-16T18:04:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 089079ad1c2c with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (29.3s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (24.5s)
ok      build (steg-sdk) (0.7s)
ok      test (steg-sdk) (2.0s)
ok      test (steg-sdk, local-socket) (2.4s)
ok      build (steg-kernel) (28.6s)
ok      test (steg-kernel) (1.1s)
ok      build (steg-server) (28.8s)
ok      test (steg-server) (21.1s)
ok      test (steg-server, race-fixture) (22.3s)
ok      build (steg) (1.9s)
ok      test (steg) (0.5s)
ok      test (steg, fixtures) (5.6s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (1.2s)
ok      test (steg-app) (0.8s)
ok      tool tests (12.0s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T16:43:00Z

---

Plan approved by @bruk (ec385ef4d563, size-3): its worker may build it.

---
<!-- COMMENTS:END -->
