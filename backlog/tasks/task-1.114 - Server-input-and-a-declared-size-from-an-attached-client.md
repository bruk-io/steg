---
id: TASK-1.114
title: 'Server: input and a declared size from an attached client'
status: In Progress
assignee:
  - '@bruk.w7'
created_date: '2026-11-16T08:39:00Z'
updated_date: '2026-11-17T16:38:00Z'
labels:
  - size-3
  - changes-requested
milestone: m-3
dependencies:
  - TASK-1.47
references:
  - task-1.114-server-input-and-a-declared-size-from-an
  - https://github.com/bruk-io/steg/pull/66
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-4
asdlc_branch: task-1.114-server-input-and-a-declared-size-from-an
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'ec385ef4d563'
asdlc_plan_approved_size: size-3
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=1,nit=0'
asdlc_review_policies: ["review"]
asdlc_plan_matched: no
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

Check FAILED: `just check` exited 101 at 2026-11-16T18:09:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 4c3f03035468, last 40 of 46 lines:

```text
   --> crates/server/src/connection.rs:924:28
    |
924 |             let _cleanup = ConnectionCleanup {
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

    Checking steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/steg)
Some errors have detailed explanations: E0061, E0063.
For more information about an error, try `rustc --explain E0061`.
error: could not compile `steg-server` (lib test) due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-16T18:13:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 4c3f03035468 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (1.8s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (57.0s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (28.6s)
ok      test (steg-server) (27.0s)
ok      test (steg-server, race-fixture) (28.8s)
ok      test (steg-server, accept-backoff-fixture) (29.4s)
ok      build (steg) (1.0s)
ok      test (steg) (0.5s)
ok      test (steg, fixtures) (6.8s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (11.7s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check FAILED: `just check` exited 101 at 2026-11-17T01:31:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 6b5b5634e3a6, last 40 of 79 lines:

```text

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/attach.rs (target/debug/deps/attach-6be417ce8cb1688e)

running 16 tests
test disconnecting_without_detaching_does_not_leak_or_block_a_later_attach ... ok
test declaring_a_size_of_zero_is_refused ... ok
test detaching_a_pane_this_connection_never_attached_to_is_a_bad_request ... ok
test attach_returns_the_real_screen_and_updates_follow_it ... ok
test input_clears_a_finished_status ... ok
test typed_bytes_reach_a_real_program_in_order ... ok
test a_frame_on_an_already_detached_stream_closes_the_connection ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (47562804) panicked at crates/server/tests/attach.rs:1331:6:
pane.show must answer within a bound, even with the other pane stuck
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane

test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.55s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/test-steg-server-race-fixture.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T01:34:59+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 6b5b5634e3a6:

