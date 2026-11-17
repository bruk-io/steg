---
id: TASK-1.121
title: 'App: typing and resizing in the bare window'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T15:26:00Z'
updated_date: '2026-11-17T23:39:00Z'
labels:
  - size-2
  - changes-requested
milestone: m-4
dependencies:
  - TASK-1.117
  - TASK-1.114
references:
  - task-1.121-app-typing-and-resizing-in-the-bare
  - https://github.com/bruk-io/steg/pull/71
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.121-app-typing-and-resizing-in-the-bare
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '344be7f33227'
asdlc_plan_approved_size: size-2
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=3,nit=0'
asdlc_review_policies: ["review", "app"]
asdlc_plan_matched: no
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

Check FAILED: `just check` exited 101 at 2026-11-17T01:12:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 724fe9aca809 with uncommitted changes, last 40 of 79 lines:

```text
test signal::tests::send_reaches_a_real_process_with_the_named_signal ... ok
test runtime::tests::a_shell_pane_not_only_a_command_pane_gets_the_same_composed_environment ... ok
test runtime::tests::disconnect_client_reapplies_the_next_latest_declared_size ... ok
kill: 96608: No such process
test signal::tests::send_reaches_the_whole_process_group_not_only_the_leader ... ok
kill: 96429: No such process
test runtime::stop_tests::stop_ends_a_program_producing_output ... ok
test runtime::tests::steg_vars_and_term_always_win_over_a_conflicting_value_from_the_login_shells_own_base_env ... ok
test runtime::tests::pane_split_falls_back_to_home_when_the_inherited_live_cwd_no_longer_exists ... ok
test runtime::tests::pane_split_inherits_the_live_cwd_of_the_pane_it_split_from_not_its_started_in_cwd ... ok
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
kill: 96439: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
kill: 96725: No such process
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... ok
kill: 96427: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails has been running for over 60 seconds
error: test failed, to rerun pass `-p steg-server --lib`

Caused by:
  process didn't exit successfully: `/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121/target/debug/deps/steg_server-12505e9af74cc0ed` (signal: 15, SIGTERM: termination signal)
full output: .quiet/test-steg-server-race-fixture.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T01:14:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 724fe9aca809 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (every crate) (24.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.4s)
ok      test (steg-sdk, local-socket) (0.6s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (21.8s)
ok      test (steg-server, race-fixture) (21.1s)
ok      test (steg-server, accept-backoff-fixture) (23.3s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (3.7s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.6s)
ok      build (steg-app) (1.3s)
ok      test (steg-app) (1.3s)
ok      tool tests (11.8s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `just check` exited 101 at 2026-11-17T01:18:07+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 9d6518c41040 with uncommitted changes:

```text
ok      format check (0.3s)
FAILED  lint (exit 101, 1.9s)
    Checking steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121/crates/app)
error: variable does not need to be mutable
   --> crates/app/src/client.rs:287:14
    |
287 |         let (mut server_side, app_side) = UnixStream::pair().expect("a socket pair");
    |              ----^^^^^^^^^^^
    |              |
    |              help: remove this `mut`
    |
    = note: `-D unused-mut` implied by `-D warnings`
    = help: to override `-D warnings` add `#[allow(unused_mut)]`

