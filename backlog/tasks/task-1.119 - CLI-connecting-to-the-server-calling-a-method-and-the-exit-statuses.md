---
id: TASK-1.119
title: 'CLI: connecting to the server, calling a method, and the exit statuses'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-16T13:35:00Z'
updated_date: '2026-11-18T02:39:00Z'
labels:
  - size-2
milestone: m-3
dependencies:
  - TASK-1.49
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.119-cli-connecting-to-the-server-calling-a
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'b60acf121e04'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from TASK-1.49 by its plan on 2026-11-16, under the engineer's standing decision to split by default. doc-2 sections 5, 6 and 8 (R32, R34); decision-4; decision-1; doc-8 R3 and R99; doc-16 R98, R101. TASK-1.49's plan, section 'The split', part 2, holds the worked design, including the fixture that accepts and closes with no hello and the use of TASK-1.86's idle stop for the end-to-end proof if it has merged. Carries TASK-1.49's criteria 3, the autostart half of 4, and 7.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every generated command connects through the sdk, starting a server on demand when none is running, calls its method and exits with the status doc-2 section 6 gives; --no-autostart exits 4 without starting one
- [ ] #2 Each exit status in doc-2 section 6 is produced by a test through the real binary against a real server, and the mapping from the API's error codes is read from the API description, not written a second time
- [ ] #3 With --json a command prints the server's reply unchanged, so the CLI is usable as soon as this task lands; plain text and -q are the next task's
- [ ] #4 A client whose connection is accepted and then closed before a hello arrives treats that the same as no server being there, and arbitrates again exactly once; the logic is in the sdk, where the app will use it too; shown with real processes
- [ ] #5 api call sends the parameters given as one JSON object on standard input
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## For the engineer

Seven small decisions, each with a recommendation. None blocks the build; all are built against the recommendation unless overruled.