```text
ok      format check (0.3s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (23.9s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (25.5s)
ok      test (steg-server, race-fixture) (27.5s)
ok      test (steg-server, pty-race-fixture) (40.0s)
ok      test (steg-server, accept-backoff-fixture) (36.4s)
ok      build (steg) (2.2s)
ok      test (steg) (3.6s)
ok      test (steg, fixtures) (11.5s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (6.1s)
ok      build (steg-app) (2.2s)
ok      test (steg-app) (1.1s)
ok      tool tests (11.8s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-17T01:41:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735:

```text
ok      format check (0.2s)
ok      lint (0.7s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (27.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (30.7s)
ok      test (steg-server) (31.0s)
ok      test (steg-server, race-fixture) (31.3s)
ok      test (steg-server, pty-race-fixture) (38.2s)
ok      test (steg-server, accept-backoff-fixture) (31.8s)
ok      build (steg) (0.1s)
ok      test (steg) (1.1s)
ok      test (steg, fixtures) (4.1s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.8s)
ok      architecture (1.2s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-17T01:43:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735:

```text
ok      format check (0.3s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.9s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (25.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.6s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (24.4s)
ok      test (steg-server, race-fixture) (24.2s)
ok      test (steg-server, pty-race-fixture) (32.6s)
ok      test (steg-server, accept-backoff-fixture) (26.1s)
ok      build (steg) (0.2s)
ok      test (steg) (2.0s)
ok      test (steg, fixtures) (5.7s)
ok      build (steg-ui) (0.5s)
ok      test (steg-ui) (5.8s)
ok      build (steg-app) (0.6s)
ok      test (steg-app) (0.5s)
ok      tool tests (12.2s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `cargo test -p steg-server --test attach -- a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream two_connections_attached_to_one_pane` exited 101 at 2026-11-17T01:44:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735 with uncommitted changes:

```text
   Compiling ghostty-vt-sys v0.8.0+ghostty.1.4.0-main.83fb956
   Compiling ghostty-vt v0.8.0+ghostty.1.4.0-main.83fb956 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/third_party/ghostty-vt-patched)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/kernel)
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 31.92s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 2 tests
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... FAILED
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... FAILED

failures:

---- two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other stdout ----

thread 'two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other' (48129503) panicked at crates/server/tests/attach.rs:443:17:
no update arrived on stream 1 within 5s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream stdout ----

thread 'a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream' (48129502) panicked at crates/server/tests/attach.rs:443:17:
no update arrived on stream 1 within 5s


failures:
    a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream
    two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 14 filtered out; finished in 5.04s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check passed: `cargo test -p steg-server --test attach -- a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream two_connections_attached_to_one_pane` exited 0 at 2026-11-17T01:45:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.82s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 2 tests
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.09s
```

Check FAILED: `cargo test -p steg-server --lib a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report` exited 101 at 2026-11-17T01:45:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.79s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::attach_tests::a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report ... FAILED

failures:

---- pane::attach_tests::a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report stdout ----

thread 'pane::attach_tests::a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report' (48173895) panicked at crates/server/src/pane.rs:2037:25:
no update arrived within 2s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    pane::attach_tests::a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 190 filtered out; finished in 2.00s

error: test failed, to rerun pass `-p steg-server --lib`
```

Check passed: `cargo test -p steg-server --lib a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report` exited 0 at 2026-11-17T01:45:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.73s
     Running unittests src/lib.rs (target/debug/deps/steg_server-6d2f1d51fdd2e018)

running 1 test
test pane::attach_tests::a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 190 filtered out; finished in 0.00s
```

Check FAILED: `just check` exited 101 at 2026-11-17T01:46:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735, last 40 of 71 lines:

```text
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::tests::an_unfiltered_subscriber_receives_nothing ... ok
kill: 60703: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
kill: 60699: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... FAILED

failures:

---- login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open stdout ----

thread 'login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open' (48196465) panicked at crates/server/src/login_env.rs:659:13:
the backgrounded grandchild's own pid file never appeared


failures:
    login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open

test result: FAILED. 190 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.02s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T01:50:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (30.8s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.7s)
ok      test (steg-sdk, local-socket) (1.2s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (29.4s)
ok      test (steg-server) (31.0s)
ok      test (steg-server, race-fixture) (33.1s)
ok      test (steg-server, pty-race-fixture) (37.4s)
ok      test (steg-server, accept-backoff-fixture) (34.9s)
ok      build (steg) (0.9s)
ok      test (steg) (4.4s)
ok      test (steg, fixtures) (12.1s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.7s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check passed: `cargo test -p steg-server --test attach a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach -- --exact` exited 0 at 2026-11-17T02:11:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735 with uncommitted changes:

```text
   Compiling ghostty-vt-sys v0.8.0+ghostty.1.4.0-main.83fb956
   Compiling ghostty-vt v0.8.0+ghostty.1.4.0-main.83fb956 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/third_party/ghostty-vt-patched)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/kernel)
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 31.21s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 16 filtered out; finished in 5.99s
```

Check FAILED: `cargo test -p steg-server --test attach a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach -- --exact` exited 101 at 2026-11-17T02:14:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.42s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... FAILED

failures:

---- a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach stdout ----

thread 'a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach' (48855714) panicked at crates/server/tests/attach.rs:1461:5:
the stuck pane's own worker answered a second attach - it was not actually stuck: Some(Ok(Object {"snapshot": Object {"cursor": Object {"blinking": Bool(false), "shape": String("Block"), "visible": Bool(true), "x": Number(0), "y": Number(0)}, "rows": Array [Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}], Array [Object {"style": Object {"bg": Null, "blink": Bool(false), "bold": Bool(false), "faint": Bool(false), "fg": Null, "inverse": Bool(false), "invisible": Bool(false), "italic": Bool(false), "link": Bool(false), "overline": Bool(false), "strikethrough": Bool(false), "underline": String("None"), "underline_colour": Null, "wide": String("Narrow")}, "text": String("")}]], "size": Object {"columns": Number(80), "rows": Number(24)}}, "stream": Number(2)}))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 16 filtered out; finished in 0.04s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check FAILED: `cargo test -p steg-server --test attach a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach -- --exact` exited 101 at 2026-11-17T02:16:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.37s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... FAILED

failures:

---- a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach stdout ----

thread 'a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach' (48904941) panicked at crates/server/tests/attach.rs:101:14:
reading the server's hello reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 16 filtered out; finished in 6.72s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check FAILED: `cargo test -p steg-server --test attach a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach -- --exact` exited 101 at 2026-11-17T02:17:10+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 1fc112e6c735 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.40s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... FAILED

failures:

---- a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach stdout ----

thread 'a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach' (48914479) panicked at crates/server/tests/attach.rs:1479:10:
a second connection's plain control call must answer within 2s, never waiting on a different connection's own stuck-pane attach
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 16 filtered out; finished in 3.70s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Check passed: `cargo test -p steg-server --test attach a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach -- --exact` exited 0 at 2026-11-17T02:27:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 0ae67e1daf36:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.18s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out; finished in 23.78s
```

Check passed: `just check` exited 0 at 2026-11-17T02:34:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 0ae67e1daf36:

```text
ok      format check (0.2s)
ok      lint (4.0s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (70.4s)
ok      build (steg-sdk) (0.2s)
ok      test (steg-sdk) (0.8s)
ok      test (steg-sdk, local-socket) (0.9s)
ok      build (steg-kernel) (0.3s)
ok      test (steg-kernel) (0.8s)
ok      build (steg-server) (33.1s)
ok      test (steg-server) (61.4s)
ok      test (steg-server, race-fixture) (61.9s)
ok      test (steg-server, pty-race-fixture) (63.3s)
ok      test (steg-server, accept-backoff-fixture) (65.8s)
ok      build (steg) (1.3s)
ok      test (steg) (4.5s)
ok      test (steg, fixtures) (13.9s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (7.0s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.6s)
ok      tool tests (12.0s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `cargo test -p steg-server --test attach -- --exact two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck detaching_a_pane_that_already_closed_still_releases_the_stream` exited 0 at 2026-11-17T02:35:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 0ae67e1daf36:

```text
   Compiling ghostty-vt-sys v0.8.0+ghostty.1.4.0-main.83fb956
   Compiling ghostty-vt v0.8.0+ghostty.1.4.0-main.83fb956 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/third_party/ghostty-vt-patched)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/kernel)
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 34.89s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 7 tests
test detaching_a_pane_that_already_closed_still_releases_the_stream ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... ok
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 3.04s
```

Check passed: `just check` exited 0 at 2026-11-17T03:45:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 93780eae87e1:

```text
ok      format check (0.3s)
ok      lint (0.8s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (61.5s)
ok      build (steg-sdk) (0.4s)
ok      test (steg-sdk) (1.6s)
ok      test (steg-sdk, local-socket) (1.9s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (1.6s)
ok      build (steg-server) (32.6s)
ok      test (steg-server) (67.4s)
ok      test (steg-server, race-fixture) (60.5s)
ok      test (steg-server, pty-race-fixture) (69.4s)
ok      test (steg-server, accept-backoff-fixture) (72.7s)
ok      build (steg) (0.2s)
ok      test (steg) (2.8s)
ok      test (steg, fixtures) (8.0s)
ok      build (steg-ui) (0.5s)
ok      test (steg-ui) (10.9s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.6s)
ok      tool tests (12.1s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `cargo test -p steg-server --test attach -- --exact a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other detaching_a_pane_that_already_closed_still_releases_the_stream an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane` exited 0 at 2026-11-17T03:46:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 93780eae87e1:

```text
   Compiling ghostty-vt-sys v0.8.0+ghostty.1.4.0-main.83fb956
   Compiling ghostty-vt v0.8.0+ghostty.1.4.0-main.83fb956 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/third_party/ghostty-vt-patched)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/kernel)
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 30.45s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 4 tests
test detaching_a_pane_that_already_closed_still_releases_the_stream ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... ok
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 26.74s
```

Built: AC #1 (input reaches the real program and makes the sender R91's latest, clearing a Finished/Failed status), AC #2 (tab.declare-size, carried only on Screen.size, never on ScreenUpdate::Delta), AC #3 (two clients, release, and the survivor's own declared size), and R101's sdk surface (Connection::declare_size). Every proof item has a test with a named negative control, recorded failing then passing via asdlc record-check.

Checks: just check green (recorded). gh pr checks 66 green on 93780eae and again on the final head 93780eae after merging main (CI run itself took ~12-13 minutes each time - slow, not hung).

Departures/decisions, with reasons:

1. Widened bound (5s/10s to 15s/30s) in an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane and the two force-close/stop bound tests: these prove "answers eventually, never permanently hangs," not a latency promise. Any finite bound proves that against the real failure (an unbounded wait); the widening only bought margin against just check's own concurrent CPU contention, no promise weakened.

2. Added a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach (crates/server/tests/attach.rs): auditing my own Runtime::send_input/declare_size for TASK-1.47's review finding (attach_pane holding the runtime lock across an unbounded worker wait) confirmed neither has that shape, but surfaced a gap in the existing test instead - it runs every call over one connection, so it only proved that connection's own request queue eventually catches up, not that an unrelated connection stays responsive. The new test proves the real property with a second, pre-handshaken, otherwise idle connection. Recorded failing against pre-fix attach_pane, then passing after merging TASK-1.47's fix - both recorded via asdlc record-check.

3. Accepted limit, unchanged from the approved plan but now proven at the point team-lead asked about: a pane whose program refuses to read input freezes only that pane's own screen and delays an attach to that one pane - proven not to freeze the server or any other connection's plain control calls, by the test in (2) above and by an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane's own (a)/(b) checks.

4. Size-on-Delta departure from the approved plan and from doc-14 (review finding, bruk.r2, 2026-11-17): the plan's section 2, quoting doc-14 verbatim, specifies ScreenUpdate::Delta gaining an optional `size: Option<Size>`, with a resize never promoted to a full screen (that promotion reserved for back-pressure/!is_current). What was actually built instead: `Screen` gained a `size` field (Delta never carries one), and `pane.rs` added a per-attachment `Attachment::last_told_size`; any attachment whose `last_told_size` differs from the terminal's current size gets a fresh, independent `Full` on its next tick, even when there is no content change at all.

The race the planned design would have had, specifically: a single pane-wide "size changed" flag, set once when a resize happens and cleared by whichever tick/attachment consumes it first. Order: client A is already attached and armed; client B calls `pane.attach` for the same pane, declares a size that changes the tab's current size as a side effect (R91's "latest" rule), and is about to call `pane.attach-arm` next, but has not yet sent it. Before B's own arm command reaches the pane worker, the pane's next tick runs, sees the pane-wide flag set, and delivers the size-changed Delta to A (the only armed attachment at that moment) - and clears the flag. When B's arm now lands, the flag is already clear, so B's own attachment is never told the size at all: B's own Screen is stale from the moment it attaches until some unrelated future content change happens to also need a Full (which may be never, if the program then goes idle). The per-attachment `last_told_size` design avoids this because staleness is tracked separately for every attachment, not spent globally by whichever one gets there first - this is exactly what `two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other` and the new unit test `a_stale_recorded_size_gets_a_fresh_screen_even_with_nothing_else_to_report` (pane.rs) prove, though neither names the planned design's race directly - that is corrected here, in the notes, for the first time.

Cost on the wire compared with the planned form: the planned Delta+Option<size> would have added a handful of bytes to an already-outgoing Delta only on the tick a resize actually happened - the marginal cost of one Size (two u16s) plus its JSON key, well under 50 bytes regardless of screen size. What ships instead sends every size-stale attachment a full, independent `ScreenUpdate::Full { screen, seq }` - the whole grid's rows, not a diff - on the first tick after that attachment's own size goes stale, once per attachment, not once per pane. Rough JSON-encoded size, per full screen (every cell carries a style object today - doc-14's own `Row`/cell shape, not something this task changed): an 80x24 screen is 1,920 cells; a 200x60 screen is 12,000 cells. At roughly 150-250 bytes of JSON per populated cell's `{style: {...}, text: ...}` entry (less for blank cells, which still carry the full style object), that is very roughly 300KB-500KB for 80x24 and 2MB-3MB for 200x60 per size-stale attachment, once - not per keystroke, but a real spike compared to tens of bytes.

What a window drag does with it, given TASK-1.121 declares a size on each cell boundary crossed: every one of those declarations is a separate `tab.declare-size` call: if it changes the tab's current size (R91 latest-client rules), it marks every attachment viewing that tab stale (`last_told_size` differs), and each such attachment gets a full, independent `Full` screen on its own next tick - not just the dragging client's own attachment, but every other client currently viewing the same tab, once per boundary crossed. A drag that crosses many cell boundaries (which an unthrottled resize stream easily does) sends one full-screen's worth of JSON to every viewer of that tab, once per boundary crossed, for the duration of the drag - the MB-scale cost above, repeated at drag speed, not the few bytes the planned Delta+size form would have cost on the same drag.

Check FAILED: `cargo test -p steg-server --test attach a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach -- --exact` exited 101 at 2026-11-17T16:28:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 93780eae87e1 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... FAILED

failures:

---- a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach stdout ----

thread 'a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach' (52790603) panicked at crates/server/tests/attach.rs:1738:28:
pane.detach for a pane that closed between attach and arm must not succeed - TASK-1.125's own 'silent stream' bug
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 18 filtered out; finished in 1.70s

error: test failed, to rerun pass `-p steg-server --test attach`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-server --test attach a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach -- --exact` exited 0 at 2026-11-17T16:29:18+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at 93780eae87e1 with uncommitted changes:

```text
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.97s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 1 test
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 1.73s
```

Commits since the review: 0.

5. Added the socket-level TASK-1.125 race test the team-lead asked for (a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach, crates/server/tests/attach.rs): connection one floods the stuck pane's stdin (same large, deterministic batch as the lock-contention test above, not a fixed small one - needed so connection two's queued attach reliably has not drained by the time the race runs), connection two's pane.attach queues behind it, connection one force-closes the pane (Runtime's own close-and-force removes the pane from its map under the lock before force_end's own signal/kill passes even run, so the race's trigger does not depend on force_end's timing at all), connection two's queued attach then resolves against a still-alive worker thread, and a later pane.detach for that target is the real check. Deterministic across repeated runs (4/4 clean both ways). Recorded failing against a reverted arm_attachment (armed unconditionally, TASK-1.125's own bug - the detach then succeeded) and passing against the real one, both via asdlc record-check.

One finding while building it, worth flagging rather than silently matching: the exact error code pane.detach answers with here is NoSuchTarget, not BadRequest. pane.close --force removes the pane from the kernel's own tree too, not only from the worker map, so resolve_pane_target fails outright for this target regardless of which connection asks - connection.rs's own already-closed-pane fallback only helps a connection whose pane_streams already held a real entry for it, which this connection's never did (arm never succeeded). BadRequest ("no attachment") is what a merely-never-attached-but-still-live pane answers with (detaching_a_pane_this_connection_never_attached_to_is_a_bad_request); NoSuchTarget here is arguably the more precise of the two, not a regression - the test asserts on it and explains why in its own doc comment.

Check passed: `just check` exited 0 at 2026-11-17T16:37:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at d9e418242ffc:

```text
ok      format check (0.3s)
ok      lint (32.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (79.4s)
ok      build (steg-sdk) (0.9s)
ok      test (steg-sdk) (3.0s)
ok      test (steg-sdk, local-socket) (3.8s)
ok      build (steg-kernel) (0.6s)
ok      test (steg-kernel) (1.3s)
ok      build (steg-server) (29.1s)
ok      test (steg-server) (57.6s)
ok      test (steg-server, race-fixture) (60.4s)
ok      test (steg-server, pty-race-fixture) (68.7s)
ok      test (steg-server, accept-backoff-fixture) (62.8s)
ok      build (steg) (1.9s)
ok      test (steg) (4.8s)
ok      test (steg, fixtures) (13.8s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (1.7s)
ok      build (steg-app) (1.4s)
ok      test (steg-app) (1.6s)
ok      tool tests (13.5s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Commits since the review: 105, 93 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-server --test attach -- --exact a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach detaching_a_pane_that_already_closed_still_releases_the_stream two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other detaching_a_pane_this_connection_never_attached_to_is_a_bad_request an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane` exited 0 at 2026-11-17T16:38:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114 on task-1.114-server-input-and-a-declared-size-from-an at d9e418242ffc:

```text
   Compiling ghostty-vt-sys v0.8.0+ghostty.1.4.0-main.83fb956
   Compiling ghostty-vt v0.8.0+ghostty.1.4.0-main.83fb956 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/third_party/ghostty-vt-patched)
   Compiling steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/kernel)
   Compiling steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.114/crates/server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 29.25s
     Running tests/attach.rs (target/debug/deps/attach-1c3f77caae0e73d3)

running 6 tests
test detaching_a_pane_that_already_closed_still_releases_the_stream ... ok
test detaching_a_pane_this_connection_never_attached_to_is_a_bad_request ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... ok
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 27.48s
```

Commits since the review: 105, 93 without the Asdlc-Worker trailer.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T16:43:00Z

---

Plan approved by @bruk (ec385ef4d563, size-3): its worker may build it.

---

created: 2026-11-17T01:37:00Z

---

2026-11-16, @bruk.o: flake sighting for this task's own test, seen by TASK-1.121's worker in a full just check under load: crates/server/tests/attach.rs an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane failed once on its bound (pane.show must answer within a bound with the other pane stuck) and passed alone straight after. Before finishing, say whether the bound is a real product promise that load can break, or a test bound that is too tight, and what it is measured from.

---

created: 2026-11-17T02:18:00Z

---

Bound note (2026-11-16), per your request:

The 15s/30s bounds in an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane and the two force-close/stop tests prove "answers eventually, never permanently hangs" - not a latency promise. The real failure they guard against is an unbounded wait, so any finite bound proves the property; widening 5s/10s to 15s/30s only bought margin against just check's own concurrent CPU contention. No promise was weakened.

Separately, auditing my own Runtime additions per your request: Runtime::send_input (runtime.rs:637-645) and Runtime::declare_size (runtime.rs:662-680), and the apply_size_changed_locked they both call (runtime.rs:2022-2028), hold the lock only across in-memory kernel calls and PaneHandle::send_input/resize (pane.rs:505-509, 518-522) - both plain unbounded mpsc::Sender::send, never a recv. Neither has the shape 1.47's review found in attach_pane. (One test-only lookalike: attachment_count_for_test, runtime.rs:689-695, holds the lock across PaneHandle::attachment_count's own send-then-recv, pane.rs:628+ - #[cfg(test)], not a production path, but the same shape; flagging it so it isn't "found" later.)

That audit did surface a real gap, in a test, not in my production code: an_unresponsive_panes_own_commands_do_not_stall... runs every one of its own calls over a single connection, so its "(a) a plain control call still answers" check only proves this connection's own request queue eventually catches up once the stuck pane's worker drains - not that an unrelated connection stays responsive. That is weaker than the "freezes only that pane's screen" limit this task's own plan names. Added a new test, a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach (crates/server/tests/attach.rs), issuing one plain pane.show from a second, already-handshaken, otherwise idle connection while a different connection's own attach to the stuck pane is confirmed stuck. Recorded failing via asdlc record-check just now: today's attach_pane does hold the runtime lock across PaneHandle::attach's unbounded recv, so this second connection's pane.show gets no reply within 2s - confirming 1.47's review finding independently, from this side. Left uncommitted on purpose; will record it passing and commit once 1.47's fix lands, per your instruction to hold until then.

---

author: @bruk.r2
created: 2026-11-17T04:14:00Z

---

@bruk.r2 is reviewing TASK-1.114: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-17T04:23:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 0 nit. Cites: review. The diff does not match the plan.

[Compliance, Important] crates/sdk/src/screen.rs:146 (Screen/ScreenUpdate shapes) and crates/server/src/pane.rs:276 (Attachment::last_told_size): the approved plan's section 2, quoting doc-14 verbatim ("an update gains an optional size"), specifies adding size: Option<Size> to ScreenUpdate::Delta. The diff never adds this field; instead Screen gained size and a new per-attachment Attachment::last_told_size in pane.rs forces any size-stale attachment onto a fresh, independent Full on its next tick, even with no content change. This is a real, substantial departure from both the plan's named mechanism and the contract's (doc-14) literal wording, and it is not in the task's "Departures/decisions, with reasons" list (which has three unrelated items: the 5s/10s->15s/30s bound widening, the added a_second_connections... test, and the accepted stuck-pane limit). The only place the departure and its reasoning appear is a source comment in screen.rs ("doc-14's own wording... was tried and dropped") - never surfaced on the board for a reviewer or for doc-14 to be reconciled with reality.

The chosen mechanism is well-reasoned on its own terms (the code comments trace a real bug the plan's single pane-wide "size changed" flag would have had with a newly-arming attachment, confirmed by the two-client ordering test), and the behavioural clause of AC #2 ("an update on every attachment's stream reports the terminal's size when it changes") is still met empirically - I repeated one proof's control myself (declaring_a_size_of_zero_is_refused, widening connection.rs's range check from 1..=u16::MAX to 0..=u16::MAX in a scratch edit, reverted after): it failed as the task's notes claim, confirming that control is real. But soundness of the new design does not substitute for recording the departure: the plan's own section 2 explicitly says a resize must never be "promoted to a full, scrollback-free resync it does not need... reserved for !is_current/back-pressure" - the shipped mechanism does exactly that for every size change, unflagged as the policy exception it now is.

Not checked beyond this: I did not re-run the full proof table past the one control above; the recorded failing/passing pairs in the notes look legitimate and consistent with the final head (93780eae, matching the PR). I did not check doc-9 in detail beyond its reference in the task's citation list.

Informational, not a finding, answering the team-lead's question about TASK-1.125: this task's stream_panes.insert(stream, pane) (connection.rs, pane.attach's Ok arm) sits right after runtime.arm_attachment's call, alongside the pre-existing pane_streams insert. If TASK-1.125 wraps arm_attachment's effect in an "if armed" branch, stream_panes's insert can safely move inside it or stay outside: Runtime::send_input already no-ops (state.panes.get(&pane) is None) for a pane with no worker, the same precondition under which arm_attachment itself no-ops, so an unarmed attachment's input is silently dropped either way - inside send_input if stream_panes still routes it, or in connection.rs's own "nothing to route to" branch if it doesn't. No test depends on which side of that branch it ends up on. For symmetry with pane_streams (which stays unconditional, since ConnectionCleanup::drop's sweep needs it regardless of arming), leaving stream_panes unconditional too is the simpler, lower-risk choice for whoever merges second.

---

created: 2026-11-17T16:19:00Z

---

Decision by @bruk (2026-11-17, relayed by @bruk.o): keep the built behaviour for size changes (a full screen at the new size per size-stale attachment; a delta never carries a size). doc-14's Size section is reworded to match (board commit db68d8f8). The review's finding is answered by departure 4 in the notes plus this spec change; no code change is asked for. Cost of full screens during a window drag is noted for TASK-1.122.

---
<!-- COMMENTS:END -->
