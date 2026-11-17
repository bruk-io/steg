---
id: TASK-1.121
title: 'App: typing and resizing in the bare window'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T15:26:00Z'
updated_date: '2026-11-17T00:40:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.117
  - TASK-1.114
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.121-app-typing-and-resizing-in-the-bare
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '344be7f33227'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.117 by its plan on 2026-11-16, under the engineer's standing decision to split by default: the window first shows a live terminal read-only (TASK-1.117), then this adds input and size. doc-9; doc-10 R20, R22; decision-2 section 3. Needs TASK-1.114 (input on the attachment's stream; tab declare-size; the size on an update). TASK-1.117's plan holds the worked design for the key mapping, the input handler and the resize. Left out, for TASK-1.56 or later: application cursor mode, bracketed paste, the mouse, the Kitty keyboard protocol; so full-screen programs are not fully usable yet. Carries TASK-1.117's original second and third criteria and the recording half of its sixth.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Typing reaches the program and its output is drawn: a shell prompt accepts a command and shows the result; keys that are text, Enter, Backspace, Tab, Escape, the arrows and Control combinations arrive as the bytes a terminal sends, from a pure mapping with a table and tests; text composed by the input method arrives as text
- [ ] #2 The window declares its size on opening and when resized, coalesced so a drag does not flood the server, and draws the terminal at the size the server reports, filling the window
- [ ] #3 Closing the window leaves the session running on the server, and running the app again shows the same screen, including what was typed before
- [ ] #4 The pull request carries a short recording of typing a command and resizing; what can be tested without a window has tests with recorded controls; nothing in just check or CI opens a window
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer

1. **One task, not two — kept as already split from TASK-1.117.** Typing and size share one
   writer (the second `Connection` TASK-1.117's own plan already designed for, §1 below),
   one startup path, and one pull request's recording (both shown typing and resizing in the
   same clip). Within the task, typing is built and proven first (it needs nothing from
   §5's resize work), size second — matching AC #1 before AC #2's own numbering. No further
   split proposed.
2. **The Option key stays composed-character, as TASK-1.117's own approved plan already
   decided ("decided, not left open").** I checked whether a cheaper per-key split (left
   Option composes, right Option is Meta) is available at the pinned version, since the
   brief asked for it if cheap: it is not. `Keystroke`/`Modifiers`
   (`platform/keystroke.rs:18-33`, `:446-470`) carry one `alt: bool` with no side — GPUI's
   public surface at 0.2.2 has no left/right distinction to read, and doc-16 R106/R98 keep
   this app from reaching past `gpui` for one. So this isn't a decision left for the
   engineer to weigh between two live options; it's confirmation that the one TASK-1.117
   already made is the only one buildable today. Carried forward, not reopened: composed
   character, which is what makes dead keys, non-US layouts and the emoji picker work at
   all through a minimal `EntityInputHandler` (§2 below); the named cost is still that
   `Alt-b`/`Alt-f`-style readline word motions don't work out of the box, same as before,
   fixed later by a setting if Bruk wants Meta instead.
3. **Cmd-V paste is included, and is handled inside the raw keystroke path, not as a GPUI
   `KeyBinding`/action.** Keeping it there, rather than `cx.bind_keys([KeyBinding::new(
   "cmd-v", Paste, None)])` the way `gpui`'s own `examples/input.rs:688` does it, is what
   keeps TASK-1.117's "zero `KeyBinding`s, R29 holds trivially" claim literally true after
   this task lands, not merely "mostly true" — Cmd combinations are already dropped by the
   same classifier that would otherwise send bytes, so Cmd-V is one more row in it:
   `cx.read_from_clipboard()` (confirmed present at 0.2.2 from the same example) read at the
   point of handling, its text's UTF-8 bytes sent the same way typed bytes are. No bracketed
   paste exists yet (TASK-1.56/1.57), so a multi-line paste is sent as plain bytes with every
   line ending translated to `\r` (exactly how Enter is encoded, §2's table) — a shell sees
   it as if each line had been typed and Enter pressed, which is adequate for the common
   case and wrong for a program that reads raw lines expecting `\n` or that would have
   wanted the whole block pasted atomically (vim's `:paste`-free default autoindent mangling
   a multi-line paste is the standard symptom bracketed paste exists to fix) — named here,
   not fixed; TASK-1.57 owns the rest of selection/copy/paste, including bracketed paste if
   it lands there. Select and copy themselves are entirely TASK-1.57's, untouched here.
4. **The recording's "throwaway home" cannot be arranged through this app's own startup
   call.** TASK-1.117's `startup.rs` calls `session.new(label: "steg-app", program: None)`;
   `session_methods()`'s own params for `session.new`
   (`crates/sdk/src/description/methods.rs:224-248`) are `label`, `cwd`, `focus`, `program`
   — there is no `env` parameter today, so the app has no way to hand the server a scrubbed
   `HOME` for the pane's shell, and `cwd` alone does not scrub a shell's history file,
   prompt, or anything else keyed off `$HOME`. The mechanism has to sit one level out:
   start the server process itself with a scrubbed environment before the app connects to
   it (`env -i HOME="$(mktemp -d)" PATH="$PATH" steg run`, or however the engineer's own
   `just run`/`steg` invocation is shaped), not a per-session override. For the capture tool
   itself: `screencapture -v <path>.mov` is the built-in option (same one-time Screen
   Recording grant `crates/ui/CLAUDE.md` already documents for `capture-gallery`, started
   and stopped by hand since this needs a live typing/resizing interaction, not a
   `just`-driven still). Confirm both — the scrubbed-server invocation and the capture tool
   — before recording, since neither is this task's own code to build.
5. **The accepted limit this task inherits, stated so the recording doesn't trip over it:**
   TASK-1.114's own "For the engineer" point 8 — a pane whose program never reads its input
   freezes that pane's own screen until the program reads, ends, or is force-closed. Typing
   into a healthy shell is unaffected; naming it here only so nobody reads a frozen demo pane
   during review as this task's own bug.

## 1. The key mapping (AC #1): a pure function, no window

**Signature and scope.** `fn key_to_bytes(key: &str, key_char: Option<&str>, modifiers:
Modifiers) -> Option<Vec<u8>>` in a new `crates/app/src/keys.rs`, called from nothing but a
`Keystroke`'s own three public fields (`platform/keystroke.rs:18-33`) — no `Window`, no
`Context`, no socket. `None` means "not sent to the program" (Cmd combinations, `fn`
combinations, anything this task doesn't map); `Some(bytes)` is what reaches
`Connection::send_data`. Byte sequences below are the published xterm/ECMA-48 convention
(xterm's `ctlseqs` documentation and the `xterm-256color` terminfo entry's normal, non-
application cursor-key mode), not read or adapted from Zed's or any other GPL terminal's
code (doc-13 R59) — cited by convention name, not by any project's source.

**Priority, in order:**

1. `modifiers.platform` (Cmd) → `None` always, even for a key this table would otherwise
   map — "Cmd combinations never sent to the program," except Cmd-V, handled one level out
   as its own case (§"For the engineer" point 3) before this function is even reached, so
   `key_to_bytes` itself never special-cases V.
2. `modifiers.function` → `None`. No established terminal encoding exists for an `fn`
   combination (it's a Mac keyboard-layer concept, not a VT one) and doc-9's acceptance set
   doesn't need one; a dropped case, not a gap requiring a byte.
3. `modifiers.control` with `key` a single ASCII letter or one of
   `[`/`\`/`]`/`^`/`_`/`?`/space → the classic C0 control-code derivation (every terminal
   emulator's own convention, not this app's invention): a letter maps to `(key.to_ascii_
   uppercase() as u8) & 0x1f` (`Ctrl-A`..`Ctrl-Z` → `0x01..0x1A`); `space` → `0x00` (NUL);
   `[` → `0x1B` (ESC — the same byte the bare Escape key sends, a genuine, stated ambiguity
   inherited from the convention itself, not an app-level bug); `\` → `0x1C` (FS); `]` →
   `0x1D` (GS); `^` → `0x1E` (RS); `_` → `0x1F` (US); `?` → `0x7F` (DEL — the same byte plain
   Backspace sends, same stated ambiguity). `Ctrl` with anything else this list doesn't name
   → `None` (dropped, not guessed at).
4. Named, non-printable keys — matched on `key` exactly as GPUI's mac backend emits it,
   taken from `is_printable_key`'s own vocabulary (`platform/keystroke.rs:377-429`) and
   `with_simulated_ime`'s own three (`:248-251`), not guessed:
   - `"enter"` → `\r` (0x0D). `"tab"` → `\t` (0x09) **unless `modifiers.shift`**, which is
     back-tab → CSI `Z` (`\x1b[Z`). `"backspace"` → `0x7F` (DEL). `"escape"` → `0x1B`.
   - `"left"`/`"right"`/`"up"`/`"down"` → `\x1b[D`/`\x1b[C`/`\x1b[A`/`\x1b[B` — normal cursor
     mode only (xterm's own default when application cursor mode, DECCKM, is off).
     **Modifiers on an arrow are dropped, not encoded**: xterm's own modified-arrow CSI
     form (`\x1b[1;5C` for Ctrl-Right, etc.) is a second table this task does not build —
     named here as a stated gap, not a silent one; a later task can add it without
     reshaping this function, since it only ever needs to look at `modifiers` here too.
   - `"home"` → `\x1b[H`. `"end"` → `\x1b[F`. `"pageup"` → `\x1b[5~`. `"pagedown"` →
     `\x1b[6~`. `"insert"` → `\x1b[2~`. `"delete"` → `\x1b[3~` (the forward-delete key,
     distinct from Backspace's DEL byte).
   - `"f1"`.."`f4`" → SS3 form `\x1bOP`..`\x1bOS` (identical in both cursor modes, which is
     why xterm uses it for exactly these four and nothing else). `"f5"`.."`f12"` → `\x1b[15~`,
     `\x1b[17~`, `\x1b[18~`, `\x1b[19~`, `\x1b[20~`, `\x1b[21~`, `\x1b[23~`, `\x1b[24~` (16
     and 22 skipped — xterm's own convention, inherited from vt100/vt220 numbering, not this
     app's choice). `"f13"` and above → `None`, named as dropped: not in doc-9's acceptance
     set and GPUI's own `is_printable_key` list only goes to `f35` with no stated mapping
     convention this task would be inventing one for.
5. Otherwise: if `key_char` is `Some(text)`, the printable case — `text.as_bytes().to_vec()`
   (UTF-8, whatever the input method or layout already resolved `key`/the modifiers to,
   including Shift — this is the same string `mac/window.rs`'s own dispatch (below) hands to
   `run_callback` for a plain, non-composing printable key). Otherwise (no `key_char`, not a
   named key above, not a dropped modifier) → `None`.

**Application cursor mode, application keypad mode, bracketed paste, mouse reporting, the
Kitty keyboard protocol and focus reporting are all out of scope, TASK-1.56's or later**, as
TASK-1.121's own description already states. The concrete cost, stated so nobody is
surprised by the demo: a full-screen program that switches into application cursor mode
(vim's insert-mode arrow keys, htop, anything reading DECCKM) still receives `\x1b[D`-style
normal-mode bytes from this task's own table, which such a program does not expect —
doc-9's acceptance set (vim, htop, tmux, Claude Code) is therefore not fully usable yet,
exactly as the description already says. Whether the mode can be read back cheaply from the
server's own updates to fix the common case: **no** — `ScreenUpdate`/`Screen`
(`crates/sdk/src/screen.rs`, as of the TASK-1.114 branch this task builds against) carry
rows, cursor and size; no field for which cursor-key mode the engine is in. Exposing one
would be a kernel/server change (reading `ghostty_vt`'s own mode state and adding it to the
wire shape), not something this task's own client-side table can shortcut — named here as
the first real follow-up for whoever picks up application-cursor-mode support.

**The text-versus-key dedup rule — one predicate, not two, and it rests on a fact the
reviewer should check against the source rather than take on my word: GPUI's own macOS
backend, not this app, decides whether a keystroke reaches `on_key_event` or the input
context, and it never sends the same physical keystroke to both.** Read from
`platform/mac/window.rs:1682-1766`:

- If an IME composition is already open (`marked_text_range()` returns `Some`, which is
  exactly why the minimal `EntityInputHandler` below must track a real marked range and not
  hard-code `None` — a handler that always answers `None` would never let `is_composing`
  become true, and the dead-key/emoji-picker path below would never engage), **or** the key
  has no `key_char` and neither `control` nor `function` is held (arrows, Escape, function
  keys, a dead key's own first press) — the native input context gets the keystroke first.
  If it consumes the keystroke (`setMarkedText`/`insertText`/a recognised selector), our
  `on_key_event` listener is never called for it at all. If it doesn't, the keystroke falls
  through to `on_key_event` as a plain `KeyDownEvent` — which is exactly the path named keys
  like arrows take in the ordinary (non-composing) case, since nothing in this app's minimal
  input handler recognises a selector for them.
- Otherwise (a plain printable key, or Control+something, with no composition open) —
  `on_key_event` is called *before* the input context ever sees the native event, and if our
  listener calls `cx.stop_propagation()` the function returns immediately
  (`mac/window.rs:1736-1739`): the input context — and therefore `replace_text_in_range` —
  is never reached for that keystroke.

So this app owns exactly one invariant, and it's what makes the dedup real rather than
assumed: **`on_key_event`'s listener calls `cx.stop_propagation()` if and only if
`key_to_bytes` returned `Some`.** Given that, no keystroke this function maps can also reach
`replace_text_in_range`, and no keystroke this function declines (composing, or a dead key's
own prefix) is ever sent as raw bytes by `on_key_event` either, since the platform routed it
to the input context before `on_key_event` ran at all. `Window::on_key_event` is registered
each paint (`window.rs:3444`, "should only be called as part of the paint phase"), matching
where `handle_input` is already registered (§2 below):

```rust
window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
    if phase != DispatchPhase::Bubble { return; }
    if let Some(bytes) = keys::key_to_bytes(
        &event.keystroke.key, event.keystroke.key_char.as_deref(), event.keystroke.modifiers,
    ) {
        client.send(bytes); // the main-thread Connection, §3 below
        cx.stop_propagation();
    }
});
```

## 2. Composed text: the minimum input handler (AC #1)

A terminal has no editable text buffer for an IME to read or replace into — unlike
`gpui`'s own `examples/input.rs` (`TextInput`, read in full for its shape, not copied: it
solves a different problem, a text field with a real `content: SharedString`), there is no
"range of the document" here. The handler therefore tracks only what composition itself
needs, nothing about the terminal's own content:

```rust
struct ImeState { marked: Option<String> } // not the terminal's grid; the IME's own scratch
```

- `marked_text_range` → `self.marked.as_ref().map(|s| 0..s.encode_utf16().count())` — real,
  not hard-coded `None` (§1's own dedup rests on this).
- `replace_and_mark_text_in_range` — sets `self.marked = Some(new_text.to_owned())` (or
  clears it, if `new_text` is empty); **sends nothing to the program.** This is the stated
  gap for uncommitted text: a dead key's pending accent, or the emoji picker's own candidate
  highlight, draws no preview on the terminal's grid (TASK-1.56's own view work owns drawing
  one, if Bruk wants it) — what matters for correctness, not polish, is that nothing is sent
  to the pty for marked text, only for committed text below.
- `replace_text_in_range` — the one path committed text takes, whether from a resolved
  composition or a direct, non-composing `insertText` call: translate `new_text`'s own line
  endings the same way Enter is encoded (`\n`/`\r\n` → `\r`, §1), send the resulting bytes,
  clear `self.marked`.
- `unmark_text` — clears `self.marked`.
- `bounds_for_range` — returns the current cursor's own cell rect (`terminal_element.rs`'s
  existing cursor-quad computation, already built for painting, reused here) so the
  candidate window anchors at the terminal's cursor rather than the window's origin; `None`
  only if no attachment has painted a cursor yet (startup's "connecting…" placeholder).
- `text_for_range`/`selected_text_range`/`character_index_for_point` — `None` always,
  an honest "nothing selectable here" answer the IME already tolerates (there is no
  selection in a read-only-to-the-IME terminal; TASK-1.57 owns real selection, which is a
  separate, mouse-driven mechanism, not this handler's business).

**Focus — new plumbing this task owns; TASK-1.117's own file list explicitly deferred it**
("no `on_key_down`, no input handler" — `crates/app/src/terminal_element.rs`'s module doc).
`Window::handle_input` no-ops unless the given `FocusHandle` `is_focused` (`window.rs:3400,
3408`), so: a `FocusHandle` field on the terminal entity (`cx.focus_handle()`), `Focusable`
implemented returning it, `.track_focus(&focus_handle)` on the root `div` in `render`, an
explicit `window.focus(&focus_handle)` once at startup right after the entity is built (the
same call `examples/input.rs:739` makes — there is exactly one focusable thing in this bare
window, so no focus-cycling logic is needed), and `window.handle_input(&focus_handle,
ElementInputHandler::new(bounds, entity), cx)` called from `terminal_element.rs`'s own
`paint`, beside `on_key_event`'s own registration (same "each paint" requirement).

## 3. The writer (AC #1)

TASK-1.117's own plan already designed the slot this fills (§1, "the main thread would own
its own second `Connection` (encode-only), seeded the same way, `open_stream(stream)` called
once from the same startup result"). `client.rs` grows a `send(&self, bytes: Vec<u8>)` method
on that second `Connection`, calling `Connection::send_data(stream, &bytes)`
(`wire/connection.rs:409`, already built, already refusing a stream this connection hasn't
opened) directly on the main thread — no channel, no lock, since only the main thread ever
writes after startup (unchanged from the appendix's own reasoning). Every call site above
(`on_key_event`, `replace_text_in_range`) reaches this one method.

## 4. Declared size (AC #2)

**Pixels to a grid, with a floor.** `fn grid_size(viewport: Size<Pixels>, metrics:
&CellMetrics) -> (u16, u16)`, pure: `columns = max(1, (viewport.width / metrics.cell_width)
.floor() as u16)`, `rows` the same with `line_height`. The floor at 1 is what keeps a zero
dimension from ever being computed, let alone sent — upstream of TASK-1.114's own server-
side `BadRequest` refusal of `columns == 0 || rows == 0`, which this task's own client code
should therefore never be able to trigger (a control for that, §6).

**Coalescing — a pure state machine, driven only by resize events, no timer.** A `declared:
Cell<(u16, u16)>` holds the last pair this client actually sent. On open, and on every
`observe_window_bounds` callback (`app/context.rs:429`, registered once from the entity's
own constructor): compute `grid_size(window.viewport_size(), &metrics)`; if it equals
`declared.get()`, do nothing; otherwise call `Connection::declare_size(Some(&tab_path),
columns, rows)` (sdk method added by TASK-1.114, `wire/connection.rs:302`-shaped) and update
`declared`. This reconciles with, rather than reverses, TASK-1.117's own appendix ("no
explicit debounce needed"): there genuinely is none — what bounds a drag is that the
*computed* pair only changes when the viewport crosses a whole cell's width or height, not a
timer, and the very last `bounds_changed` firing (mouse-up, or the final frame of a
programmatic resize) is what carries the drag's true final size, since GPUI fires it
synchronously on each real resize rather than batching or dropping intermediate ones
(`window.rs:1675-1685`). `tab_path` is `"{session}/{tab}"` (`Connection::declare_size`'s own
`tab: Option<&str>` parameter targets a tab, TASK-1.114 §2 — not the pane path startup
attaches with), built once from the same `{session}/{tab}/{pane}` string startup already
resolved and kept on the entity.

**The model draws the size the server reports, not a size it tracks itself.** TASK-1.117's
own `Model::from_snapshot` currently counts a row's own cells to fix `(columns, rows)` once,
forever, because the 1.47-only `Screen` it was built against has no `size` field at all
(`crates/app/src/model.rs:1-75`, read on the `task-1.117` worktree). TASK-1.114 adds
`Screen.size`/`ScreenUpdate::Delta.size: Option<Size>` (§2 of its own plan) precisely so a
resize is told to every attachment, including one that didn't ask for it — this task is the
first caller that needs that field read, not merely carried. Two changes, both narrowing
rather than widening what `Model` computes:

- `from_snapshot` reads `snapshot.size` directly instead of counting the first row's own
  cells — simpler, and correct even for a blank row, which cell-counting was never truly
  safe against to begin with.
- `apply`'s `Delta` arm sets `self.size = size` whenever `size` is `Some(..)` (including the
  size-only case TASK-1.114 §2 describes: empty `rows`, an advanced `seq`, `size: Some(..)`)
  — the grid's own dimensions now follow the latest declaration/input exactly as R91
  requires, drawn top-left, letterboxed or cropped exactly as TASK-1.117's own §2 already
  describes (that paragraph's reasoning is unchanged; only which size it measures against
  changes, from "fixed at attach" to "whatever the latest update says").

**This directly falsifies two tests TASK-1.117 is shipping, and the plan says so rather than
letting review find an inverted assertion on its own:**
`from_snapshot_reads_its_fixed_size_once_from_the_first_row` and
`a_full_update_replaces_the_whole_grid_but_not_the_fixed_size_it_was_built_with`
(`crates/app/src/model.rs:204`, `:234`, confirmed present on the `task-1.117` worktree at the
time this plan was written) both assert a size that *never* changes, by construction of the
branch they were written against. This task replaces both with tests asserting the opposite
property under the new `Screen`/`ScreenUpdate` shape: a `Full` establishes `size` from
`Screen.size`; a content-only `Delta` (`size: None`) leaves `size` unchanged; a `Delta` with
`size: Some(new)` changes it while still touching only the rows it names. Renamed to say
what they now prove, not patched in place under their old, now-false names.

## 5. Reopen (AC #3)

Nothing new here beyond what TASK-1.117 already built: the same session/tab is found and
attached to (its own §6, "the stale-pane case"), and whatever was typed is simply what the
real pty/shell already holds — this task makes typing possible, so what AC #3 shows is
TASK-1.117's own mechanism plus this task's own bytes, not a new persistence path. The pull
request's own recording is this task's proof of it (`echo hello`, close the window, reopen,
see it in scrollback) — weak evidence in TASK-1.117 alone (a bare, unchanged prompt), real
evidence here, which is why the team lead moved this criterion's own demonstration to this
task in TASK-1.117's own comment #2.

## 6. Proof

Every item below is recorded through `asdlc record-check` as a failing run (the mechanism
reverted) and a passing run, against the tests as they stand at the end, each with its own
named control — not a hand-run-and-reverted demonstration.

**Pure data and functions, no window, no socket:**
- `keys::key_to_bytes` — one table-driven test iterating every row named in §1 above (every
  named key, the full Control+letter/punctuation set, a plain printable `key_char`, Cmd-
  anything, fn-anything, a modified arrow) asserting the exact byte string or `None`; a
  second assertion per printable/control row that the table has no row this test doesn't
  cover, so a row added to the plan's own prose without a matching assertion fails the
  build, not just a missed case. Control: delete one arm (e.g. make Tab and Shift-Tab share
  a byte) and its own row's assertion fails.
- The dedup invariant, as a pure check on the classifier's own contract rather than a window
  test (no window can observe whether the native input context was skipped): assert that
  every row `key_to_bytes` maps to `Some` is a row this task's own `on_key_event` closure is
  written to call `stop_propagation` for, by construction (one `if let Some(bytes) = ...`
  guarding both the send and the stop_propagation call, §1's own code) — not testable as a
  second independent fact, since the invariant is exactly that there is only the one branch.
  Control: a version with two separate conditions for "send bytes" and "stop propagation"
  (rather than one `if let`) is what this shape is written to make structurally impossible,
  not merely unobserved.
- `grid_size` — a handful of `(viewport, metrics)` pairs against the expected `(columns,
  rows)`, including a viewport smaller than one cell asserting the `(1, 1)` floor, never
  `(0, 0)`. Control: drop the `max(1, ..)` and the floor assertion fails.
- The coalescing state machine, as a pure `(declared, computed) -> Option<(u16,u16)>` step
  function (extracted from §4's own callback so it's testable without `Window` at all): fed
  a sequence of computed sizes simulating a drag (several that round to the same cell pair,
  then one that doesn't, then the pre-mouse-up final one), assert it emits exactly the
  distinct sizes, in order, and always ends on the sequence's own last value. Control: skip
  the `declared` comparison (emit on every call) and the "bounded number of declarations"
  assertion fails.
- `ImeState`'s own transitions (`marked_text_range`'s derived state, `replace_and_mark_text_
  in_range` then `replace_text_in_range` then `unmark_text`) as plain data in, data out —
  assert marked text sends nothing, committed text sends exactly its own bytes once.
  Control: call `replace_text_in_range` without first clearing `marked` and the "sends
  exactly once" assertion fails (an accidental second send from stale marked state).
- `Model::apply`'s own two replaced tests (§4) and one new one for the size-only `Delta`
  shape (empty `rows`, `size: Some`) — asserts `size` changes with no row touched. Control:
  revert to reading `size` only on `Full` and the size-only-`Delta` test fails.

**A real socket, no server, no window** (`UnixStream::pair()`, the same standing testing-
rust.md shape TASK-1.117's own §4 already uses): feed a `tab.declare_size` call through the
client-side `Connection` and assert the exact bytes land on the wire in the request shape
TASK-1.114's sdk addition defines; feed input bytes through `Connection::send_data` and
assert they land as a `Data` frame on the attachment's own stream, never on stream 0.
Control: swap `send_data`'s own stream id for `0` and the frame-shape assertion fails.

**What's shown by running it.** `just run` (unchanged, `justfile:208`). The engineer runs it,
types a command, resizes the window, and captures a short recording (§"For the engineer"
point 4 — the scrubbed-server invocation and the capture tool, both named there since
neither is this task's own code). Nothing in `just check`/CI opens a window: every file
above compiles and its tests run under `just check-packages` like any other crate's code.

## 7. Building against unmerged work, and merge order

Builds against TASK-1.117's branch for `crates/app`'s existing files (`model.rs`,
`terminal_element.rs`, `client.rs`, `startup.rs`, `main.rs`) and TASK-1.114's branch for
`Screen.size`/`ScreenUpdate::Delta.size`/`Connection::declare_size`/`Connection::send_data`'s
own confirmation — both merged into this task's own branch, re-merged (not rebased, once
either is pushed) before finishing. Merge order, matching the dependency graph: TASK-1.46 →
TASK-1.47 → TASK-1.114 and TASK-1.117 (either order between those two, since they touch
different files: 1.114 is server-side, 1.117 is `crates/app`) → this task. Expected conflict
surface: `crates/app/src/model.rs` (this task's own `from_snapshot`/`apply` changes land on
top of 1.117's own, not beside them) and `crates/app/src/client.rs`/`startup.rs` (the second
`Connection`/`declare_size` wiring TASK-1.117's own plan left as a named slot for this task
to fill, not a file this task invents from nothing).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-9, doc-10 R20/R22/R29, decision-2 section 3 and TASK-1.114's plan by @bruk.o (plan 344be7f33227): ok. Key bytes come from a pure table; text and keys cannot both send (one branch sends and stops propagation); size is declared on open and when the computed cell grid changes, with no timer; the model draws the size the server reports. For the engineer to weigh: (1) Option composes characters, because GPUI 0.2.2 reports one alt flag with no left or right, so Alt-b and Alt-f word motions do not work until a setting exists; (2) Cmd-V paste is added here as plain bytes with line ends sent as Enter, ahead of TASK-1.57, without bracketed paste; (3) a drag sends one declaration per cell boundary crossed, not one per drag; (4) arrows are sent in normal mode only, so vim and htop arrow keys are wrong until the cursor-key mode is carried on an update. Note for the worker: TASK-1.117 changed its model after this plan was written (it tracks the row count and crops columns to the window), so the two tests named in section 4 may have other names now; replace whichever assert a size that never changes.

Check FAILED: `just check` exited 1 at 2026-11-17T00:13:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at f377b58d5d92 with uncommitted changes, last 40 of 64 lines:

```text
 /// and always ends on the sequence's own last distinct value" a single, direct assertion.
[31m-pub(crate) fn coalesce(initial: GridSize, computed: impl IntoIterator<Item = GridSize>) -> Vec<GridSize> {
(B[m[32m+pub(crate) fn coalesce(
(B[m[32m+    initial: GridSize,
(B[m[32m+    computed: impl IntoIterator<Item = GridSize>,
(B[m[32m+) -> Vec<GridSize> {
(B[m     let mut declared = initial;
     let mut sent = Vec::new();
     for size in computed {
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121/crates/app/src/size.rs:102:
 
     #[test]
     fn a_changed_computed_size_declares_itself() {
[31m-        assert_eq!(declare_if_changed(size(80, 24), size(81, 24)), Some(size(81, 24)));
(B[m[32m+        assert_eq!(
(B[m[32m+            declare_if_changed(size(80, 24), size(81, 24)),
(B[m[32m+            Some(size(81, 24))
(B[m[32m+        );
(B[m     }
 
     /// The bounded-declarations proof: a drag that passes through several viewport sizes
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121/crates/app/src/size.rs:112:
     #[test]
     fn a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one() {
         let drag = [
[31m-            size(80, 24), size(80, 24), size(80, 24), // sub-cell jitter: no new size yet
(B[m[31m-            size(81, 24),                              // crosses one column
(B[m[31m-            size(81, 24),                              // settles there briefly
(B[m[31m-            size(81, 25),                              // crosses one row too - the final size
(B[m[32m+            size(80, 24),
(B[m[32m+            size(80, 24),
(B[m[32m+            size(80, 24), // sub-cell jitter: no new size yet
(B[m[32m+            size(81, 24), // crosses one column
(B[m[32m+            size(81, 24), // settles there briefly
(B[m[32m+            size(81, 25), // crosses one row too - the final size
(B[m         ];
         let sent = coalesce(size(80, 24), drag);
         assert_eq!(sent, vec![size(81, 24), size(81, 25)]);
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 147 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-17T00:15:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at f377b58d5d92 with uncommitted changes, last 40 of 65 lines:

```text
    Checking tokio-rustls v0.26.6
    Checking hyper-rustls v0.27.10
    Checking zed-reqwest v0.12.15-zed
   Compiling toml v0.8.23
   Compiling half v2.7.1
    Checking gpui_http_client v0.2.2
    Checking tiff v0.11.3
    Checking exr v1.74.3
    Checking ravif v0.13.0
    Checking image v0.25.10
    Checking blade-graphics v0.7.1
    Checking blade-util v0.3.0
   Compiling gpui v0.2.2
    Checking steg-ui v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121/crates/ui)
    Checking steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121/crates/app)
error: this `if` statement can be collapsed
  --> crates/app/src/keys.rs:50:5
   |
50 | /     if modifiers.control {
51 | |         if let Some(byte) = control_byte(key) {
52 | |             return Some(vec![byte]);
53 | |         }
54 | |     }
   | |_____^
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#collapsible_if
   = note: `-D clippy::collapsible-if` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::collapsible_if)]`
help: collapse nested if block
   |
50 ~     if modifiers.control
51 ~         && let Some(byte) = control_byte(key) {
52 |             return Some(vec![byte]);
53 ~         }
   |

error: could not compile `steg-app` (bin "steg-app" test) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T00:19:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at f377b58d5d92 with uncommitted changes:

```text
ok      format check (0.3s)
ok      lint (1.9s)
ok      boundaries (0.2s)
ok      foundations (0.2s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (-p steg-app) (20.1s)
ok      build (steg-sdk) (3.5s)
ok      test (steg-sdk) (1.9s)
ok      test (steg-sdk, local-socket) (2.4s)
ok      build (steg-kernel) (45.6s)
ok      test (steg-kernel) (1.5s)
ok      build (steg-server) (44.6s)
ok      test (steg-server) (22.1s)
ok      test (steg-server, race-fixture) (20.8s)
ok      test (steg-server, accept-backoff-fixture) (23.5s)
ok      build (steg) (2.1s)
ok      test (steg) (0.4s)
ok      test (steg, fixtures) (4.7s)
ok      build (steg-ui) (1.0s)
ok      test (steg-ui) (1.2s)
ok      build (steg-app) (0.6s)
ok      test (steg-app) (0.3s)
ok      tool tests (12.6s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `just test` exited 101 at 2026-11-17T00:20:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 145b60b6bc33 with uncommitted changes, last 40 of 52 lines:

```text
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-84c5e5a9e7a26d91)

running 15 tests
test ime::tests::marking_empty_text_closes_the_composition ... ok
test ime::tests::a_multi_line_commit_sends_carriage_returns_like_enter_does ... ok
test ime::tests::marking_text_opens_the_composition_and_sends_nothing ... ok
test ime::tests::committing_a_resolved_composition_sends_its_own_bytes_and_closes_it ... ok
test ime::tests::unmarking_closes_the_composition ... ok
test ime::tests::committing_with_no_composition_open_still_sends_the_text ... ok
test keys::tests::the_table_covers_every_named_key_this_module_maps ... ok
test size::tests::a_changed_computed_size_declares_itself ... ok
test size::tests::a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one ... ok
test size::tests::a_viewport_many_cells_wide_and_tall_divides_evenly ... ok
test size::tests::a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero ... ok
test size::tests::a_viewport_that_does_not_divide_evenly_floors_the_remainder ... ok
test size::tests::an_unchanged_computed_size_declares_nothing ... ok
test size::tests::a_zero_cell_width_is_a_caller_bug_not_a_zero_grid - should panic ... ok
test keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says ... FAILED

failures:

---- keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says stdout ----

thread 'keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says' (45979149) panicked at crates/app/src/keys.rs:427:13:
assertion `left == right` failed: row "shift-tab": key="tab" key_char=None modifiers=Modifiers { control: false, alt: false, shift: true, platform: false, function: false }
  left: Some([9])
 right: Some([27, 91, 90])
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says

test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
full output: .quiet/tests-p-steg-app.log
error: recipe `test` failed with exit code 101
```

Check passed: `just test` exited 0 at 2026-11-17T00:20:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 145b60b6bc33:

```text
ok      tests (-p steg-app) (0.9s)
```

Check FAILED: `just test` exited 101 at 2026-11-17T00:20:41+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 145b60b6bc33 with uncommitted changes, last 40 of 42 lines:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.60s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-84c5e5a9e7a26d91)

running 15 tests
test ime::tests::unmarking_closes_the_composition ... ok
test size::tests::a_changed_computed_size_declares_itself ... ok
test ime::tests::marking_empty_text_closes_the_composition ... ok
test keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says ... ok
test ime::tests::marking_text_opens_the_composition_and_sends_nothing ... ok
test ime::tests::committing_a_resolved_composition_sends_its_own_bytes_and_closes_it ... ok
test ime::tests::committing_with_no_composition_open_still_sends_the_text ... ok
test ime::tests::a_multi_line_commit_sends_carriage_returns_like_enter_does ... ok
test size::tests::a_viewport_that_does_not_divide_evenly_floors_the_remainder ... ok
test size::tests::a_viewport_many_cells_wide_and_tall_divides_evenly ... ok
test size::tests::a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one ... ok
test keys::tests::the_table_covers_every_named_key_this_module_maps ... ok
test size::tests::an_unchanged_computed_size_declares_nothing ... ok
test size::tests::a_zero_cell_width_is_a_caller_bug_not_a_zero_grid - should panic ... ok
test size::tests::a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero ... FAILED

failures:

---- size::tests::a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero stdout ----

thread 'size::tests::a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero' (45989810) panicked at crates/app/src/size.rs:96:9:
assertion `left == right` failed
  left: GridSize { columns: 0, rows: 0 }
 right: GridSize { columns: 1, rows: 1 }


failures:
    size::tests::a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero

test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
full output: .quiet/tests-p-steg-app.log
error: recipe `test` failed with exit code 101
```

Check passed: `just test` exited 0 at 2026-11-17T00:20:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 145b60b6bc33:

```text
ok      tests (-p steg-app) (0.9s)
```

Check FAILED: `just test` exited 101 at 2026-11-17T00:21:07+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 145b60b6bc33 with uncommitted changes, last 40 of 51 lines:

```text
test ime::tests::a_multi_line_commit_sends_carriage_returns_like_enter_does ... ok
test ime::tests::committing_with_no_composition_open_still_sends_the_text ... ok
test ime::tests::committing_a_resolved_composition_sends_its_own_bytes_and_closes_it ... ok
test size::tests::a_viewport_that_does_not_divide_evenly_floors_the_remainder ... ok
test ime::tests::unmarking_closes_the_composition ... ok
test size::tests::a_viewport_many_cells_wide_and_tall_divides_evenly ... ok
test size::tests::a_changed_computed_size_declares_itself ... ok
test size::tests::a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero ... ok
test keys::tests::the_table_covers_every_named_key_this_module_maps ... ok
test size::tests::a_zero_cell_width_is_a_caller_bug_not_a_zero_grid - should panic ... ok
test size::tests::a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one ... FAILED
test size::tests::an_unchanged_computed_size_declares_nothing ... FAILED

failures:

---- size::tests::a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one stdout ----

thread 'size::tests::a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one' (45999435) panicked at crates/app/src/size.rs:129:9:
assertion `left == right` failed
  left: [GridSize { columns: 80, rows: 24 }, GridSize { columns: 80, rows: 24 }, GridSize { columns: 80, rows: 24 }, GridSize { columns: 81, rows: 24 }, GridSize { columns: 81, rows: 24 }, GridSize { columns: 81, rows: 25 }]
 right: [GridSize { columns: 81, rows: 24 }, GridSize { columns: 81, rows: 25 }]

---- size::tests::an_unchanged_computed_size_declares_nothing stdout ----

thread 'size::tests::an_unchanged_computed_size_declares_nothing' (45999440) panicked at crates/app/src/size.rs:103:9:
assertion `left == right` failed
  left: Some(GridSize { columns: 80, rows: 24 })
 right: None
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    size::tests::a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one
    size::tests::an_unchanged_computed_size_declares_nothing

test result: FAILED. 13 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
full output: .quiet/tests-p-steg-app.log
error: recipe `test` failed with exit code 101
```

Check passed: `just test` exited 0 at 2026-11-17T00:21:18+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 145b60b6bc33:

```text
ok      tests (-p steg-app) (0.8s)
```

Check FAILED: `just test` exited 101 at 2026-11-17T00:21:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 145b60b6bc33 with uncommitted changes, last 40 of 41 lines:

```text
   Compiling steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121/crates/app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.62s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-84c5e5a9e7a26d91)

running 15 tests
test ime::tests::marking_text_opens_the_composition_and_sends_nothing ... ok
test keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says ... ok
test ime::tests::committing_with_no_composition_open_still_sends_the_text ... ok
test ime::tests::a_multi_line_commit_sends_carriage_returns_like_enter_does ... ok
test ime::tests::marking_empty_text_closes_the_composition ... ok
test ime::tests::unmarking_closes_the_composition ... ok
test size::tests::a_changed_computed_size_declares_itself ... ok
test keys::tests::the_table_covers_every_named_key_this_module_maps ... ok
test size::tests::a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one ... ok
test size::tests::a_viewport_many_cells_wide_and_tall_divides_evenly ... ok
test size::tests::a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero ... ok
test size::tests::a_viewport_that_does_not_divide_evenly_floors_the_remainder ... ok
test size::tests::an_unchanged_computed_size_declares_nothing ... ok
test ime::tests::committing_a_resolved_composition_sends_its_own_bytes_and_closes_it ... FAILED
test size::tests::a_zero_cell_width_is_a_caller_bug_not_a_zero_grid - should panic ... ok

failures:

---- ime::tests::committing_a_resolved_composition_sends_its_own_bytes_and_closes_it stdout ----

thread 'ime::tests::committing_a_resolved_composition_sends_its_own_bytes_and_closes_it' (46006094) panicked at crates/app/src/ime.rs:96:9:
assertion failed: !state.is_composing()
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    ime::tests::committing_a_resolved_composition_sends_its_own_bytes_and_closes_it

test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
full output: .quiet/tests-p-steg-app.log
error: recipe `test` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T00:23:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 145b60b6bc33:

```text
ok      format check (0.2s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.6s)
ok      tests (-p steg-app) (0.8s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.3s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (18.4s)
ok      test (steg-server, race-fixture) (18.4s)
ok      test (steg-server, accept-backoff-fixture) (19.0s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (3.3s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (11.7s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-17T00:31:43+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 05dd9808cb59 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (2.1s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.6s)
ok      tests (every crate) (59.4s)
ok      build (steg-sdk) (1.0s)
ok      test (steg-sdk) (2.3s)
ok      test (steg-sdk, local-socket) (2.6s)
ok      build (steg-kernel) (1.3s)
ok      test (steg-kernel) (1.4s)
ok      build (steg-server) (1.3s)
ok      test (steg-server) (25.9s)
ok      test (steg-server, race-fixture) (27.0s)
ok      test (steg-server, accept-backoff-fixture) (27.7s)
ok      build (steg) (2.4s)
ok      test (steg) (0.6s)
ok      test (steg, fixtures) (5.6s)
ok      build (steg-ui) (0.4s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (1.4s)
ok      test (steg-app) (1.4s)
ok      tool tests (11.8s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `just check` exited 101 at 2026-11-17T00:40:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 724fe9aca809 with uncommitted changes:

```text
ok      format check (0.3s)
FAILED  lint (exit 101, 0.8s)
    Blocking waiting for file lock on package cache
    Checking steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121/crates/app)
error: large size difference between variants
  --> crates/app/src/main.rs:56:1
   |
56 | / enum Phase {
57 | |     Connecting,
58 | |     Attached(AttachedState),
   | |     ----------------------- the largest variant contains at least 304 bytes
59 | |     Disconnected(String),
   | |     -------------------- the second-largest variant contains at least 24 bytes
60 | | }
   | |_^ the entire enum is at least 304 bytes
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#large_enum_variant
   = note: `-D clippy::large-enum-variant` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::large_enum_variant)]`
help: consider boxing the large fields or introducing indirection in some other way to reduce the total size of the enum
   |
58 -     Attached(AttachedState),
58 +     Attached(Box<AttachedState>),
   |

error: could not compile `steg-app` (bin "steg-app") due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg-app` (bin "steg-app" test) due to 1 previous error
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-17T00:07:00Z

---

Plan approved by @bruk (344be7f33227, size-2): its worker may build it.

---

created: 2026-11-17T00:32:00Z

---

Merged origin/task-1.117-app-a-bare-window-with-one-terminal-you (clean) and origin/task-1.114-server-input-and-a-declared-size-from-an (two conflicts) into this branch.

Conflicts: crates/server/src/connection.rs (pane_streams's key type - took 1.114's side, the AttachmentId fix per the orchestrator's note) and crates/server/src/listen.rs (a test-only Duration literal for the now-shared attach_tick parameter - kept the named DEFAULT_ATTACH_TICK constant, incidental to that test's own assertion either way).

Model (crates/app/src/model.rs) departs from the plan's section 4 as written, because 1.117's own model changed shape before this merge: it now tracks only a row count (columns are cropped by pixel bounds, not counted), not the full (columns, rows) the plan's prose assumed. Updated it to read that row count from Screen.size (now real, from TASK-1.114) instead of rows.len(), and to update it from a Full resync's own size. Renamed/replaced the two tests the plan named as falsified, under their real current names: from_snapshot_reads_its_fixed_row_count_once -> from_snapshot_reads_its_row_count_from_the_snapshots_own_size; a_full_update_replaces_the_whole_grid_but_not_the_fixed_row_count_it_was_built_with -> a_full_update_replaces_the_whole_grid_and_its_own_reported_size (now asserts the size changes, the opposite of its old name). a_delta_touches_only_the_rows_it_names_... is left asserting "unchanged" for now and renamed to say so plainly, because ScreenUpdate::Delta carries no size field on the branch this was merged against yet - TASK-1.114's own section 2 adds one; this is the known, named gap to close at the next re-merge, not a decision that Delta should stay size-blind.

just check: exit 0 after the merge and this fix.

---
<!-- COMMENTS:END -->