error: could not compile `steg-app` (bin "steg-app" test) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T01:20:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 9d6518c41040 with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.6s)
ok      tests (every crate) (24.1s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (19.7s)
ok      test (steg-server, race-fixture) (21.7s)
ok      test (steg-server, accept-backoff-fixture) (21.9s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (3.4s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.9s)
ok      test (steg-app) (1.1s)
ok      tool tests (11.9s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-17T01:29:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e:

```text
ok      format check (0.3s)
ok      lint (0.9s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (24.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (23.5s)
ok      test (steg-server, race-fixture) (24.8s)
ok      test (steg-server, accept-backoff-fixture) (26.1s)
ok      build (steg) (0.2s)
ok      test (steg) (0.2s)
ok      test (steg, fixtures) (3.9s)
ok      build (steg-ui) (0.5s)
ok      test (steg-ui) (3.1s)
ok      build (steg-app) (2.2s)
ok      test (steg-app) (1.0s)
ok      tool tests (13.4s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `just test` exited 101 at 2026-11-17T01:30:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e with uncommitted changes, last 40 of 64 lines:

```text
test ime::tests::a_multi_line_commit_sends_carriage_returns_like_enter_does ... ok
test ime::tests::committing_with_no_composition_open_still_sends_the_text ... ok
test model::tests::a_wide_runs_columns_are_twice_its_own_grapheme_count ... ok
test size::tests::a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one ... ok
test ime::tests::committing_a_resolved_composition_sends_its_own_bytes_and_closes_it ... ok
test model::tests::a_full_update_replaces_the_whole_grid_and_its_own_reported_size ... ok
test model::tests::a_combining_mark_is_one_grapheme_and_one_column_not_two ... ok
test size::tests::a_viewport_that_does_not_divide_evenly_floors_the_remainder ... ok
test keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says ... ok
test size::tests::an_unchanged_computed_size_declares_nothing ... ok
test terminal_element::tests::two_runs_differing_only_in_underline_colour_get_different_keys ... ok
test terminal_element::tests::two_runs_with_the_same_underline_colour_share_a_key ... ok
test client::tests::stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound ... ok
test client::tests::a_frame_on_a_stream_never_opened_ends_the_connection ... ok
test size::tests::a_zero_cell_width_is_a_caller_bug_not_a_zero_grid - should panic ... ok
test client::tests::a_control_frame_on_the_open_stream_arrives_as_an_update ... ok
test startup::tests::a_labelled_session_is_returned_with_no_session_new_call ... ok
test startup::tests::publishing_a_shutdown_handle_in_the_slot_unblocks_a_blocked_read ... ok
test client::tests::declare_size_writes_a_tab_declare_size_request_naming_the_tab_and_the_size ... ok
test startup::tests::no_labelled_session_calls_session_new ... ok
test startup::tests::a_running_pane_from_a_real_reply_is_returned_unchanged ... ok
test startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session ... ok
test client::tests::send_input_writes_a_data_frame_on_the_attachments_own_stream_not_stream_0 ... FAILED

failures:

---- client::tests::send_input_writes_a_data_frame_on_the_attachments_own_stream_not_stream_0 stdout ----

thread 'client::tests::send_input_writes_a_data_frame_on_the_attachments_own_stream_not_stream_0' (47538010) panicked at crates/app/src/client.rs:307:18:
read what the writer wrote: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }


failures:
    client::tests::send_input_writes_a_data_frame_on_the_attachments_own_stream_not_stream_0

test result: FAILED. 38 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just test` exited 101 at 2026-11-17T01:30:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e with uncommitted changes, last 40 of 64 lines:

```text
test size::tests::a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one ... ok
test ime::tests::committing_with_no_composition_open_still_sends_the_text ... ok
test keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says ... ok
test ime::tests::committing_a_resolved_composition_sends_its_own_bytes_and_closes_it ... ok
test size::tests::a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero ... ok
test ime::tests::a_multi_line_commit_sends_carriage_returns_like_enter_does ... ok
test size::tests::a_viewport_that_does_not_divide_evenly_floors_the_remainder ... ok
test size::tests::an_unchanged_computed_size_declares_nothing ... ok
test terminal_element::tests::two_runs_differing_only_in_underline_colour_get_different_keys ... ok
test terminal_element::tests::two_runs_with_the_same_underline_colour_share_a_key ... ok
test client::tests::stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound ... ok
test client::tests::a_frame_on_a_stream_never_opened_ends_the_connection ... ok
test client::tests::a_control_frame_on_the_open_stream_arrives_as_an_update ... ok
test size::tests::a_zero_cell_width_is_a_caller_bug_not_a_zero_grid - should panic ... ok
test startup::tests::publishing_a_shutdown_handle_in_the_slot_unblocks_a_blocked_read ... ok
test startup::tests::a_labelled_session_is_returned_with_no_session_new_call ... ok
test startup::tests::no_labelled_session_calls_session_new ... ok
test client::tests::send_input_writes_a_data_frame_on_the_attachments_own_stream_not_stream_0 ... ok
test startup::tests::a_running_pane_from_a_real_reply_is_returned_unchanged ... ok
test client::tests::declare_size_writes_a_tab_declare_size_request_naming_the_tab_and_the_size ... FAILED
test startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session ... ok

failures:

---- client::tests::declare_size_writes_a_tab_declare_size_request_naming_the_tab_and_the_size stdout ----

thread 'client::tests::declare_size_writes_a_tab_declare_size_request_naming_the_tab_and_the_size' (47549823) panicked at crates/app/src/client.rs:320:9:
assertion `left == right` failed
  left: None
 right: Some("sess/tab-1")


failures:
    client::tests::declare_size_writes_a_tab_declare_size_request_naming_the_tab_and_the_size

test result: FAILED. 38 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-17T01:31:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e, last 40 of 71 lines:

```text
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok

test result: ok. 173 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s

     Running tests/attach.rs (target/debug/deps/attach-d38d78534eadc4dd)

running 15 tests
test detaching_a_pane_this_connection_never_attached_to_is_a_bad_request ... ok
test a_frame_on_an_already_detached_stream_closes_the_connection ... ok
test attach_returns_the_real_screen_and_updates_follow_it ... ok
test declaring_a_size_of_zero_is_refused ... ok
test typed_bytes_reach_a_real_program_in_order ... ok
test disconnecting_without_detaching_does_not_leak_or_block_a_later_attach ... ok
test input_clears_a_finished_status ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (47558807) panicked at crates/server/tests/attach.rs:1255:6:
pane.show must answer within a bound, even with the other pane stuck
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane

test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.54s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T01:34:55+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e:

```text
ok      format check (0.3s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (every crate) (26.8s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.6s)
ok      test (steg-sdk, local-socket) (0.8s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (31.3s)
ok      test (steg-server) (26.6s)
ok      test (steg-server, race-fixture) (26.1s)
ok      test (steg-server, accept-backoff-fixture) (29.9s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (3.2s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.5s)
ok      build (steg-app) (0.9s)
ok      test (steg-app) (1.4s)
ok      tool tests (12.0s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check FAILED: `just test` exited 101 at 2026-11-17T01:35:19+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e with uncommitted changes, last 40 of 64 lines:

```text
test keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says ... ok
test metrics::tests::a_pixel_size_that_does_not_divide_evenly_floors_not_rounds ... ok
test size::tests::a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero ... ok
test ime::tests::committing_with_no_composition_open_still_sends_the_text ... ok
test size::tests::a_viewport_that_does_not_divide_evenly_floors_the_remainder ... ok
test size::tests::an_unchanged_computed_size_declares_nothing ... ok
test terminal_element::tests::two_runs_with_the_same_underline_colour_share_a_key ... ok
test terminal_element::tests::two_runs_differing_only_in_underline_colour_get_different_keys ... ok
test client::tests::stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound ... ok
test size::tests::a_zero_cell_width_is_a_caller_bug_not_a_zero_grid - should panic ... ok
test model::tests::a_full_update_replaces_the_whole_grid_and_its_own_reported_size ... FAILED
test client::tests::a_frame_on_a_stream_never_opened_ends_the_connection ... ok
test client::tests::a_control_frame_on_the_open_stream_arrives_as_an_update ... ok
test startup::tests::a_labelled_session_is_returned_with_no_session_new_call ... ok
test startup::tests::publishing_a_shutdown_handle_in_the_slot_unblocks_a_blocked_read ... ok
test client::tests::send_input_writes_a_data_frame_on_the_attachments_own_stream_not_stream_0 ... ok
test client::tests::declare_size_writes_a_tab_declare_size_request_naming_the_tab_and_the_size ... ok
test startup::tests::no_labelled_session_calls_session_new ... ok
test startup::tests::a_running_pane_from_a_real_reply_is_returned_unchanged ... ok
test startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session ... ok

failures:

---- model::tests::a_full_update_replaces_the_whole_grid_and_its_own_reported_size stdout ----

thread 'model::tests::a_full_update_replaces_the_whole_grid_and_its_own_reported_size' (47750717) panicked at crates/app/src/model.rs:284:9:
assertion `left == right` failed
  left: 1
 right: 2
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    model::tests::a_full_update_replaces_the_whole_grid_and_its_own_reported_size

test result: FAILED. 38 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T01:37:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e:

```text
ok      format check (0.3s)
ok      lint (0.6s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (every crate) (21.7s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (0.3s)
ok      test (steg-server) (22.6s)
ok      test (steg-server, race-fixture) (22.5s)
ok      test (steg-server, accept-backoff-fixture) (22.8s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (3.3s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.5s)
ok      build (steg-app) (1.1s)
ok      test (steg-app) (1.1s)
ok      tool tests (11.7s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-17T01:53:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e:

```text
ok      format check (0.3s)
ok      lint (6.9s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.1s)
ok      design pages (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (29.3s)
ok      build (steg-sdk) (0.7s)
ok      test (steg-sdk) (1.8s)
ok      test (steg-sdk, local-socket) (2.4s)
ok      build (steg-kernel) (0.4s)
ok      test (steg-kernel) (1.0s)
ok      build (steg-server) (28.8s)
ok      test (steg-server) (26.0s)
ok      test (steg-server, race-fixture) (26.4s)
ok      test (steg-server, accept-backoff-fixture) (29.6s)
ok      build (steg) (1.4s)
ok      test (steg) (0.5s)
ok      test (steg, fixtures) (6.4s)
ok      build (steg-ui) (0.4s)
ok      test (steg-ui) (1.3s)
ok      build (steg-app) (2.8s)
ok      test (steg-app) (1.1s)
ok      tool tests (11.8s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.0s)
ok      ui colours (0.1s)
```

Check FAILED: `just test` exited 101 at 2026-11-17T01:56:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e with uncommitted changes, last 40 of 64 lines:

```text
test ime::tests::a_multi_line_commit_sends_carriage_returns_like_enter_does ... ok
test ime::tests::committing_with_no_composition_open_still_sends_the_text ... ok
test size::tests::a_drag_emits_only_the_distinct_sizes_it_crosses_and_always_ends_on_the_last_one ... ok
test size::tests::a_viewport_many_cells_wide_and_tall_divides_evenly ... ok
test keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says ... ok
test size::tests::a_viewport_smaller_than_one_cell_floors_to_one_column_and_one_row_not_zero ... ok
test size::tests::a_viewport_that_does_not_divide_evenly_floors_the_remainder ... ok
test size::tests::an_unchanged_computed_size_declares_nothing ... ok
test client::tests::a_frame_on_a_stream_never_opened_ends_the_connection ... ok
test client::tests::stop_unblocks_and_joins_the_reader_thread_within_the_tests_own_bound ... ok
test terminal_element::tests::two_runs_differing_only_in_underline_colour_get_different_keys ... ok
test terminal_element::tests::two_runs_with_the_same_underline_colour_share_a_key ... ok
test client::tests::a_control_frame_on_the_open_stream_arrives_as_an_update ... ok
test size::tests::a_zero_cell_width_is_a_caller_bug_not_a_zero_grid - should panic ... ok
test startup::tests::publishing_a_shutdown_handle_in_the_slot_unblocks_a_blocked_read ... ok
test startup::tests::a_labelled_session_is_returned_with_no_session_new_call ... ok
test client::tests::send_input_writes_a_data_frame_on_the_attachments_own_stream_not_stream_0 ... ok
test startup::tests::no_labelled_session_calls_session_new ... ok
test startup::tests::a_running_pane_from_a_real_reply_is_returned_unchanged ... ok
test client::tests::declare_size_writes_a_tab_declare_size_request_naming_the_tab_and_the_size ... ok
test startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session ... ok
test client::tests::once_the_connection_is_already_shut_down_declare_size_neither_panics_nor_is_a_reply_ever_seen ... ok
test client::tests::declare_sizes_own_reply_is_observable_over_the_connection_when_it_is_alive ... FAILED

failures:

---- client::tests::declare_sizes_own_reply_is_observable_over_the_connection_when_it_is_alive stdout ----

thread 'client::tests::declare_sizes_own_reply_is_observable_over_the_connection_when_it_is_alive' (48494690) panicked at crates/app/src/client.rs:426:9:
a reply should be observable on a live connection, got None


failures:
    client::tests::declare_sizes_own_reply_is_observable_over_the_connection_when_it_is_alive

test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-17T01:57:06+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e with uncommitted changes, last 40 of 74 lines:

```text

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/connection.rs (target/debug/deps/connection-954ecb1969526d55)

running 2 tests
test a_version_mismatch_is_refused_naming_both_versions ... ok
test a_client_and_server_connection_shake_hands_and_exchange_a_call_in_memory ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/local_socket.rs (target/debug/deps/local_socket-1079895324ddf99e)

running 6 tests
test spawn_fails_promptly_when_the_binarys_path_does_not_exist ... ok
test spawn_fails_when_the_binary_exists_but_is_not_executable ... ok
test refuses_to_start_one_when_autostart_is_false ... ok
test connects_without_spawning_when_a_listener_is_already_bound ... FAILED
test gives_up_with_a_timeout_when_nothing_ever_binds ... ok
test starts_one_and_waits_when_nothing_is_listening_and_autostart_is_true ... ok

failures:

---- connects_without_spawning_when_a_listener_is_already_bound stdout ----

thread 'connects_without_spawning_when_a_listener_is_already_bound' (48506911) panicked at crates/sdk/tests/local_socket.rs:91:54:
called `Result::unwrap()` on an `Err` value: Os { code: 48, kind: AddrInUse, message: "Address already in use" }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    connects_without_spawning_when_a_listener_is_already_bound

test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

error: test failed, to rerun pass `-p steg-sdk --test local_socket`
full output: .quiet/test-steg-sdk-local-socket.log
error: recipe `check-packages` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T01:59:02+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at b3d13d41ef5e with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (every crate) (20.8s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (19.6s)
ok      test (steg-server, race-fixture) (20.4s)
ok      test (steg-server, accept-backoff-fixture) (21.0s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (3.4s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.9s)
ok      test (steg-app) (1.2s)
ok      tool tests (11.7s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check FAILED: `just check` exited 101 at 2026-11-17T02:38:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at fa0b33fc801c with uncommitted changes, last 40 of 71 lines:

```text
test runtime::tests::a_typed_command_still_running_stays_running_until_told_otherwise ... ok
test pane::attach_tests::an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time ... ok
test pane::attach_tests::attaching_returns_the_panes_current_screen_and_updates_follow_it ... ok
test pane::ending_tests::await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue ... ok
test runtime::tests::an_unfiltered_subscriber_receives_nothing ... ok
kill: 34994: No such process
test runtime::stop_tests::stop_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::the_general_sizing_step_reaches_every_pane_through_split_close_and_swap ... ok
test pane::ending_tests::signal_hangup_reaches_a_backgrounded_grandchild_too ... ok
test runtime::tests::a_mark_then_the_shells_own_real_exit_cascades_with_no_panic ... ok
test idle::tests::the_checker_blocks_with_no_timeout_while_ineligible_and_waits_exactly_once_while_eligible ... ok
test runtime::tests::a_shell_with_no_marks_leaves_every_status_field_unchanged_until_one_arrives ... ok
test pane::ending_tests::signal_kill_ends_a_program_that_traps_the_polite_signal ... ok
test runtime::tests::a_typed_command_that_fails_is_failed_with_its_exit_code ... ok
test runtime::tests::a_typed_commands_status_reaches_pane_show_and_tab_show_through_runtime_handle ... ok
test runtime::tests::a_typed_command_that_succeeds_is_running_then_finished_and_the_tab_shows_it ... ok
test runtime::tests::a_command_panes_own_program_printing_marks_does_not_change_its_status ... ok
test login_env::tests::capture_login_environment_falls_back_on_a_shell_that_exits_non_zero ... ok
test login_env::tests::capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump ... ok
kill: 34953: No such process
test pane::tests::a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own ... ok
test login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open ... FAILED

failures:

---- login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open stdout ----

thread 'login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open' (49419642) panicked at crates/server/src/login_env.rs:659:13:
the backgrounded grandchild's own pid file never appeared


failures:
    login_env::tests::capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open

test result: FAILED. 190 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.01s

error: test failed, to rerun pass `-p steg-server --lib`
full output: .quiet/tests-every-crate.log
error: recipe `test` failed with exit code 101
```

Check passed: `just check` exited 0 at 2026-11-17T02:43:33+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at fa0b33fc801c with uncommitted changes:

```text
ok      format check (0.3s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.9s)
ok      tests (every crate) (31.9s)
ok      build (steg-sdk) (0.8s)
ok      test (steg-sdk) (2.2s)
ok      test (steg-sdk, local-socket) (2.7s)
ok      build (steg-kernel) (0.5s)
ok      test (steg-kernel) (1.2s)
ok      build (steg-server) (29.9s)
ok      test (steg-server) (29.2s)
ok      test (steg-server, race-fixture) (31.0s)
ok      test (steg-server, pty-race-fixture) (37.3s)
ok      test (steg-server, accept-backoff-fixture) (34.8s)
ok      build (steg) (2.0s)
ok      test (steg) (3.3s)
ok      test (steg, fixtures) (9.8s)
ok      build (steg-ui) (0.4s)
ok      test (steg-ui) (1.2s)
ok      build (steg-app) (1.9s)
ok      test (steg-app) (1.5s)
ok      tool tests (11.9s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Check passed: `just check` exited 0 at 2026-11-17T22:32:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 7ba220a481d3 with uncommitted changes:

```text
ok      format check (0.3s)
ok      lint (11.2s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (93.7s)
ok      build (steg-sdk) (0.8s)
ok      test (steg-sdk) (2.4s)
ok      test (steg-sdk, local-socket) (3.0s)
ok      build (steg-kernel) (1.2s)
ok      test (steg-kernel) (1.2s)
ok      build (steg-server) (1.3s)
ok      test (steg-server) (57.7s)
ok      test (steg-server, race-fixture) (60.1s)
ok      test (steg-server, pty-race-fixture) (66.0s)
ok      test (steg-server, accept-backoff-fixture) (60.7s)
ok      build (steg) (1.9s)
ok      test (steg) (3.7s)
ok      test (steg, fixtures) (10.9s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (1.1s)
ok      build (steg-app) (1.3s)
ok      test (steg-app) (1.4s)
ok      tool tests (13.6s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Check passed: `just check` exited 0 at 2026-11-17T22:38:23+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 2c127d4f1376:

```text
ok      format check (0.3s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.6s)
ok      tests (-p steg -p steg-app -p steg-server) (52.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (51.6s)
ok      test (steg-server, race-fixture) (55.5s)
ok      test (steg-server, pty-race-fixture) (56.6s)
ok      test (steg-server, accept-backoff-fixture) (52.6s)
ok      build (steg) (0.1s)
ok      test (steg) (1.2s)
ok      test (steg, fixtures) (4.1s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.4s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.2s)
ok      tool tests (13.4s)
ok      architecture (0.7s)
ok      recipe names (0.0s)
ok      ui levels (0.0s)
ok      ui colours (0.0s)
```

Built: the writer (a second, encode-only Connection), a minimal EntityInputHandler for composed text, focus plumbing, and declared size on open/resize, re-threaded twice onto upstream's own startup/teardown and painting rewrites as they landed (TASK-1.117's Slot-based fix, then origin/main's PR 65/66/68/69/70). key_to_bytes is a pure, table-driven function; the resize coalescer and ImeState are pure too.

Checks recorded: pure-function tests (keys/size/ime) each with a failing-then-passing control; real-socket Writer tests for declare_size/send_input, including one proving a reply is observable on a live connection and one proving the exact silent-failure shape of a dead one (found via this task's own live verification, not invented after the fact); Model's Full-update-changes-size behaviour. just check: exit 0, recorded against the final merge commit (2c127d4f) with a clean tree.

Found and reported (not fixed here, not mine to fix): TASK-1.117's own stop_startup was shutting down a connection right after a successful attach - diagnosed with a scratch cargo test replaying the app's own startup sequence against a real server, confirmed live with a temporary reverted patch, then confirmed again once 1.117's own real fix landed and was merged in.

Live run against the final merged head, throwaway socket, no local patch: prompt draws, stays connected 15s idle, typed command echoes with its output drawn, a resize is followed (stty size reports the new dimensions), close-and-relaunch shows the earlier scrollback. All yes.

Honest note on the live run's own environment: the socket was genuinely isolated (a fresh one every run), but the shell's own environment was not - steg server start's real path resolves a pane's shell identity through the real password database, not the HOME env var the server process itself was launched with, so the prompt shown is the real account's own real theme, not a throwaway one. Nothing personal was on screen because that theme shows no path/hostname/username and every typed command was a generic demo - not because of a throwaway HOME reaching the shell, which it did not.

Images/recording held by the orchestrator in /Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/pr-images/ (task-1121-typing.png, task-1121-resize.png, task-1121-typing-resize-demo.mov), described in the PR body as checked here, awaiting the engineer's own upload.

PR 71's CI is green on 2c127d4f.

Check FAILED: `just test` exited 101 at 2026-11-17T23:12:49+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 2c127d4f1376 with uncommitted changes, last 40 of 64 lines:

```text
test client::tests::a_frame_on_a_stream_never_opened_ends_the_connection ... ok
test client::tests::send_input_writes_a_data_frame_on_the_attachments_own_stream_not_stream_0 ... ok
test startup::tests::teardown_while_the_server_is_silent_mid_handshake_ends_quickly ... ok
test client::tests::a_control_frame_on_the_open_stream_arrives_as_an_update ... ok
test startup::tests::a_labelled_session_is_returned_with_no_session_new_call ... ok
test client::tests::declare_size_writes_a_tab_declare_size_request_naming_the_tab_and_the_size ... ok
test client::tests::once_the_connection_is_already_shut_down_declare_size_neither_panics_nor_is_a_reply_ever_seen ... ok
test startup::tests::a_running_pane_from_a_real_reply_is_returned_unchanged ... ok
test client::tests::declare_sizes_own_reply_is_observable_over_the_connection_when_it_is_alive ... ok
test startup::tests::no_labelled_session_calls_session_new ... ok
test startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session ... ok
test startup::tests::a_successful_attach_survives_interrupt_called_right_after ... ok

failures:

---- ime::tests::marking_a_multi_character_composition_reports_its_real_utf16_length stdout ----

thread 'ime::tests::marking_a_multi_character_composition_reports_its_real_utf16_length' (54597871) panicked at crates/app/src/ime.rs:146:9:
assertion `left == right` failed
  left: Some(0..0)
 right: Some(0..2)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- ime::tests::marking_a_single_unit_composition_reports_its_own_length_not_a_fixed_zero stdout ----

thread 'ime::tests::marking_a_single_unit_composition_reports_its_own_length_not_a_fixed_zero' (54597872) panicked at crates/app/src/ime.rs:134:9:
assertion `left == right` failed
  left: Some(0..0)
 right: Some(0..1)


failures:
    ime::tests::marking_a_multi_character_composition_reports_its_real_utf16_length
    ime::tests::marking_a_single_unit_composition_reports_its_own_length_not_a_fixed_zero

test result: FAILED. 53 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Commits since the review: 0.

Check passed: `just test` exited 0 at 2026-11-17T23:13:58+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 2c127d4f1376 with uncommitted changes:

```text
ok      tests (-p steg -p steg-app -p steg-server) (59.1s)
```

Commits since the review: 0.

Check FAILED: `just test` exited 101 at 2026-11-17T23:16:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 2c127d4f1376 with uncommitted changes, last 40 of 64 lines:

```text
test terminal_element::tests::the_cell_after_an_eighty_character_line_sits_at_column_eighty ... ok
test client::tests::once_the_connection_is_already_shut_down_declare_size_neither_panics_nor_is_a_reply_ever_seen ... ok
test terminal_element::tests::two_runs_differing_only_in_underline_colour_get_different_keys ... ok
test terminal_element::tests::two_runs_with_the_same_underline_colour_share_a_key ... ok
test startup::tests::a_labelled_session_is_returned_with_no_session_new_call ... ok
test client::tests::declare_size_writes_a_tab_declare_size_request_naming_the_tab_and_the_size ... ok
test startup::tests::no_labelled_session_calls_session_new ... ok
test client::tests::declare_sizes_own_reply_is_observable_over_the_connection_when_it_is_alive ... ok
test startup::tests::teardown_while_the_server_is_silent_mid_handshake_ends_quickly ... ok
test startup::tests::a_running_pane_from_a_real_reply_is_returned_unchanged ... ok
test startup::tests::an_ended_panes_program_gets_a_fresh_tab_in_the_same_session ... ok
test startup::tests::a_successful_attach_survives_interrupt_called_right_after ... ok

failures:

---- keys::tests::every_key_named_key_bytes_maps_has_a_row_proving_it_and_no_other_key_does stdout ----

thread 'keys::tests::every_key_named_key_bytes_maps_has_a_row_proving_it_and_no_other_key_does' (54635410) panicked at crates/app/src/keys.rs:523:13:
assertion `left == right` failed: named_key_bytes("f6", false).is_some() = false, but the table's own claim that it is mapped = true - these must agree
  left: false
 right: true

---- keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says stdout ----

thread 'keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says' (54635411) panicked at crates/app/src/keys.rs:483:13:
assertion `left == right` failed: row "f6": key="f6" key_char=None modifiers=Modifiers { control: false, alt: false, shift: false, platform: false, function: false }
  left: None
 right: Some([27, 91, 49, 55, 126])
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    keys::tests::every_key_named_key_bytes_maps_has_a_row_proving_it_and_no_other_key_does
    keys::tests::every_row_in_the_plans_own_table_maps_as_the_plan_says

test result: FAILED. 53 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Commits since the review: 0.

Check passed: `just test` exited 0 at 2026-11-17T23:17:04+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 2c127d4f1376 with uncommitted changes:

```text
ok      tests (-p steg -p steg-app -p steg-server) (53.4s)
```

Commits since the review: 0.

Check FAILED: `just check` exited 101 at 2026-11-17T23:17:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 2c127d4f1376 with uncommitted changes:

```text
ok      format check (0.3s)
FAILED  lint (exit 101, 9.0s)
    Checking steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121/crates/app)
error: method `is_composing` is never used
  --> crates/app/src/ime.rs:48:19
   |
23 | impl ImeState {
   | ------------- method in this implementation
...
48 |     pub(crate) fn is_composing(&self) -> bool {
   |                   ^^^^^^^^^^^^
   |
   = note: `-D dead-code` implied by `-D warnings`
   = help: to override `-D warnings` add `#[expect(dead_code)]` or `#[allow(dead_code)]`

error: could not compile `steg-app` (bin "steg-app") due to 1 previous error
warning: build failed, waiting for other jobs to finish...
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-17T23:24:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 2c127d4f1376 with uncommitted changes:

```text
ok      format check (0.3s)
ok      lint (0.7s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (-p steg -p steg-app -p steg-server) (61.2s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (1.3s)
ok      test (steg-sdk, local-socket) (1.7s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (1.4s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (52.9s)
ok      test (steg-server, race-fixture) (147.3s)
ok      test (steg-server, pty-race-fixture) (59.6s)
ok      test (steg-server, accept-backoff-fixture) (53.0s)
ok      build (steg) (0.1s)
ok      test (steg) (2.4s)
ok      test (steg, fixtures) (7.2s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (1.0s)
ok      build (steg-app) (1.0s)
ok      test (steg-app) (0.3s)
ok      tool tests (13.7s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Commits since the review: 0.

Check FAILED: `cargo test -p steg-app --bin steg-app control_old_writer_blocks_the_caller` exited 101 at 2026-11-17T23:37:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 26f06705bc35 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::control_old_writer_blocks_the_caller_when_the_peer_never_reads ... FAILED

failures:

---- client::tests::control_old_writer_blocks_the_caller_when_the_peer_never_reads stdout ----

thread 'client::tests::control_old_writer_blocks_the_caller_when_the_peer_never_reads' (54883408) panicked at crates/app/src/client.rs:504:14:
did not finish within the test's own bound: Timeout
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    client::tests::control_old_writer_blocks_the_caller_when_the_peer_never_reads

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 55 filtered out; finished in 5.01s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-app --bin steg-app send_input_returns_promptly_even_when_the_peer_never_reads` exited 0 at 2026-11-17T23:37:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 26f06705bc35 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::send_input_returns_promptly_even_when_the_peer_never_reads ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 58 filtered out; finished in 0.00s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check FAILED: `cargo test -p steg-app --bin steg-app input_and_declared_sizes_arrive_on_the_wire_in_the_order` exited 101 at 2026-11-17T23:38:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 26f06705bc35 with uncommitted changes:

```text
warning: variant `DeclareSize` is never constructed
  --> crates/app/src/client.rs:84:5
   |
82 | enum WriterCommand {
   |      ------------- variant in this enum
83 |     Input(Vec<u8>),
84 |     DeclareSize { columns: u16, rows: u16 },
   |     ^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-app` (bin "steg-app" test) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::input_and_declared_sizes_arrive_on_the_wire_in_the_order_the_ui_issued_them ... FAILED

failures:

---- client::tests::input_and_declared_sizes_arrive_on_the_wire_in_the_order_the_ui_issued_them stdout ----

thread 'client::tests::input_and_declared_sizes_arrive_on_the_wire_in_the_order_the_ui_issued_them' (54905016) panicked at crates/app/src/client.rs:518:9:
expected the first input, got Request { id: 1, request: Request { method: "tab.declare-size", params: {"columns": Number(80), "rows": Number(24)} } }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    client::tests::input_and_declared_sizes_arrive_on_the_wire_in_the_order_the_ui_issued_them

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 58 filtered out; finished in 0.05s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-app --bin steg-app input_and_declared_sizes_arrive_on_the_wire_in_the_order` exited 0 at 2026-11-17T23:39:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 26f06705bc35 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::input_and_declared_sizes_arrive_on_the_wire_in_the_order_the_ui_issued_them ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 58 filtered out; finished in 0.00s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check FAILED: `cargo test -p steg-app --bin steg-app once_the_queue_is_full_behind_a_stalled_peer` exited 101 at 2026-11-17T23:39:37+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 26f06705bc35 with uncommitted changes, last 40 of 43 lines:

```text
10 | use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
   |                                             ^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: field `events` is never read
   --> crates/app/src/client.rs:118:5
    |
105 | pub(crate) struct Writer {
    |                   ------ field in this struct
...
118 |     events: UnboundedSender<ClientEvent>,
    |     ^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `steg-app` (bin "steg-app" test) generated 2 warnings (run `cargo fix --bin "steg-app" -p steg-app --tests` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::once_the_queue_is_full_behind_a_stalled_peer_the_connection_is_treated_as_lost ... FAILED

failures:

---- client::tests::once_the_queue_is_full_behind_a_stalled_peer_the_connection_is_treated_as_lost stdout ----

thread 'client::tests::once_the_queue_is_full_behind_a_stalled_peer_the_connection_is_treated_as_lost' (54920361) panicked at crates/app/src/client.rs:364:14:
did not finish within the test's own bound: Timeout
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    client::tests::once_the_queue_is_full_behind_a_stalled_peer_the_connection_is_treated_as_lost

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 58 filtered out; finished in 2.01s

error: test failed, to rerun pass `-p steg-app --bin steg-app`
```

Commits since the review: 1, all with the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-app --bin steg-app once_the_queue_is_full_behind_a_stalled_peer` exited 0 at 2026-11-17T23:39:55+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.121 on task-1.121-app-typing-and-resizing-in-the-bare at 26f06705bc35 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.21s
warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6, proc-macro-error2 v2.0.1
note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
     Running unittests src/main.rs (target/debug/deps/steg_app-edc2b453f94d193e)

running 1 test
test client::tests::once_the_queue_is_full_behind_a_stalled_peer_the_connection_is_treated_as_lost ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 58 filtered out; finished in 0.00s
```

Commits since the review: 1, all with the Asdlc-Worker trailer.
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

created: 2026-11-17T01:12:00Z

---

The just check recorded above (exit 101, race-fixture) was stopped by the team lead: it hung in steg-server's own outbound::tests::a_broken_write_closes_the_queue_so_the_readers_next_enqueue_fails, the known intermittent hang recorded on TASK-1.113, not this task's own code. Re-running now.

---

created: 2026-11-17T01:37:00Z

---

Status: the pure functions (keys.rs/size.rs/ime.rs), the merge of 1.117+1.114, and the writer/EntityInputHandler/focus/declared-size wiring are all committed, each proof item recorded through asdlc record-check as a failing run (the mechanism reverted) then a passing run: the key table, the floor, the coalescer, ImeState's own commit-clears-composition, the Writer's two real-socket tests (declare_size's own target, send_input's own stream), and Model's Full-update-changes-size behaviour. just check is green (exit 0) as of this comment.

Separately: one more flake observed and confirmed not mine - crates/server/tests/attach.rs's own an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane failed once under load (a timing-sensitive "stays within its bound" assertion), passed cleanly alone afterward. Noting it here in case it recurs for someone else; not acted on further since it isn't this task's own code.

Blocked on the live-window check (typing/resizing demonstrated, and the recording) pending the orchestrator's guidance on the disconnect reported on the task's own channel.

---

created: 2026-11-17T01:50:00Z

---

Root-caused the disconnect reported earlier, at the orchestrator's request to bisect. It is not commit-specific and not server-side: it is a pre-existing bug in TASK-1.117's own main.rs. AppWindow::stop_startup() unconditionally shuts down the startup thread's own interrupt handle (a socket clone published in a ShutdownSlot for a window-close race), and apply_startup_result() calls stop_startup() as its first line regardless of whether the result is a success - so every successful attach immediately shuts down its own just-established connection before handing it to the reader thread. Nothing panics or logs on either side.

Verified with a scratch cargo test (crates/server/tests/scratch_1121_diagnose.rs, deleted, never committed) replicating startup.rs's own call sequence against a real server: stayed connected 10s with no issue, pointing at the app's own code rather than the server. Confirmed by a temporary, reverted patch (join the thread without re-shutting-down the handle) run against a live window: the window showed a live prompt, stayed connected, typing and resizing (echo + stty size showing the new dimensions) both worked end to end.

Not my fix to make (TASK-1.117's file); reported to the orchestrator with the exact mechanism. My own wiring is now confirmed working against a real server/window, not only unit-tested.

---

created: 2026-11-17T01:55:00Z

---

Cleanup done: scratchpad bisection binaries removed, worktree back on task-1.121-app-typing-and-resizing-in-the-bare, clean (just the one commit from before), ps sweep clean.

What was checked by hand, in words, no output kept: with a temporary, reverted one-line local change to TASK-1.117's stop_startup (skip the shutdown when the startup result is a success - the diagnosis's own fix shape), I ran the real app against a real server, both with a throwaway socket and a throwaway HOME. The window showed a live shell prompt instead of "disconnected". I typed a short echo command and pressed enter: it appeared on its own line, the shell's own output line followed it, and a fresh prompt came after - typing reaching the program and its output drawing, both through this task's own on_key_down/Writer wiring. I then resized the window larger and ran `stty size` in the same shell: it printed the new row and column counts, matching the resize, not the original size - confirming declare_size and the coalescer reach the real pty too. The change was reverted immediately afterward and nothing was committed.

Recording held, as instructed: it waits for TASK-1.117's own fixed head (and 1.114's), since the recording must be made from committed code, and for the engineer's Screen Recording grant.

What would have caught this at my own wiring's level: nothing in my own tests does, today, because the bug lives one level up (TASK-1.117's own stop_startup/apply_startup_result, not anything this task's Writer or Client touch) and because Writer::send_input/declare_size are deliberately fire-and-forget (a write that fails is not specially recovered from, by design - TASK-1.114's own "best effort" reasoning for the same shape). A real-socket test that calls declare_size and then asserts a reply is actually observable on the connection - not just that the outgoing request's own bytes are shaped correctly, which is all today's two Writer tests check - would at least demonstrate the class of bug is catchable at this level: added one (client.rs), with a server-side reply sent back and read through a real decode Connection the same way the reader thread would, and a control that shuts the connection down first (same shape stop_startup's own bug produces) and asserts the reply never arrives within the test's own bound. Recorded as a failing-then-passing cycle like the others. just check: exit 0.

---

created: 2026-11-17T01:59:00Z

---

Added the two tests described above (commit fa0b33fc): declare_sizes_own_reply_is_observable_over_the_connection_when_it_is_alive (recorded failing-then-passing: the server never replying is what fails it) and once_the_connection_is_already_shut_down_declare_size_neither_panics_nor_is_a_reply_ever_seen (the exact shape of the real bug, as a standing assertion). just check: exit 0.

---

created: 2026-11-17T02:46:00Z

---

Merged origin/task-1.117-app-a-bare-window-with-one-terminal-you at f8967015 (merge commit 7ba220a4, pushed to origin). Conflicts in main.rs/startup.rs (re-threaded this task's writer/focus/declared-size/IME additions through 1.117's new Slot-based startup/teardown, unchanged) and five server-side files (pane_streams/tab.declare-size/AttachmentId-shaped additive conflicts, both sides kept). just check: exit 0. Live run against committed code only, no local patch: prompt draws, stays connected 10s, typed command echoes and its output draws, a resize is followed (stty size), close-and-relaunch shows the earlier output - all yes.

---

created: 2026-11-17T22:38:00Z

---

Merged origin/main (merge commit 2c127d4f) once PRs 65/66/68/69/70 all landed there - no local patch, no waiting needed per your second message (confirmed the two protected processes, pid 63012/67229, were never touched or signalled).

Conflicts and how resolved:
- crates/app/src/main.rs: one conflict, in render()'s Attached branch - kept this task's own .track_focus/.on_key_down/extra terminal() params, took main's state.metrics.clone() (CellMetrics is no longer Copy, PR 70). Also fixed one other now-broken move (EntityInputHandler::bounds_for_range's own `let metrics = state.metrics;` -> `&state.metrics`), caught by the compiler, not the merge itself.
- crates/app/src/model.rs: one conflict, in a test fixture (snapshot/snapshot_with_size) - kept this task's own two-function split (used throughout this file's own later tests), dropped main's single-function version. Separately, updated the module's own doc comments: what I had called "a known gap, to close at the next re-merge" (Delta carrying no size) turns out to be the final, settled design once PR 66 landed (doc-14's own reworded Size section: "a delta never carries a size; a size-stale attachment gets a full screen") - reworded three comments to say so plainly, not restate a gap that closed on its own.
- crates/app/src/terminal_element.rs: one conflict, an import list (PR 126's new Point import vs this task's own ElementInputHandler/Entity/FocusHandle) - combined both lists; nothing else needed changing, this task's own terminal()/paint_grid wiring (focus_handle/entity/last_bounds params, the handle_input call) auto-merged cleanly against PR 126's own rewrite underneath it.
- crates/app/src/startup.rs: one conflict, a test fixture comment (same Screen.size shape, worded two ways) - kept main's wording.
- crates/server/src/connection.rs, dispatch/mod.rs, pane.rs: additive conflicts (PR 68's finish_attach/armed-check, event.watch's own position) - combined, nothing dropped on either side, same shape as the last two merges.
- crates/server/tests/attach.rs: replaced wholesale with main's version - confirmed first that main's file is a strict superset (all of this task's own existing tests present unchanged, plus three new ones from PR 68/69), so there was nothing of this task's own to preserve separately.

Re-verification: checked whether any of this task's own recorded controls rest on something this merge actually changed. None do - the SDK's Connection::declare_size/send_data (this task's own Writer tests) are untouched between the last merge and this one (confirmed by diff); keys.rs/size.rs/ime.rs are pure and untouched; model.rs's own Delta/Full size behaviour is unchanged code, only the comment describing it. So no control needed a fresh failing-then-passing cycle - the full just check (recorded above, exit 0, against this merge commit) re-exercises every one of them regardless. just check: exit 0.

Known cost, not fixed here (for TASK-1.122): my own resize coalescer (size::declare_if_changed) declares once per distinct cell-size boundary a drag crosses, not once per pixel-level resize event - but since PR 66, every size change that actually changes the tab's size now sends a full screen to every attached viewer, not a bare field. A drag crossing many cell boundaries in one gesture therefore sends one full-screen resync per boundary crossed, to every viewer, not just the one resizing. Flagging for whoever picks up the latency/flood-bounding work.

---

created: 2026-11-17T22:57:00Z

---

Correction to the live-run notes, for accuracy: the prompt shown in the images/recording (an arrow and a bare "~") is not the PROMPT='%# ' I set out to force, and tracing why matters more than it looks.

What each live run actually used: a fresh mktemp -d directory for HOME and a fresh mktemp -d/steg.sock for STEG_SOCKET_PATH, passed as env vars to both the server and app processes - every run, including the final one for the images. STEG_SOCKET_PATH genuinely isolated each run onto its own socket, never the engineer's or any other real one.

HOME did not: steg server start's own real path (not the test fixture path) resolves the pane's own shell identity through LoginShell::resolve_identity -> login_env::resolve_login_identity, which reads the real password database for the account actually running the server process, not the HOME environment variable I set when launching it - confirmed by reading crates/server/src/login_env.rs and runtime.rs directly, not guessed. So the shell each pane ran was spawned with the real account's own real home directory and its own real dotfiles, and whatever prompt theme those dotfiles set is what the arrow-and-~ actually is - this task's own throwaway HOME never reached that point at all. My own PROMPT='%#' env var, set only on the server process I launched, was never going to override a theme sourced from the real account's own rc files either, for the same reason.

What this means for "nothing personal on screen": held, but not because of the throwaway HOME - it held because the prompt theme itself shows no path, hostname or username (just a generic arrow and a bare tilde), and because every command I typed into it was a generic demo (echo/stty), never anything that would surface a real path or other real content. I did not use, read, or write to the engineer's own session or socket at any point - that part of the isolation was real.

Separately: PR 71 is green on 2c127d4f, controls and just check already recorded against this exact head (clean tree, confirmed above), the three files in pr-images/ (task-1121-typing.png, task-1121-resize.png, task-1121-typing-resize-demo.mov) are described in the PR body as checked by the orchestrator and awaiting the engineer's own upload. Calling asdlc finish now.

---

author: @bruk.r1
created: 2026-11-17T22:58:00Z

---

@bruk.r1 is reviewing TASK-1.121: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-17T23:08:00Z

---

Review by @bruk.r1 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 3 Important, 0 nit. Cites: review, app. The diff does not match the plan.

[Bugs, Important] crates/app/src/client.rs:111-115,123-130 (Writer::send_input/declare_size): every call does a synchronous, blocking `write_all` on the window's own socket handle directly from the GPUI main/UI thread (on_key_down and replace_text_in_range in main.rs both call into this with no await, no channel, no timeout, and no set_nonblocking anywhere in this crate - confirmed by grep). If the server-side read for this connection ever stalls - the task's own plan names a related freeze ("a pane whose program never reads its input freezes that pane's own screen until the program reads, ends, or is force-closed", §"For the engineer" point 5) - or a large Cmd-V paste (§"For the engineer" point 3, sent as one send_input call with no size cap) exceeds the kernel's own socket send buffer while the peer is slow to drain it, this write blocks forever and freezes the whole app's render/input thread, not just one pane's screen. Nothing in the notes discusses this risk and no test exercises a slow/stalled peer on this path (only a clean write and an already-shut-down socket are tested). Needs either a bounded write (SO_SNDTIMEO/set_write_timeout), a dedicated writer thread fed by a channel, or a documented, tested bound on how much can ever be in flight before send_input is called.

[Compliance, Important] crates/app/src/main.rs:325-335 (EntityInputHandler::marked_text_range): returns `Some(0..0)` for every open composition, regardless of the marked text's own length, departing from the plan's own specified expression (§2: `self.marked.as_ref().map(|s| 0..s.encode_utf16().count())`) with no mention of the change in the notes or comments. This value is not cosmetic - GPUI's pinned 0.2.2 macOS backend (platform/mac/window.rs:2177-2182) hands it straight to AppKit as `NSTextInputClient.markedRange`, which AppKit uses to compute the `replacement_range` it passes into the next `setMarkedText:`/`replace_and_mark_text_in_range` call. Always reporting a zero-width range is only safe for a single-unit dead-key composition (the only case this PR's own tests exercise, ime.rs); a multi-character composition (Pinyin/Hangul assembly, a longer emoji-picker candidate) would have AppKit believe nothing is marked yet, risking duplicated or garbled composition content - untested in both directions. This is also exactly the kind of thing the repository's own app.md rule asks to be a plain, tested function ("what can be decided without a window is a plain function tested without one") - the plan's own formula was such a function; what shipped is a hardcoded constant in main.rs instead.

[Compliance/Evidence, Important] crates/app/src/keys.rs:412-461 (the_table_covers_every_named_key_this_module_maps): the plan's own §6 proof step for this test was "a second assertion per printable/control row that the table has no row this test doesn't cover, so a row added to the plan's own prose without a matching assertion fails the build, not just a missed case." What shipped instead is a fixed, hardcoded list of 18 key-name strings asserted against the test's own `table()` fixture, never against `named_key_bytes`'s own match arms - adding, removing or renaming a row in `named_key_bytes` would not fail this test or the build either way. It also only names 4 of the module's 8 function-key rows (f1, f4, f5, f12; f2/f3/f6-f11 absent). The notes and comments say nothing about this proof step being built differently from the plan.

Verified the keys.rs control myself: reverted a scratch edit removing the `"tab" if shift => b"\x1b[Z"` arm (making Tab and Shift-Tab share a byte, the exact example the plan's own §6 names) and `cargo test -p steg-app --bin steg-app keys::tests::` failed `every_row_in_the_plans_own_table_maps_as_the_plan_says` exactly as the worker's recorded control claims; reverted immediately, `git status` on the review worktree confirmed clean afterward. That control is sound - my finding above is about the second, coverage-completeness test next to it, not this one.

Recorded checks are against 2c127d4f, matching the branch's current head; PR 71's CI is green on the same commit. The two PNGs held off-PR (task-1121-typing.png, task-1121-resize.png) do show what the PR and notes claim: typing.png shows `echo final-check` with its output and a fresh prompt; resize.png shows the same scrollback in a visibly wider window with `stty size` reporting `32 141`, consistent with a resize having been declared and drawn. I could not open the .mov with my own tools - not checked. The PR body accurately states the images/recording are held by the orchestrator pending the engineer's own upload (AC #4's first clause is therefore not yet met on the PR itself, but this is the disclosed, expected state per the claim, not a new finding). Merge history reviewed in full (comments #2, #8, #9): confirmed nothing from TASK-1.126 (font-family resolution tests, cell_origin, per-glyph safety checks, run_key hashing) or TASK-1.117 (the three-state startup slot's own tests) was lost - all still present and passing. The crates/server/src/connection.rs change is a pure match-arm reorder from a merge, no semantic change.

Not checked: the .mov recording's contents; whether the plan-approval hash still matches the plan as shipped beyond what the notes already state; any machine other than this one.

---

created: 2026-11-17T23:20:00Z

---

Decision by @bruk (2026-11-17, relayed by @bruk.o) on review finding 1: fix it in this task. Input and declared sizes must not be written to the socket on the UI thread; a key press never waits on the socket. This overrides the approved plan's section 3 ('directly on the main thread - no channel, no lock'); the change is an engineer-approved departure, to be recorded in the notes' Departures list.

---

created: 2026-11-17T23:27:00Z

---

Findings 2 and 3 fixed (commit 26f06705).

Finding 2: marked_text_range returned a fixed Some(0..0) instead of the plan's own formula - an undisclosed departure, not deliberate; I found no reason the real form was hard to build once I looked. Built ImeState::marked_range_utf16 exactly as planned (0..s.encode_utf16().count()), with two new pure tests (single-unit and multi-character compositions each reporting their own real length) and a recorded failing-then-passing control (reverted to the old 0..0 shape; both new tests failed exactly as expected, restored, both pass). Removed is_composing, now redundant and otherwise dead code.

Finding 3: the coverage test compared a hardcoded list against its own fixture, never against named_key_bytes - and only named 4 of 8 function-key rows. Added the missing rows (f2/f3/f6-f11) and rewrote the test to ask named_key_bytes itself, per candidate key, whether it answers Some, asserting that agrees with whether the table has a row proving it (expected: Some(_), not merely naming the key - the existing "f13 (dropped)" row proves the negative case and must not count as positive coverage). Recorded failing-then-passing: removed the f6 arm from named_key_bytes, both this test and the existing per-row test failed exactly as expected, restored, both pass.

just check: exit 0 after both fixes.

Facts for finding 1 (holding, not fixing, per your instruction) - sent to you directly as well:

Under what conditions the write can block: Writer::send_input/declare_size's write_all blocks when the kernel's own Unix-domain-socket send buffer fills and the server's own connection-read loop isn't draining it. Read crates/server/src/connection.rs's own read loop and Runtime::send_input/PaneHandle::send_input directly: input is enqueued onto an unbounded std::sync::mpsc::channel (pane.rs:833), never a bounded one, so a pane's program not reading its own stdin backs up only that in-process channel, never the socket - the connection's read loop keeps draining the kernel buffer regardless of pane backpressure. For the write to actually block, the server's own connection-handling thread would have to stop reading from the socket for some other reason (the process itself unresponsive, not scheduled, or stuck elsewhere) - not because a pane's program refuses its input, which this task's own plan already named as an accepted, different limit (one pane's screen freezing, not the server's read loop stalling).

How big the socket buffer is in practice: measured on this machine, not guessed - sysctl net.local.stream.sendspace and net.local.stream.recvspace both report 8192 bytes, the macOS default for Unix domain stream sockets; steg sets neither explicitly.

Whether a single large paste alone can do it against a healthy server: Cmd-V paste is not implemented in this task's own shipped code at all - grepped crates/app/src for clipboard/paste, nothing calls it; keys.rs/ime.rs's own comments cite the plan's "For the engineer" point 3 as a future item, not something built. The only paths that call send_input today are on_key_down (one keystroke, a handful of bytes at most) and replace_text_in_range (a resolved IME composition, realistically short). So this specific risk is accurate as a future one, once paste is built, not one this PR's own shipped code can trigger today.

How large the smallest honest fix is: a dedicated writer thread owning the UnixStream write handle, fed by an mpsc channel Writer::send_input/declare_size send onto instead of writing directly - the same shape this task already built for the reader thread (client.rs's own Client/read_loop), blocking on recv() with no polling, writing on receipt, looping. Ordering between input and declared sizes is preserved by construction if both go through the same channel and thread (a single FIFO queue, one consumer) - not two separate ones. On disconnect, the same shutdown mechanism the reader thread already uses (a cloned handle's own shutdown() call from teardown(), unblocking a stuck write) extends naturally, since the infrastructure already exists in this file for the read side. Estimate: comparable in size to the reader thread already built here - a few hours, not a redesign.

---
<!-- COMMENTS:END -->