1. **How does `--autostart`/`--no-autostart` and `--json` reach every command?** They are not API parameters (confirmed: `methods.rs`'s own `no_method_declares_a_cli_only_flag_as_a_parameter` test already lists `json`/`autostart`/`no_autostart` as CLI-only, so no method can ever collide with them and `methods.rs` needs no edit). **Recommendation:** add them as hand-written `clap` args on the root `Command` in `tree.rs`, each `.global(true)` — `--autostart`/`--no-autostart` as an overriding switch pair (the same shape `add_switch` already gives every generated switch), `--json` as a plain `ArgAction::SetTrue`. clap resolves a global arg from whichever level's `ArgMatches` the caller reads, including the leaf subcommand's own — confirmed against clap 4.6.7's own documented behaviour, not assumed — so `connect.rs`/`api_call.rs` read them straight off `verb_matches`, no plumbing through `main.rs`.

2. **Where does the EOF-before-hello retry live, given `crates/app/src/startup.rs` already calls `steg_sdk::local_socket::connect` and does its own hello afterward?** Extending `connect()` itself to also handshake would make the app send a second hello and silently break it — a file this task must not touch, and another worker is in it now. **Recommendation: a new function, `connect()` left byte-for-byte unchanged.** `local_socket::connect_handshaken(options: &ConnectOptions, client: &str, kind: ClientKind, label: Option<&str>) -> Result<(UnixStream, Vec<u8>), ConnectError>` — calls the existing `connect()` to get a stream, sends the hello (`Connection::send_hello`), then reads with a bound (see point 3) until either a complete `Incoming::Hello` frame is decoded (returns `Ok((stream, payload))`, raw and undecoded — this function's job is the retry, not interpreting the hello; the caller still calls `parse_server_hello` itself, exactly as `startup.rs` does today) or the read hits EOF with nothing decoded yet. **EOF before any complete hello is treated identically to "nothing was listening": the whole arbitration (`connect()` again — a fresh direct-connect-or-spawn-and-wait) is retried exactly once.** A second EOF is not retried again — it returns a new `ConnectError::ClosedBeforeHello`, added as a new enum variant. Checked, not assumed, that this is non-breaking: `startup.rs`'s only use of `ConnectError` is `StartupError::Connect(ConnectError)` with a `Display` impl that forwards via `{err}` — it never matches `ConnectError`'s variants itself, so a new variant compiles there unchanged. **Named follow-up, not built here:** move `crates/app/src/startup.rs`'s own `run`/`attach_over` onto `connect_handshaken` and delete its local hello-sending/reading (the two already overlap almost exactly), so the app gets the same retry-once protection doc-16 R101 asks it to share. Left as its own task, since `crates/app` is off limits here.
   - **A correction to my own first reading, caught before writing this down:** a bare `peek()` on the socket cannot tell "healthy and silent" (the client speaks first; the server says nothing until spoken to) from "accepted and about to close" — the race's FIN may not even have arrived at the instant of the peek. The only sound signal is what TASK-1.49's own appendix already named: send the hello, then read; EOF before a complete hello is the condition. This plan is built on that, not on peeking.

3. **The hello read's own bound.** `connect_handshaken` sets a read timeout before its read loop and clears it before returning the stream — the same justification `startup.rs`'s own module doc already gives almost verbatim ("a timeout on a blocking read is a bound, not polling"), cited rather than re-argued. **Recommendation: reuse `ConnectOptions::connect_timeout` for this bound too**, rather than adding a new field — the whole call, worst case (one EOF, one retry), is bounded by at most twice that duration, which is acceptable and is stated here rather than left for a reviewer to total up.

4. **How does `api call` avoid the client-side `validate()` that would otherwise refuse it?** Checked, not assumed: `Connection::call` unconditionally calls `description::validate` before encoding, and `api.call`'s own description entry declares only one parameter, `method` — a real call's merged params (the called method's own, flat, per decision-1 and the description's own doc comment on `api.call`) would be refused by that check before ever reaching the wire, mirroring exactly why the *server's* `dispatch::api::call` has to intercept `api.call` before its own generic `validate` runs. **Recommendation:** add one more method to `Connection` beside `call`/`watch`/`attach`/`detach`/`declare_size`: `pub fn call_unvalidated(&mut self, method: &str, params: Map<String, Value>) -> (u64, Vec<u8>)` — builds and encodes the request exactly as `call` does, with no `validate` step, documented as the one escape hatch `api.call` needs and nothing else in this crate should reach for (the same reasoning `api.call`'s own description entry already gives: "for a method a plugin adds before the CLI knows it" — a method client-side `validate` cannot know about either). `steg`'s own `api_call.rs` is the only caller.

5. **Malformed JSON on `api call`'s stdin.** Doc-2 doesn't say. **Default: status 2** (bad usage — the shape of what was given, not a server failure). No real alternative argued; low priority either way, checked against one test.

6. **A protocol-level refusal (version mismatch, malformed hello) once a hello reply is in hand.** Not in the description's `errors` table (that table is server-answered API errors only — see criterion 2 below). **Default: status 4**, on the reasoning that an unusable server is, for the CLI's purposes, the same outcome as an unreachable one. Named, not deeply tested (one assertion, if the fixture work below leaves room) — doc-8 R39 is TASK-1.119's to map, not to re-litigate.

7. **An I/O failure after a successful connect (a write or read that errors outright, distinct from the clean EOF-before-hello case point 2 already covers).** **Default: status 4** — grouping every "could not finish talking to a server" outcome under one status is the simpler rule for a script to check against (`if status == 4, retry later`, with no second case to remember). **Alternative: status 1** ("the command ran and failed") — arguably more honest, since the server *was* reached once, and a script that wants to tell "never found a server" apart from "found one, then lost it" would want the two statuses to differ. Recommendation stays with 4, on the simplicity argument, but this is genuinely closer to a coin flip than points 5/6 and is named as such.

8. **How is exit status 130 actually produced?** Changed from this plan's first pass, on review: **no SIGINT handler at all.** Left at its default disposition, an interrupted `steg` dies by the signal, which is exactly what a shell reports as 130 (128 + 2) — the behaviour well-behaved command-line tools aim for (a parent can tell the child was interrupted; a script's own Ctrl-C handling keeps working), and it costs nothing: no new dependency, no change to `boundaries.toml`'s `[steg]` entry (which this task otherwise has no reason to touch), and no async-signal-safety rules to keep. Checked for a reason this can't work here, not assumed: `steg` pulls in no async runtime and nothing in `crates/server`/`crates/kernel`/`crates/sdk` installs a signal handler of its own (grepped directly, not recalled) — nothing already claims SIGINT out from under this. The test asserts the real, observable fact — `ExitStatus::signal() == Some(2)` (`std::os::unix::process::ExitStatusExt`, already stable, no new dependency) — and the plan states in one line, rather than leaving it to a reader to connect, that doc-2's "130" is the status a shell shows for a process a signal ended, not a literal `exit()` call this binary makes. **Named alternative, not built:** a real handler (`libc::signal` + `process::exit(130)`) would make `.code()` observable too, for a parent that inspects the child directly rather than through a shell — but it would need `libc` added to `[steg]`'s boundary (a package-boundary change this task has no other reason to make) and a handler that is not strictly async-signal-safe (named, not solved, in the first pass of this plan). The one thing that would justify paying that cost: cleanup `steg` must do before exiting that the OS's own teardown of a signalled process wouldn't do for it. There is none today — every command here either hasn't connected yet (nothing to clean up) or holds a `UnixStream` the OS closes on process death the same way either kind of exit would. If a later task gives the CLI something to flush or unwind, that is the point to revisit this, not now.

## Is this one change or two?

**One.** The brief's own default is to split, but the natural cut — the sdk's `connect_handshaken`/`call_unvalidated` on one side, the CLI's use of them on the other — would land a public sdk function with no consumer until the same task's CLI half calls it, and criterion 1 ("every generated command connects... and exits with the status doc-2 section 6 gives") spans both halves by its own wording. Nothing in the appendix's own two-file split (`local_socket.rs` and `crates/steg/src/cli/*`) suggested cutting a second task either. Kept as one.

## Design

### sdk (`crates/sdk/src/local_socket.rs`, `crates/sdk/src/wire/connection.rs`)

- `local_socket.rs`: add `connect_handshaken` (point 2) and `ConnectError::ClosedBeforeHello` (`Display`: "the server accepted the connection and closed it before a hello arrived, twice"). `connect()` itself is untouched — its own existing unit tests keep passing unmodified, which is itself part of the proof that this is additive, not a rewrite.
- `wire/connection.rs`: add `Connection::call_unvalidated` (point 4), documented as `api.call`'s own escape hatch.
- No change to `description/methods.rs` (point 1's own finding) and no change to `crates/app` or `crates/server/src` (collision constraints respected throughout).

### `crates/steg/src/cli/`

- `tree.rs`: add the two global args to `build()`'s root `Command` (point 1). `NON_METHOD_COMMANDS` and the per-method loop are untouched.
- `values.rs`, `params.rs`: untouched — `extract_params` already reads a method's own described params, which is all it ever needs to.
- `connect.rs` (new): the real path. Resolves its own binary via `std::env::current_exe()` (simpler than the app's `resolve_binary` — this binary *is* the one to autostart, no `STEG_BINARY` override needed), builds `ConnectOptions { binary, socket_path: None, autostart, connect_timeout: CONNECT_TIMEOUT }` (a module constant, 10 s, matching the app's own), calls `local_socket::connect_handshaken` with `ClientKind::Cli` and `client = format!("steg-cli/{}", env!("CARGO_PKG_VERSION"))`, `parse_server_hello`s the payload itself, then `connection.call(dotted, params)`, writes, and blocks (a small local read-poll loop, the same shape `server_start.rs`'s own `call_and_read_reply` test helper and `startup.rs`'s own `block_until` already use — glue code reading bytes and driving `Connection::poll`, not protocol logic, so writing one here is not a R101 violation) until the matching reply arrives. On `Response::Ok`, prints the raw `serde_json::Value` (its own `Display` impl is already compact JSON) when `--json` was given, nothing otherwise (point below); on `Response::Error`, prints the message to stderr always, JSON or not. A write/read failure distinct from the EOF-before-hello case (point 7) maps to its own status directly.
- `exit.rs` (new): `pub fn api_error_status(api: &Description, code: ErrorCode) -> i32` — looks the code up in `api.errors` and returns its own `exit_status`, panicking only if the table is missing an entry the description's own completeness test (`errors_has_twelve_codes_with_their_exit_statuses`) already guarantees exists. `pub fn connect_error_status(err: &ConnectError) -> i32` — every variant, including the new `ClosedBeforeHello`, maps to 4.
- `api_call.rs` (new): reads stdin fully, parses as a JSON object (anything else, or invalid JSON, is status 2 — point 5), reads the target's own method name, merges `{"method": name}` into the object, calls `Connection::call_unvalidated("api.call", merged)`, writes, blocks for the reply the same way `connect.rs` does, and **always** prints the raw reply as JSON (doc-2 section 3: `api call`'s own output is JSON by its nature, not gated on `--json`) — the one command this task special-cases outside the generic `--json` rule, named as such.
- `mod.rs`: drop `run` (the module and its placeholder are deleted outright, per TASK-1.49's own appendix: "meant to be replaced outright... not built around"); re-export `connect::run` as `run` so `main.rs`'s call sites barely change.
- `main.rs`: no signal handler (point 8). In `run_generated`'s dispatch, add one more special case beside `server.start`: `dotted == "api.call"` goes to `steg::cli::api_call::run`; everything else goes to the new `steg::cli::run` (unchanged call shape, now doing the real work).
- No `boundaries.toml` or `Cargo.toml` edit (point 8's reversal removes the only reason this task had to touch either).

### Deleted

- `crates/steg/src/cli/run.rs` and `crates/steg/tests/cli_run_placeholder.rs` — the placeholder and its own test, both named in TASK-1.49's plan as this task's to replace outright. `cli_drift.rs` and `cli_no_server.rs` are untouched (their own criteria, #1 and the no-server half of #4 from TASK-1.49, don't change).

## Order of work

1. `local_socket.rs`: `ConnectError::ClosedBeforeHello`, then `connect_handshaken`, proved first with the in-process, two-listener-thread unit tests (no real server, no new binary — see criterion 4's proof below) before anything in `steg` depends on it.
2. `wire/connection.rs`: `call_unvalidated`, proved with a small unit test alongside `call`'s own existing ones.
3. `tree.rs`: the two global args, proved by a focused test that `--no-autostart` parses both before and after the target (clap's global-arg-at-any-level behaviour is the one part its own docs don't spell out, so it is checked here rather than assumed).
4. `exit.rs`, then `connect.rs`, then wire `main.rs`'s dispatch — in that order, since `connect.rs` is what `exit.rs`'s functions exist to serve, and `main.rs`'s wiring is what makes it reachable.
5. `api_call.rs`, last of the three new `steg` files, since it depends on `call_unvalidated`'s own shape settling first.
6. Delete `run.rs`/`cli_run_placeholder.rs`.
7. `cli_exit_statuses.rs` and `cli_connection_arbitration.rs`, built against the finished code, each with its own negative control (below).
8. `just check` at each of the boundary points above, recorded by `asdlc record-check`; a final `just check` after everything, and the negative controls performed by hand and reverted before the commit they'd otherwise land in (matching TASK-1.49's own convention for this).

## Risks

- **The hello-read loop's bound (point 3) interacting with the retry (point 2).** Worst case is roughly `2 × connect_timeout`; stated here so it isn't a surprise at review, and the 10 s default keeps even the worst case well under anything a person would sit through once, while the 130 test (below) needs a scenario that *doesn't* hit this bound at all — a fixture that completes the hello and then goes silent only after that point, never during it.
- **The account database, not `HOME`, resolves a started pane's shell.** The status-1 test (below) that splits a pane names its program explicitly (`-- /bin/cat`), never a bare shell, so it never depends on this machine's real login shell.
- **Every test in `cli_exit_statuses.rs` and `cli_connection_arbitration.rs` uses a throwaway `STEG_SOCKET_PATH` per test** (the same `unique_socket_path` shape already duplicated across this crate's test files), and every wait is bounded (`wait_for_socket`/`wait_bounded`/a channel with `recv_timeout`, the same shapes already in `server_start.rs`/`idle_stop.rs`) — no sleep-and-hope.

## Proof, one block per criterion

### #1 (connects through the sdk, autostarts, calls, exits per section 6; `--no-autostart` exits 4)
- `cli_exit_statuses.rs::no_autostart_with_nothing_running_exits_4`: a throwaway socket path nothing is listening at, `steg session list --no-autostart`, assert exit 4, assert (the same `assert_never_accepted`-shaped listener trick `cli_no_server.rs` already uses, reused here) that no server process is left running afterward.
- `cli_exit_statuses.rs::a_real_round_trip_through_an_autostarted_server_succeeds`: `steg session list --json` against a throwaway path with nothing running and `--autostart` (the default); assert exit 0, assert stdout is `[]` (no sessions on a fresh server), assert a real server process now answers at that path. `session.list` is chosen because it's already proven live end to end (`crates/app/src/startup.rs`'s own `find_or_make_session` calls it for real), not merely described.

### #2 (every exit status from the description's own table, never written a second time)
- The "never written a second time" half, proved directly rather than asserted: `exit.rs`'s own unit test iterates every `ErrorRecord` in `description().errors` and asserts `api_error_status` returns exactly that record's own `exit_status`; negative control: clone the description, change one record's `exit_status` to a different value, assert the function follows the change (proving it reads the table, not a parallel hard-coded match).
- **0:** criterion #1's own success test above.
- **1 ("ran and failed"), via real dispatch, not a stub that could land under this task:** `cli_exit_statuses.rs::a_program_still_running_refuses_close_with_status_1`: autostart a real server, `steg pane split -- /bin/cat` (explicit program, per the risk above), then `steg session close <that session>` with no `--force` — real `dispatch::session::close`'s own `ProgramRunning` refusal, exit 1. Named fallback, not built unless time allows: `steg server show` (confirmed, by reading `crates/server/src/dispatch/mod.rs` directly, to still answer `NotImplementedYet` today) as a second, cheaper example — but the primary proof is the one that survives `server.show` landing later.
- **2 ("bad usage"):** one clap-level case kept in this file for self-containedness (`steg pane split` with `--direction` given a word outside its closed set, exit 2 — the same mechanism `cli_run_placeholder.rs` proved before being deleted, re-asserted here so criterion #2's evidence doesn't rest on a deleted file), plus `api_call.rs`'s own malformed-stdin case (point 5).
- **3 (target matched nothing/more than one):** `steg session show <a syntactically valid but nonexistent id>` against a real autostarted server — real `dispatch::session::show`'s own `NoSuchTarget`, exit 3.
- **4:** criterion #1's own `--no-autostart` test above, plus `connect_error_status`'s own unit test covering `Spawn`/`Timeout`/`ClosedBeforeHello` directly (no process needed for these three — they're constructed by hand and passed straight to the function).
- **130:** `cli_exit_statuses.rs::sigint_during_a_hung_call_ends_the_process_by_signal`: a hostile-but-cooperative in-process listener thread that completes a real hello (so the CLI gets past the handshake, landing it inside the one place nothing bounds the wait — the reply read) and then never answers the call at all; spawn the real `steg` binary against it, confirm (via `try_wait`) it is genuinely still running, send it `SIGINT` (`Command::new("kill").args(["-s", "INT", &pid])`, the same mechanism `server_start.rs`'s own SIGHUP test already uses), assert `wait_bounded` returns `.signal() == Some(2)` within a short window — point 8's own default disposition, proved rather than assumed, with the plan's own note that this is what a shell renders as 130.

### #3 (`--json` prints the reply unchanged; plain text is the next task's)
- `cli_exit_statuses.rs::json_prints_the_replys_own_shape_unchanged`: `steg session list --json` against a real server with one session created first (`steg session new --label proof --json`, itself exercising `Returns::Identifier`'s own JSON shape) — assert the printed bytes parse as JSON and the array has exactly one record whose fields are the API's own names.
- **The built-in contrast the criterion needs to not pass vacuously:** `cli_exit_statuses.rs::without_json_a_successful_command_prints_nothing_on_stdout`: the identical call with `--json` dropped — assert stdout is empty, stderr is empty, exit 0. This is deliberate and named: plain-text rendering, even for the trivial `Returns::Nothing`/`Identifier` cases, is TASK-1.120's, not reused early here.
- **The error case, both ways:** one test with `--json` and a request that fails server-side — assert nothing on stdout, the message on stderr, the mapped status — so `--json` is never read as "also formats errors as JSON."
- **`api call`'s own exception, stated and tested:** `api_call.rs`'s own test calls `api call session.list` with `{}` on stdin, with *no* `--json` given at all, and still gets JSON on stdout — proving the documented exception (doc-2 section 3) rather than merely asserting it in prose.

### #4 (EOF-before-hello treated as no server, arbitrated again exactly once, in the sdk, shown with real processes)
- **The load-bearing proof, in `local_socket.rs`'s own unit tests — no new process, two in-thread `UnixListener`s with a shared `AtomicU32` accept counter:**
  - Mode A: every accepted connection is dropped immediately with nothing written. `connect_handshaken` with `autostart: false` (so `connect()`'s own retry never spawns anything, isolating this test to the EOF logic alone) against that listener's path: asserts the call returns `Err(ConnectError::ClosedBeforeHello)` and the counter reads exactly 2 — not 1 (proving the retry happened at all) and not 3 or more (proving it happened *exactly* once, the sharpest part of this criterion).
  - Mode B: the first accepted connection is dropped immediately; the second is answered with a real `Connection::accept_or_refuse`-built hello. Asserts `connect_handshaken` returns `Ok` and the counter reads exactly 2.
  - Both reuse `bounded`/`bounded_join`-shaped helpers (the same pattern `startup.rs`'s own tests already use) so a regression that makes this hang fails the test, not the test run.
- **Corroboration with real processes, kept weaker on purpose so a race can't make it flaky:** `cli_connection_arbitration.rs`: an in-process hostile listener thread (mode A, above) bound at a throwaway socket, then the real `steg` binary (`steg session list --json`, autostart default) run as a genuine child process against that same path. Asserts only that the command succeeds (exit 0) within a bounded wait — not a specific retry count, which a real process's own timing can't be pinned to as tightly as the in-thread test above already did precisely. If TASK-1.86 is still merged on `main` by build time (confirmed already merged, `idle_stop.rs` present), its own end-to-end scenario (a real on-demand server stopping itself, then a fresh connect starting a new one) is additional, real-process evidence that the retry path is exercised in practice — corroboration, not a substitute for the counting proof above, exactly as the appendix said.

### #5 (`api call` sends parameters as one JSON object on stdin)
- `api_call.rs`'s own test: `echo '{"direction": "right"}' | steg api call pane.split` (with an explicit prior `pane.split -- /bin/cat` target set up, or run against the default current-pane-less target and accept whatever the real `NoSuchTarget`/success outcome is — settled during the build once a real session exists to target) — asserts the JSON on stdin reaches the server as `pane.split`'s own `direction` parameter, not nested under a second key, by asserting on the real effect (a new pane appears in the expected direction) rather than only on the exit status.
- `Connection::call_unvalidated`'s own unit test: builds a request with extra keys `api.call`'s own description doesn't declare, asserts it still encodes (where `Connection::call` with the same keys would have already returned an `ApiError` before this test could even reach the assertion) — the direct proof that this path really does skip `validate`, not merely that the CLI happens to work.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan checked against doc-2 sections 5, 6 and 8, doc-16 and the task's five criteria by @bruk.o: ok. The sdk gains connect_handshaken beside an untouched connect() (arbitrate again exactly once when a connection closes before its hello) and call_unvalidated for api call; the CLI's exit status is read from the description's own errors table; --json prints the reply unchanged. It stays inside crates/steg and crates/sdk, touches no package boundary, and leaves crates/app's own start-up on the old path as a named follow-up. Status 130 is the default signal disposition, at the orchestrator's request (the first plan added a handler and libc). Four status choices go to the engineer with the plan.

Check FAILED: `just check` exited 101 at 2026-11-18T02:39:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c, last 40 of 79 lines:

```text
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test a_frame_on_an_already_detached_stream_closes_the_connection ... ok
test input_clears_a_finished_status ... ok
test attach_returns_the_real_screen_and_updates_follow_it ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test typed_bytes_reach_a_real_program_in_order ... ok
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach has been running for over 60 seconds
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane has been running for over 60 seconds
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (56811726) panicked at crates/server/tests/attach.rs:1427:6:
pane.show must answer within a bound, even with the other pane stuck
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane

test result: FAILED. 22 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 123.87s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/test-steg-server-race-fixture.log
error: recipe `check-packages` failed with exit code 101
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T02:30:00Z

---

Plan approved by @bruk (b60acf121e04, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
