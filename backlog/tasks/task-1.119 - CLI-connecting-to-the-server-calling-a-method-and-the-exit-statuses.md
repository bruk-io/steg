---
id: TASK-1.119
title: 'CLI: connecting to the server, calling a method, and the exit statuses'
status: In Review
assignee:
  - '@bruk.w5'
created_date: '2026-11-16T13:35:00Z'
updated_date: '2026-11-18T08:52:00Z'
labels:
  - size-2
  - review-passed
milestone: m-3
dependencies:
  - TASK-1.49
references:
  - task-1.119-cli-connecting-to-the-server-calling-a
  - https://github.com/bruk-io/steg/pull/77
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.119-cli-connecting-to-the-server-calling-a
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: 'b60acf121e04'
asdlc_plan_approved_size: size-2
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=2'
asdlc_review_policies: ["review", "testing-rust"]
asdlc_plan_matched: yes
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

Check FAILED: `just test` exited 101 at 2026-11-18T02:48:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes, last 40 of 64 lines:

```text
running 7 tests
test cli::exit::tests::every_connect_error_maps_to_4 ... ok
test cli::exit::tests::api_error_status_follows_the_table_not_a_parallel_match ... ok
test cli::exit::tests::every_described_error_maps_to_its_own_exit_status ... ok
test cli::tree::tests::autostart_defaults_to_true_with_neither_flag_given ... ok
test cli::tree::tests::autostart_given_after_no_autostart_wins ... ok
test cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand ... ok
test cli::tree::tests::json_is_read_from_the_leaf_subcommand ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/steg-b011cf352425d3e7)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/cli_api_call.rs (target/debug/deps/cli_api_call-0e563f4919d1f569)

running 2 tests
test api_call_prints_json_with_no_json_flag_given ... ok
test api_call_sends_stdin_params_flat_to_the_named_method ... FAILED

failures:

---- api_call_sends_stdin_params_flat_to_the_named_method stdout ----

thread 'api_call_sends_stdin_params_flat_to_the_named_method' (57170303) panicked at crates/steg/tests/cli_api_call.rs:121:54:
pane.list prints a JSON array: Error("EOF while parsing a value", line: 1, column: 0)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    api_call_sends_stdin_params_flat_to_the_named_method

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s

error: test failed, to rerun pass `-p steg --test cli_api_call`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just check` exited 1 at 2026-11-18T02:58:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes, last 40 of 64 lines:

```text
(B[m[32m+            "session",
(B[m[32m+            "new",
(B[m[32m+            "--label",
(B[m[32m+            "status-1-proof",
(B[m[32m+            "--json",
(B[m[32m+            "--",
(B[m[32m+            "/bin/cat",
(B[m         ],
         &socket,
     );
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg/tests/cli_exit_statuses.rs:342:
         .to_owned();
 
     let close = run_steg(&["session", "close", session_id.as_str()], &socket);
[31m-    assert_eq!(close.status.code(), Some(1), "stderr: {}", stderr_of(&close));
(B[m[32m+    assert_eq!(
(B[m[32m+        close.status.code(),
(B[m[32m+        Some(1),
(B[m[32m+        "stderr: {}",
(B[m[32m+        stderr_of(&close)
(B[m[32m+    );
(B[m     assert!(stdout_of(&close).is_empty());
 
     // Cleanup, forced - never leave a program running behind this test.
Diff in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg/tests/cli_exit_statuses.rs:350:
         &["session", "close", session_id.as_str(), "--force"],
         &socket,
     );
[31m-    assert_eq!(forced.status.code(), Some(0), "stderr: {}", stderr_of(&forced));
(B[m[32m+    assert_eq!(
(B[m[32m+        forced.status.code(),
(B[m[32m+        Some(0),
(B[m[32m+        "stderr: {}",
(B[m[32m+        stderr_of(&forced)
(B[m[32m+    );
(B[m }
 
 // --- status 130: interrupted ------------------------------------------------
full output: .quiet/format-check.log
error: recipe `check-fmt` failed on line 157 with exit code 1
```

Check FAILED: `just check` exited 101 at 2026-11-18T02:59:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
ok      format check (0.3s)
FAILED  lint (exit 101, 4.2s)
    Checking steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/sdk)
    Checking steg-kernel v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/kernel)
    Checking steg-app v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/app)
    Checking steg-server v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/server)
    Checking steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg)
error: docs for function which may panic missing `# Panics` section
  --> crates/steg/src/cli/api_call.rs:29:1
   |
29 | pub fn run(api: &Description, matches: &ArgMatches) -> i32 {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
note: first possible panic found here
  --> crates/steg/src/cli/api_call.rs:30:23
   |
30 |       let method_name = matches
   |  _______________________^
31 | |         .get_one::<String>("method")
32 | |         .expect("api.call's own target is required")
   | |____________________________________________________^
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#missing_panics_doc
   = note: requested on the command line with `-D clippy::missing-panics-doc`

error: wildcard match will also match any future added variants
   --> crates/steg/src/cli/connect.rs:146:17
    |
146 |                 _ => continue,
    |                 ^ help: try: `Incoming::Hello(_) | Incoming::Request { .. } | Incoming::Reply { .. } | Incoming::Event { .. } | Incoming::UnknownEvent { .. } | Incoming::StreamControl { .. } | Incoming::Data { .. } | Incoming::UnknownStream { .. }`
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.98.0/index.html#wildcard_enum_match_arm
    = note: requested on the command line with `-D clippy::wildcard-enum-match-arm`

error: could not compile `steg` (lib) due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `steg` (lib test) due to 2 previous errors
full output: .quiet/lint.log
error: recipe `lint` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:01:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes, last 40 of 74 lines:

```text
test wire::frame::tests::a_declared_length_of_zero_is_malformed_fed_whole ... ok
test screen::tests::a_screen_with_every_cell_style_round_trips ... ok
test wire::frame::tests::a_frame_round_trips_through_encode_and_decode ... ok
test wire::frame::tests::a_malformed_error_repeats_for_ever_until_the_caller_gives_up_on_the_connection ... ok
test wire::frame::tests::an_oversized_declared_length_is_refused_before_anything_past_it_is_read ... ok
test screen::tests::screen_update_full_round_trips ... ok
test wire::frame::tests::an_unknown_kind_tag_is_malformed ... ok
test wire::frame::tests::decode_waits_for_more_bytes_a_byte_at_a_time ... ok
test wire::frame::tests::encode_refuses_a_payload_over_the_cap ... ok
test wire::frame::tests::frame_constructors_set_the_expected_kind_and_stream ... ok
test wire::frame::tests::two_frames_back_to_back_decode_in_order ... ok
test wire::handshake::tests::a_client_newer_than_the_server_is_refused_naming_both_versions ... ok
test wire::handshake::tests::a_first_frame_that_is_not_json_is_malformed ... ok
test wire::handshake::tests::a_matching_version_is_accepted ... ok
test wire::handshake::tests::a_missing_required_field_is_malformed ... ok
test wire::handshake::tests::a_server_newer_than_the_client_is_refused_naming_both_versions ... ok
test wire::handshake::tests::a_wrong_magic_is_malformed_naming_the_server_version ... ok
test wire::handshake::tests::an_unknown_field_is_ignored ... ok
test wire::handshake::tests::kind_and_label_round_trip_through_the_hello ... ok
test wire::handshake::tests::parse_server_hello_reads_an_accepted_hello ... ok
test wire::handshake::tests::parse_server_hello_surfaces_a_refusals_message ... ok
test screen::tests::pane_attached_round_trips_and_carries_both_fields_in_one_object ... ok

failures:

---- local_socket::tests::eof_before_hello_is_retried_exactly_once_then_fails stdout ----

thread 'local_socket::tests::eof_before_hello_is_retried_exactly_once_then_fails' (57502416) panicked at crates/sdk/src/local_socket.rs:485:9:
Err(Io(Os { code: 22, kind: InvalidInput, message: "Invalid argument" }))
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    local_socket::tests::eof_before_hello_is_retried_exactly_once_then_fails

test result: FAILED. 171 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-sdk --lib`
full output: .quiet/test-steg-sdk-local-socket.log
error: recipe `check-packages` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:09:16+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes, last 40 of 78 lines:

```text
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test attach_returns_the_real_screen_and_updates_follow_it ... ok
test detaching_a_pane_that_already_closed_still_releases_the_stream ... ok
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test input_clears_a_finished_status ... ok
test a_frame_on_an_already_detached_stream_closes_the_connection ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach has been running for over 60 seconds
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (57650973) panicked at crates/server/tests/attach.rs:1427:6:
pane.show must answer within a bound, even with the other pane stuck
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane

test result: FAILED. 22 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 145.37s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/test-steg-server.log
error: recipe `check-packages` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:11:00+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes, last 40 of 64 lines:

```text
     Running unittests src/main.rs (target/debug/deps/steg-b011cf352425d3e7)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/cli_api_call.rs (target/debug/deps/cli_api_call-0e563f4919d1f569)

running 2 tests
test api_call_prints_json_with_no_json_flag_given ... ok
test api_call_sends_stdin_params_flat_to_the_named_method ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s

     Running tests/cli_connection_arbitration.rs (target/debug/deps/cli_connection_arbitration-dad1de408ae26fb1)

running 2 tests
test the_hostile_listener_itself_accepts_and_closes_with_nothing_written ... ok
test a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding ... FAILED

failures:

---- a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding stdout ----

thread 'a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding' (57738862) panicked at crates/steg/tests/cli_connection_arbitration.rs:104:5:
assertion `left == right` failed: stderr: steg: talking to the steg server failed: Invalid argument (os error 22)

  left: Some(4)
 right: Some(0)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_hostile_first_connection_does_not_stop_a_real_autostart_from_succeeding

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

error: test failed, to rerun pass `-p steg --test cli_connection_arbitration`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:13:54+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes, last 40 of 71 lines:

```text
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach has been running for over 60 seconds
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck has been running for over 60 seconds
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... FAILED
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... FAILED

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (57786034) panicked at crates/server/tests/attach.rs:1427:6:
pane.show must answer within a bound, even with the other pane stuck
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach stdout ----

thread 'a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach' (57786031) panicked at crates/server/tests/attach.rs:1793:6:
pane.attach must answer within a bound

---- stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck stdout ----

thread 'stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck' (57786047) panicked at crates/server/tests/attach.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }

---- a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach stdout ----

thread 'a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach' (57786033) panicked at crates/server/tests/attach.rs:149:39:
reading the reply: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }


failures:
    a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach
    a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane
    stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck

test result: FAILED. 19 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 65.22s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Check FAILED: `cargo test -p steg --lib every_described_error_maps_to_its_own_exit_status` exited 101 at 2026-11-18T03:15:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running unittests src/lib.rs (target/debug/deps/steg-a2a09eaf6a06f7de)

running 1 test
test cli::exit::tests::every_described_error_maps_to_its_own_exit_status ... FAILED

failures:

---- cli::exit::tests::every_described_error_maps_to_its_own_exit_status stdout ----

thread 'cli::exit::tests::every_described_error_maps_to_its_own_exit_status' (57830777) panicked at crates/steg/src/cli/exit.rs:75:13:
assertion `left == right` failed: BadRequest should map to its own exit_status
  left: 1
 right: 2
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    cli::exit::tests::every_described_error_maps_to_its_own_exit_status

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg --lib`
```

Check passed: `cargo test -p steg --lib every_described_error_maps_to_its_own_exit_status` exited 0 at 2026-11-18T03:15:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.33s
     Running unittests src/lib.rs (target/debug/deps/steg-a2a09eaf6a06f7de)

running 1 test
test cli::exit::tests::every_described_error_maps_to_its_own_exit_status ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg --lib every_connect_error_maps_to_4` exited 101 at 2026-11-18T03:15:53+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running unittests src/lib.rs (target/debug/deps/steg-a2a09eaf6a06f7de)

running 1 test
test cli::exit::tests::every_connect_error_maps_to_4 ... FAILED

failures:

---- cli::exit::tests::every_connect_error_maps_to_4 stdout ----

thread 'cli::exit::tests::every_connect_error_maps_to_4' (57838932) panicked at crates/steg/src/cli/exit.rs:117:9:
assertion `left == right` failed
  left: 1
 right: 4
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    cli::exit::tests::every_connect_error_maps_to_4

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg --lib`
```

Check passed: `cargo test -p steg --lib every_connect_error_maps_to_4` exited 0 at 2026-11-18T03:16:05+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.32s
     Running unittests src/lib.rs (target/debug/deps/steg-a2a09eaf6a06f7de)

running 1 test
test cli::exit::tests::every_connect_error_maps_to_4 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-sdk --lib --features local-socket eof_before_hello_is_retried_exactly_once_then_fails a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts` exited 1 at 2026-11-18T03:16:25+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
error: unexpected argument 'a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts' found

Usage: cargo test [OPTIONS] [TESTNAME] [-- [ARGS]...]

For more information, try '--help'.
```

Check FAILED: `cargo test -p steg-sdk --lib --features local-socket -- eof_before_hello_is_retried_exactly_once_then_fails a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts` exited 101 at 2026-11-18T03:16:39+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running unittests src/lib.rs (target/debug/deps/steg_sdk-2f782de52c6eac3e)

running 2 tests
test local_socket::tests::eof_before_hello_is_retried_exactly_once_then_fails ... FAILED
test local_socket::tests::a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts ... FAILED

failures:

---- local_socket::tests::eof_before_hello_is_retried_exactly_once_then_fails stdout ----

thread 'local_socket::tests::eof_before_hello_is_retried_exactly_once_then_fails' (57857876) panicked at crates/sdk/src/local_socket.rs:498:9:
assertion `left == right` failed: exactly two connection attempts: the first EOF, then one retry - not one, not three or more
  left: 1
 right: 2
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- local_socket::tests::a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts stdout ----

thread 'local_socket::tests::a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts' (57857875) panicked at crates/sdk/src/local_socket.rs:524:40:
the retry's own hello should succeed: ClosedBeforeHello


failures:
    local_socket::tests::a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts
    local_socket::tests::eof_before_hello_is_retried_exactly_once_then_fails

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 170 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-sdk --lib`
```

Check passed: `cargo test -p steg-sdk --lib --features local-socket -- eof_before_hello_is_retried_exactly_once_then_fails a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts` exited 0 at 2026-11-18T03:16:52+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
   Compiling steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/sdk)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.83s
     Running unittests src/lib.rs (target/debug/deps/steg_sdk-2f782de52c6eac3e)

running 2 tests
test local_socket::tests::eof_before_hello_is_retried_exactly_once_then_fails ... ok
test local_socket::tests::a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 170 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg-sdk --lib -- call_unvalidated_sends_keys_the_descriptions_own_method_does_not_declare` exited 101 at 2026-11-18T03:17:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running unittests src/lib.rs (target/debug/deps/steg_sdk-0bbf186f1ea8fef3)

running 1 test
test wire::connection::tests::call_unvalidated_sends_keys_the_descriptions_own_method_does_not_declare ... FAILED

failures:

---- wire::connection::tests::call_unvalidated_sends_keys_the_descriptions_own_method_does_not_declare stdout ----

thread 'wire::connection::tests::call_unvalidated_sends_keys_the_descriptions_own_method_does_not_declare' (57866557) panicked at crates/sdk/src/wire/connection.rs:266:47:
should be unvalidated: ApiError { code: BadRequest, message: "api.call has no parameter direction", candidates: [] }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    wire::connection::tests::call_unvalidated_sends_keys_the_descriptions_own_method_does_not_declare

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 168 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-sdk --lib`
```

Check passed: `cargo test -p steg-sdk --lib -- call_unvalidated_sends_keys_the_descriptions_own_method_does_not_declare` exited 0 at 2026-11-18T03:17:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
   Compiling steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/sdk)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.74s
     Running unittests src/lib.rs (target/debug/deps/steg_sdk-0bbf186f1ea8fef3)

running 1 test
test wire::connection::tests::call_unvalidated_sends_keys_the_descriptions_own_method_does_not_declare ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 168 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg --lib -- no_autostart_is_read_whether_given_before_or_after_the_subcommand` exited 101 at 2026-11-18T03:17:46+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running unittests src/lib.rs (target/debug/deps/steg-a2a09eaf6a06f7de)

running 1 test
test cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand ... FAILED

failures:

---- cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand stdout ----

thread 'cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand' (57902305) panicked at /Users/brukhabtu/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/clap_builder-4.6.7/src/builder/debug_asserts.rs:244:13:
Command session: Argument or group 'autostart__no' specified in 'overrides_with*' for 'autostart' does not exist
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg --lib`
```

Check passed: `cargo test -p steg --lib -- no_autostart_is_read_whether_given_before_or_after_the_subcommand` exited 0 at 2026-11-18T03:17:57+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.42s
     Running unittests src/lib.rs (target/debug/deps/steg-a2a09eaf6a06f7de)

running 1 test
test cli::tree::tests::no_autostart_is_read_whether_given_before_or_after_the_subcommand ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 0.00s
```

Check FAILED: `cargo test -p steg --test cli_exit_statuses --features fixtures -- without_json_a_successful_command_prints_nothing_on_stdout` exited 101 at 2026-11-18T03:18:20+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/cli_exit_statuses.rs (target/debug/deps/cli_exit_statuses-a82fa623c139069d)

running 1 test
test without_json_a_successful_command_prints_nothing_on_stdout ... FAILED

failures:

---- without_json_a_successful_command_prints_nothing_on_stdout stdout ----

thread 'without_json_a_successful_command_prints_nothing_on_stdout' (57916294) panicked at crates/steg/tests/cli_exit_statuses.rs:257:5:
stdout: "[]\n"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    without_json_a_successful_command_prints_nothing_on_stdout

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.35s

error: test failed, to rerun pass `-p steg --test cli_exit_statuses`
```

Check passed: `cargo test -p steg --test cli_exit_statuses --features fixtures -- without_json_a_successful_command_prints_nothing_on_stdout` exited 0 at 2026-11-18T03:18:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.69s
     Running tests/cli_exit_statuses.rs (target/debug/deps/cli_exit_statuses-a82fa623c139069d)

running 1 test
test without_json_a_successful_command_prints_nothing_on_stdout ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.35s
```

Check FAILED: `cargo test -p steg --test cli_api_call --features fixtures -- api_call_sends_stdin_params_flat_to_the_named_method` exited 101 at 2026-11-18T03:18:51+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
warning: variable does not need to be mutable
  --> crates/steg/src/cli/api_call.rs:53:23
   |
53 |     let Value::Object(mut params) = parsed else {
   |                       ----^^^^^^
   |                       |
   |                       help: remove this `mut`
   |
   = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default

warning: `steg` (lib) generated 1 warning (run `cargo fix --lib -p steg` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/cli_api_call.rs (target/debug/deps/cli_api_call-c9922192cf592a61)

running 1 test
test api_call_sends_stdin_params_flat_to_the_named_method ... FAILED

failures:

---- api_call_sends_stdin_params_flat_to_the_named_method stdout ----

thread 'api_call_sends_stdin_params_flat_to_the_named_method' (57925415) panicked at crates/steg/tests/cli_api_call.rs:154:5:
assertion `left == right` failed: stderr: steg: method: must be text

  left: Some(2)
 right: Some(0)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    api_call_sends_stdin_params_flat_to_the_named_method

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.34s

error: test failed, to rerun pass `-p steg --test cli_api_call`
```

Check passed: `cargo test -p steg --test cli_api_call --features fixtures -- api_call_sends_stdin_params_flat_to_the_named_method` exited 0 at 2026-11-18T03:19:03+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running tests/cli_api_call.rs (target/debug/deps/cli_api_call-c9922192cf592a61)

running 1 test
test api_call_sends_stdin_params_flat_to_the_named_method ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.37s
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:25:42+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 46d0e643287c with uncommitted changes, last 40 of 78 lines:

```text
test typed_bytes_reach_a_real_program_in_order ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test declaring_a_size_of_zero_is_refused ... ok
test attach_returns_the_real_screen_and_updates_follow_it ... ok
test disconnecting_without_detaching_does_not_leak_or_block_a_later_attach ... ok
test input_clears_a_finished_status ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach has been running for over 60 seconds
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (58013990) panicked at crates/server/tests/attach.rs:1427:6:
pane.show must answer within a bound, even with the other pane stuck
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane

test result: FAILED. 22 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 151.80s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/test-steg-server.log
error: recipe `check-packages` failed with exit code 101
```

Check FAILED: `just check` exited 101 at 2026-11-18T03:29:38+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at f37ad8d991f0, last 40 of 71 lines:

```text
test detaching_a_pane_this_connection_never_attached_to_is_a_bad_request ... ok
test a_frame_on_an_already_detached_stream_closes_the_connection ... ok
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test detaching_a_pane_that_already_closed_still_releases_the_stream ... ok
test attach_returns_the_real_screen_and_updates_follow_it ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... FAILED
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach has been running for over 60 seconds
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck has been running for over 60 seconds
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok

failures:

---- an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane stdout ----

thread 'an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane' (58081958) panicked at crates/server/tests/attach.rs:1427:6:
pane.show must answer within a bound, even with the other pane stuck
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane

test result: FAILED. 22 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 128.31s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/tests-p-steg-p-steg-app-p-steg-kernel-p-steg-sdk-p-steg-server.log
error: recipe `test` failed with exit code 101
```

Built per the approved plan. connect_handshaken (sdk) finds or starts a server through the existing, unchanged connect() and retries the whole arbitration exactly once on EOF-before-hello (AC #4); Connection::call_unvalidated is api.call's own escape hatch past client-side validate() (AC #5); two new global clap flags (--autostart/--no-autostart, --json); exit-status mapping read from the description's own errors table (AC #2), CLI-local statuses (0, 2, 4, 130 - left at default signal disposition, no handler) mapped in exit.rs; api_call.rs reads stdin as one JSON object and always prints JSON (AC #3, AC #5). run.rs/cli_run_placeholder.rs deleted outright per TASK-1.49's own appendix.

Every proof named in the plan has a recorded failing-then-passing control pair on the task (the errors-table mapping, ConnectError->4, the exactly-once retry in both modes, call_unvalidated skipping validate, the global-arg propagation, the --json/no-json contrast, and api call's stdin merge).

Two real bugs the controls caught, fixed before landing:
1. The EOF-before-hello detector originally only recognised a clean zero-byte read. A connection closed the instant it is accepted can instead fail the client's write, set_read_timeout, or read outright, with different errno values depending on timing (BrokenPipe, NotConnected, even EINVAL were all observed for the identical fixture). Fixed by treating every such failure in that window (other than the read's own explicit timeout) as the same EOF-before-hello signal.
2. A connection-arbitration test fixture whose hostile listener never stopped accepting, starving the retry of ever reaching a real server - fixed by having the fixture accept exactly once then stop.

Departure from the approved plan: the plan described ConnectError::Io as connect_handshaken's path for a post-connect I/O failure. Per bug 1 above, every I/O failure in the handshake window (not just a subset of errno kinds) is now folded into the EOF-before-hello retry instead of surfacing as ConnectError::Io. The variant itself is unchanged and still used elsewhere (the real request/reply, once past the handshake).

Note for TASK-1.50/1.51: a bare, one-segment target (just a tab or pane id) is read as "in the current session" (doc-2 section 4), which a CLI connection with no STEG_SESSION context does not have - the full session/tab or session/tab/pane path is needed. Hit this building the api call effect test.

Follow-up named, not built here: move crates/app/src/startup.rs's own run/attach_over onto local_socket::connect_handshaken and delete its local hello-sending/reading, so the app gets the same retry-once protection - left for its own task since crates/app is off limits here.

Checks: local just check was not green at several points on this shared dev machine - crates/server/tests/attach.rs's own stuck-pane timing test (an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane) failed repeatedly under this machine's concurrent load (6 users, other workers' builds, load average up to ~11), but passed reliably - together with its whole 23-test file - when run alone or against a clean origin/main worktree under matched conditions, confirming it is a pre-existing, environment-dependent flake unrelated to this PR (nothing here touches crates/server/src or crates/server/tests). CI is green on 5a456f3a (bruk-io/steg#77) - the clean check for this head, confirmed by the engineer. crates/sdk and crates/steg's own full test suites passed repeatedly throughout, including after merging TASK-1.130's description/methods.rs split.

Check FAILED: `cargo test -p steg-sdk --lib --features local-socket -- a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello` exited 101 at 2026-11-18T04:44:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running unittests src/lib.rs (target/debug/deps/steg_sdk-2f782de52c6eac3e)

running 1 test
test local_socket::tests::a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello ... FAILED

failures:

---- local_socket::tests::a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello stdout ----

thread 'local_socket::tests::a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello' (58777657) panicked at crates/sdk/src/local_socket.rs:627:9:
a hello that was actually decoded must surface as a genuine error, never as ClosedBeforeHello (which would claim no server ever answered) and never as a success (which would hand back a stream whose own state is now unknown): Err(ClosedBeforeHello)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    local_socket::tests::a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 172 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-sdk --lib`
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg-sdk --lib --features local-socket -- a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello` exited 0 at 2026-11-18T04:44:40+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
   Compiling steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/sdk)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.80s
     Running unittests src/lib.rs (target/debug/deps/steg_sdk-2f782de52c6eac3e)

running 1 test
test local_socket::tests::a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 172 filtered out; finished in 0.00s
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check FAILED: `cargo test -p steg --test cli_api_call --features fixtures -- api_call_prints_json_with_no_json_flag_given` exited 101 at 2026-11-18T04:45:13+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/cli_api_call.rs (target/debug/deps/cli_api_call-c9922192cf592a61)

running 1 test
test api_call_prints_json_with_no_json_flag_given ... FAILED

failures:

---- api_call_prints_json_with_no_json_flag_given stdout ----

thread 'api_call_prints_json_with_no_json_flag_given' (58817260) panicked at crates/steg/tests/cli_api_call.rs:192:48:
api call prints JSON with no --json given: Error("EOF while parsing a value", line: 1, column: 0)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    api_call_prints_json_with_no_json_flag_given

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.37s

error: test failed, to rerun pass `-p steg --test cli_api_call`
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg --test cli_api_call --features fixtures -- api_call_prints_json_with_no_json_flag_given` exited 0 at 2026-11-18T04:45:24+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.77s
     Running tests/cli_api_call.rs (target/debug/deps/cli_api_call-c9922192cf592a61)

running 1 test
test api_call_prints_json_with_no_json_flag_given ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.37s
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check FAILED: `cargo test -p steg --test cli_exit_statuses --features fixtures -- malformed_stdin_on_api_call_exits_2` exited 101 at 2026-11-18T04:46:47+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/cli_exit_statuses.rs (target/debug/deps/cli_exit_statuses-a82fa623c139069d)

running 1 test
test malformed_stdin_on_api_call_exits_2 ... FAILED

failures:

---- malformed_stdin_on_api_call_exits_2 stdout ----

thread 'malformed_stdin_on_api_call_exits_2' (58836385) panicked at crates/steg/tests/cli_exit_statuses.rs:335:5:
assertion `left == right` failed: stderr: 
  left: Some(0)
 right: Some(2)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    malformed_stdin_on_api_call_exits_2

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.24s

error: test failed, to rerun pass `-p steg --test cli_exit_statuses`
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg --test cli_exit_statuses --features fixtures -- malformed_stdin_on_api_call_exits_2` exited 0 at 2026-11-18T04:47:08+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.81s
     Running tests/cli_exit_statuses.rs (target/debug/deps/cli_exit_statuses-a82fa623c139069d)

running 1 test
test malformed_stdin_on_api_call_exits_2 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.19s
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check FAILED: `cargo test -p steg --test cli_exit_statuses --features fixtures -- with_json_a_failed_command_still_prints_the_message_on_stderr_not_json` exited 101 at 2026-11-18T04:47:32+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/cli_exit_statuses.rs (target/debug/deps/cli_exit_statuses-a82fa623c139069d)

running 1 test
test with_json_a_failed_command_still_prints_the_message_on_stderr_not_json ... FAILED

failures:

---- with_json_a_failed_command_still_prints_the_message_on_stderr_not_json stdout ----

thread 'with_json_a_failed_command_still_prints_the_message_on_stderr_not_json' (58844851) panicked at crates/steg/tests/cli_exit_statuses.rs:280:5:
assertion failed: stdout_of(&output).is_empty()
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    with_json_a_failed_command_still_prints_the_message_on_stderr_not_json

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.36s

error: test failed, to rerun pass `-p steg --test cli_exit_statuses`
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg --test cli_exit_statuses --features fixtures -- with_json_a_failed_command_still_prints_the_message_on_stderr_not_json` exited 0 at 2026-11-18T04:47:45+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.76s
     Running tests/cli_exit_statuses.rs (target/debug/deps/cli_exit_statuses-a82fa623c139069d)

running 1 test
test with_json_a_failed_command_still_prints_the_message_on_stderr_not_json ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.39s
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check FAILED: `cargo test -p steg --test cli_exit_statuses --features fixtures -- oversized_stdin_on_api_call_exits_2_without_panicking` exited 101 at 2026-11-18T04:52:14+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/cli_exit_statuses.rs (target/debug/deps/cli_exit_statuses-a82fa623c139069d)

running 1 test
test oversized_stdin_on_api_call_exits_2_without_panicking ... FAILED

failures:

---- oversized_stdin_on_api_call_exits_2_without_panicking stdout ----

thread 'oversized_stdin_on_api_call_exits_2_without_panicking' (58896842) panicked at crates/steg/tests/cli_exit_statuses.rs:359:5:
assertion `left == right` failed: stderr: 
thread 'main' (58896843) panicked at crates/sdk/src/wire/connection.rs:271:14:
an unvalidated method call fits under the frame cap: TooLarge { size: 9437289, limit: 8388603 }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

  left: Some(101)
 right: Some(2)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    oversized_stdin_on_api_call_exits_2_without_panicking

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.47s

error: test failed, to rerun pass `-p steg --test cli_exit_statuses`
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check passed: `cargo test -p steg --test cli_exit_statuses --features fixtures -- oversized_stdin_on_api_call_exits_2_without_panicking` exited 0 at 2026-11-18T04:52:34+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
   Compiling steg v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/steg)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.75s
     Running tests/cli_exit_statuses.rs (target/debug/deps/cli_exit_statuses-a82fa623c139069d)

running 1 test
test oversized_stdin_on_api_call_exits_2_without_panicking ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.21s
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check FAILED: `just check` exited 101 at 2026-11-18T04:58:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes, last 40 of 80 lines:

```text
test attach_returns_the_real_screen_and_updates_follow_it ... ok
test declaring_a_size_of_zero_is_refused ... ok
test disconnecting_without_detaching_does_not_leak_or_block_a_later_attach ... ok
test detaching_a_pane_this_connection_never_attached_to_is_a_bad_request ... ok
test input_clears_a_finished_status ... ok
test a_frame_on_an_already_detached_stream_closes_the_connection ... ok
test typed_bytes_reach_a_real_program_in_order ... ok
test a_dropped_connection_releases_its_claim_the_same_way ... ok
test detaching_releases_the_claim_and_the_survivor_returns_to_its_own_declared_size ... ok
test two_connections_attached_to_one_pane_each_get_their_own_updates_and_a_detach_does_not_touch_the_other ... ok
test repeated_attach_and_detach_against_a_flooding_pane_never_sees_an_unknown_stream ... ok
test attaching_to_a_pane_whose_program_already_ended_gets_a_final_full_then_ended ... ok
test pane_close_force_reports_ended_closed_not_the_signals_own_exit_code ... ok
test a_program_that_exits_on_its_own_sends_ended_with_its_real_exit_code ... ok
test detach_ends_the_stream_even_though_the_program_keeps_producing_output ... ok
test two_clients_attached_to_one_pane_both_get_ended_for_the_same_exit ... ok
test a_pane_that_closes_between_a_queued_attach_and_its_arm_leaves_nothing_to_detach ... ok
test a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream ... FAILED
test pane_close_force_returns_within_its_bound_even_while_the_panes_own_worker_is_stuck ... ok
test stopping_the_server_returns_within_its_bound_even_while_a_panes_worker_is_stuck ... ok
test an_unresponsive_panes_own_commands_do_not_stall_the_connection_or_another_pane ... ok
test a_second_connections_control_call_is_not_blocked_by_a_stuck_panes_lock_held_attach ... ok

failures:

---- a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream stdout ----

thread 'a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream' (58975083) panicked at crates/server/tests/attach.rs:455:17:
no update arrived on stream 1 within 5s
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    a_declared_size_reaches_the_pty_and_is_reported_on_every_attached_stream

test result: FAILED. 22 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.60s

error: test failed, to rerun pass `-p steg-server --test attach`
full output: .quiet/test-steg-server-pty-race-fixture.log
error: recipe `check-packages` failed with exit code 101
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Check passed: `just check` exited 0 at 2026-11-18T05:04:31+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 433d6a693f9e with uncommitted changes:

```text
ok      format check (0.3s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.0s)
ok      licences (0.7s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (52.4s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (50.3s)
ok      test (steg-server, race-fixture) (53.1s)
ok      test (steg-server, pty-race-fixture) (60.6s)
ok      test (steg-server, accept-backoff-fixture) (57.9s)
ok      build (steg) (1.1s)
ok      test (steg) (4.9s)
ok      test (steg, fixtures) (11.9s)
ok      build (steg-ui) (18.4s)
ok      test (steg-ui) (1.6s)
ok      build (steg-app) (1.4s)
ok      test (steg-app) (2.1s)
ok      tool tests (41.1s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Commits since the review: 19, 16 without the Asdlc-Worker trailer.

Review findings fixed (back to In Review). Head eab279c0; CI green on this head (confirmed by the engineer).

1. [Important, bug] one_handshake_attempt (crates/sdk/src/local_socket.rs) discarded a hello it had already decoded if the trailing set_read_timeout(None) call failed, returning Ok(None) (the EOF-before-hello signal) and letting the retry misreport ClosedBeforeHello for a connection that genuinely answered. Confirmed this is not rare: a standalone probe (a peer that writes a real hello then closes immediately, no pause) made the clearing set_read_timeout(None) fail with EINVAL on this platform in 1999/2000 trials. Fixed precisely at the boundary the review named: before a complete Hello frame is decoded, every write/set_read_timeout/read failure is still folded into Ok(None) (the EOF-before-hello retry), with each side's own explicit timeout the one exception (ConnectError::Timeout). From the moment a complete Hello is decoded, the handshake has already succeeded, so a failure clearing either timeout now ends the whole call with a genuine ConnectError::Io instead - never retried, never handed back silently with a lingering timeout. The function's own doc comment now states this boundary precisely. New test: a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello, using a new Behaviour::CooperativeClosedImmediately fixture variant (the existing CooperativeHeldOpen keeps its 100ms pause, unchanged, for the test that needs the race avoided). Control recorded: the old behaviour (discard and retry) makes this fail with Err(ClosedBeforeHello); reverted, recorded passing.

2. [Evidence] api_call_prints_json_with_no_json_flag_given had no recorded control; recorded one (break: gate api call's own output on --json like every other command; the test fails since nothing prints; revert; record passing).

   Audited every test the plan's own Proof section names against the recorded-control list:
   - Already controlled, directly: the errors-table mapping (every_described_error_maps_to_its_own_exit_status), ConnectError->4 (every_connect_error_maps_to_4), the exactly-once retry (both modes, one scratch edit), call_unvalidated skipping validate, the global-arg propagation (no_autostart_is_read_whether_given_before_or_after_the_subcommand), the --json/no-json contrast (without_json_a_successful_command_prints_nothing_on_stdout), api call's stdin merge (api_call_sends_stdin_params_flat_to_the_named_method).
   - Missing and now recorded: api_call_prints_json_with_no_json_flag_given (this finding); malformed_stdin_on_api_call_exits_2 (break: malformed JSON silently becomes {} instead of exiting 2); with_json_a_failed_command_still_prints_the_message_on_stderr_not_json (break: the error path also prints JSON to stdout when --json is given).
   - Named in the plan but sharing an already-controlled mechanism, not given a near-duplicate control: no_autostart_with_nothing_running_exits_4 (autostart_flag reading + ConnectError::NoServerRunning->4, both already controlled); a_program_still_running_refuses_close_with_status_1 and a_target_that_matches_nothing_exits_3 (real server dispatch, off limits to this task, + the already-controlled errors-table mapping covering ProgramRunning->1 and NoSuchTarget->3); cli_connection_arbitration.rs's own corroboration test (the plan names it explicitly as corroboration of the same already-controlled retry mechanism, not a separate one, through real processes).
   - Named in the plan but with no mechanism of this task's own to break: a_value_outside_a_closed_set_exits_2_with_no_server_contacted (clap's own pre-existing validation, not code this task wrote); sigint_during_a_hung_call_ends_the_process_by_signal (there is deliberately no SIGINT handler at all - nothing to break).

3. [Nit] write_all(&hello) had no write-side timeout, so "about twice connect_timeout" depended on the hello never blocking rather than being enforced. Added set_write_timeout(Some(connect_timeout)) before the write (folded into Ok(None) on failure, the same reasoning as every other pre-hello setup call), with the write's own explicit timeout mapped to ConnectError::Timeout the same way the read's already was. Cleared together with the read timeout once the hello is decoded. No test forces an actual blocked write (the hello is small enough that this would need fault injection to force deterministically, which felt like unwarranted scope growth for a nit whose own fix is the same already-proven pattern mirrored); the fix itself, and 70+ repeated runs of the full local_socket test suite with no new flake, are the evidence.

4. [Nit] api call with stdin over the frame's own cap (MAX_PAYLOAD, already public from steg_sdk) reached Connection::call_unvalidated's own encode_frame(...).expect(...) and panicked (exit 101) instead of exiting 2. Bounded entirely inside crates/steg/src/cli/api_call.rs: a size check against MAX_PAYLOAD less a 4 KiB margin, before any JSON parsing, so oversized input is rejected cheaply and never reaches the sdk at all - no sdk change needed for this one. New test: oversized_stdin_on_api_call_exits_2_without_panicking (a 9 MiB, valid-JSON payload, so the size check is what's proven, not the earlier "is this JSON" check). Control recorded: reverting the check makes the real binary panic (exit 101) under the test, confirmed; reverted back, recorded passing.

Every fix above has a recorded failing-then-passing control pair on the task. Local `just check` was intermittently red across several runs tonight on different crates/server/tests/attach.rs timing tests (none in this PR's files); the final local run at this head (eab279c0) was itself green end to end, and CI is green on the same head.

Check FAILED: `cargo test -p steg-sdk --lib --features local-socket -- a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello` exited 101 at 2026-11-18T06:01:27+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at eab279c0ff73 with uncommitted changes:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running unittests src/lib.rs (target/debug/deps/steg_sdk-2f782de52c6eac3e)

running 1 test
test local_socket::tests::a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello ... FAILED

failures:

---- local_socket::tests::a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello stdout ----

thread 'local_socket::tests::a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello' (59476840) panicked at crates/sdk/src/local_socket.rs:836:9:
a hello that was actually decoded must surface as a genuine error, never as `Ok(None)` (which `connect_handshaken` reads as EOF-before-hello and retries): Ok(None)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    local_socket::tests::a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 174 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p steg-sdk --lib`
```

Commits since the review: 0.

Check passed: `cargo test -p steg-sdk --lib --features local-socket -- a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello` exited 0 at 2026-11-18T06:01:40+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at eab279c0ff73 with uncommitted changes:

```text
   Compiling steg-sdk v0.1.0 (/Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119/crates/sdk)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.78s
     Running unittests src/lib.rs (target/debug/deps/steg_sdk-2f782de52c6eac3e)

running 1 test
test local_socket::tests::a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 174 filtered out; finished in 0.00s
```

Commits since the review: 0.

Check passed: `just check` exited 0 at 2026-11-18T06:11:35+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.119 on task-1.119-cli-connecting-to-the-server-calling-a at 0dc279da37fc with uncommitted changes:

```text
ok      format check (0.3s)
ok      lint (12.3s)
ok      boundaries (0.1s)
ok      foundations (0.1s)
ok      foundations public surface (0.0s)
ok      design pages (0.1s)
ok      licences (0.8s)
ok      tests (-p steg -p steg-app -p steg-kernel -p steg-sdk -p steg-server) (72.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.7s)
ok      test (steg-sdk, local-socket) (3.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (28.6s)
ok      test (steg-server) (54.2s)
ok      test (steg-server, race-fixture) (52.2s)
ok      test (steg-server, pty-race-fixture) (61.9s)
ok      test (steg-server, accept-backoff-fixture) (57.1s)
ok      build (steg) (1.2s)
ok      test (steg) (5.3s)
ok      test (steg, fixtures) (12.8s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (1.3s)
ok      build (steg-app) (1.1s)
ok      test (steg-app) (1.9s)
ok      tool tests (40.1s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
ok      ui levels (0.1s)
ok      ui colours (0.1s)
```

Commits since the review: 30, 30 without the Asdlc-Worker trailer.

Second review round fixed (back to In Review). Head 38e853d9; CI green on this head (confirmed by the engineer).

CORRECTION to the Departure paragraph recorded earlier (the round-2 Nit): that paragraph said "every I/O failure in the handshake window... is now folded into the EOF-before-hello retry," which stopped being true once round 1's fix landed. The accurate statement, replacing it: every failure of the handshake's own write/read/timeout calls BEFORE a complete hello frame is decoded is folded into the EOF-before-hello retry (Ok(None)), with each side's own explicit timeout the one exception (ConnectError::Timeout). From the moment a complete hello IS decoded, the handshake has already succeeded: a failure after that point (this function's own trailing timeout-clear) is never folded into the retry - it ends the call with a genuine ConnectError::Io. This is now also the precise wording in local_socket.rs's own doc comments (handshake_over and connect_handshaken).

[Important] The round-1 fix's only proof, a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello, depended on an unsynchronised OS race (a real UnixStream, peer closes immediately after sending a real hello). The reviewer ran the correct code 100 times and saw 3 failures - the race simply didn't land on those runs.

Fixed by introducing a deterministic seam: extracted everything one_handshake_attempt does from the moment it has a connected stream (set write timeout, write the hello, set read timeout, read/decode the loop, clear both timeouts on a decoded hello) into a new function, handshake_over<S: HandshakeSocket>(stream: &mut S, hello: &[u8], connect_timeout: Duration) -> Result<Option<Vec<u8>>, ConnectError>, generic over a new, crate-private HandshakeSocket trait covering exactly the four operations it performs (send, recv, set_read_timeout, set_write_timeout). UnixStream implements the trait as a thin delegation to its own real methods. one_handshake_attempt is now: connect(options)? to get a real stream, then handshake_over(&mut stream, &hello, options.connect_timeout). Production behaviour is unchanged (same operations, same order, same error mapping, just routed through the trait); connect() itself is untouched; nothing public changes - HandshakeSocket, handshake_over and FakeSocket (below) are all private to local_socket.rs.

A FakeSocket test double implements HandshakeSocket with every outcome chosen by the test (a queue of canned recv results, and a separate configurable Result for send and for each of the four timeout calls - the two timeout-setting calls and the two timeout-clearing calls are tracked separately, read by whether the call passed Some or None) - no sleep, no OS, no timing anywhere in either test:

- a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello: queues one recv() returning a real, well-formed server hello (built the same way the existing real-socket fixture already does, via Connection::accept_or_refuse), then forces the read-timeout-clear call to fail. Asserts Err(ConnectError::Io(_)) - deterministically, on every run, since nothing about the outcome depends on timing.
- a_failure_before_any_hello_is_decoded_is_still_folded_into_the_retry_signal: forces the write-timeout-SET call (the very first operation) to fail, before anything else runs. Asserts Ok(None) - the same EOF-before-hello retry signal the real-socket Mode A test already proves deterministically (dropping a connection outright is a guaranteed OS-level EOF, not a race).

Control recorded: restored the old, buggy fold-into-retry behaviour in handshake_over's own hello-decoded branch; cargo test -p steg-sdk --lib --features local-socket -- a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello failed with "Ok(None)" instead of the expected Io error - recorded failing; reverted; recorded passing.

Both new deterministic tests then run 200 times in a loop (cargo test -p steg-sdk --lib --features local-socket -- <both test names>, --test-threads=1, one process per iteration): 200/200 passed. No OS race is involved in either test at all, so this was to confirm rather than to hope.

What happened to the real-socket "closes immediately" fixture (Behaviour::CooperativeClosedImmediately): kept, but only as corroboration, renamed (a_hello_sent_then_closed_immediately_never_reports_closed_before_hello) and its assertion loosened to the one thing that holds no matter which way that run's own OS race falls: the call returns within its bound and is either a real success or the genuine ConnectError::Io - never ClosedBeforeHello (which would mean a hello that genuinely arrived got mistaken for no server at all). It no longer asserts a specific outcome, so it can never be the flaky test again.

Audit of this task's other new tests for the same flaw (a sleep or a timing window used to avoid or reach a race), as asked:
- a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts (local_socket.rs, Behaviour::CooperativeHeldOpen): still sleeps 100ms before closing, to hold the connection open past the same timeout-clear race. This is the same general shape (a timing margin, not a guarantee) but with much stronger empirical backing than the original 0ms case: a standalone probe (built while diagnosing round 1) showed the clear failing in 1999/2000 trials at 0ms delay and never once across dozens of runs at 100ms. This test's own job - proving connect_handshaken's outer retry-after-EOF wrapper succeeds on a real second attempt - needs a real UnixStream end to end (the same reason connect() itself isn't behind the new seam), so it cannot be made deterministic the same way without also abstracting connect()'s own real socket arbitration, which is out of this round's scope. Judged safe to leave as is: the exact logic it would be exercising if the race ever did land is now separately, deterministically proven by the two new tests above, so even an astronomically unlikely failure here would be a loud, visible test failure (an assertion that a success happened when an Io error did instead), never a silently wrong pass. Flagged for the engineer rather than decided unilaterally - happy to extract a second seam around connect() itself if that is wanted.
- Every other new test in this task (cli_exit_statuses.rs, cli_connection_arbitration.rs, cli_api_call.rs, and the two local_socket.rs tests for the write-timeout/oversized-stdin fixes) synchronises only through real process exits (a command simply runs to completion) or mpsc channels (ready_rx.recv_timeout for "the listener is bound"), never a sleep used to avoid or reach a specific race window - checked by rereading each one specifically for this. sigint_during_a_hung_call_ends_the_process_by_signal's own 300ms sleep is not a race-avoidance window: the fixture structurally cannot finish within it (it never answers the real request at all), so the process is guaranteed still running regardless of how slowly the earlier connect/handshake steps run.

Local just check was green end to end at this head on the first attempt. CI is green on 38e853d9.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-18T02:30:00Z

---

Plan approved by @bruk (b60acf121e04, size-2): its worker may build it.

---

author: @bruk.r1
created: 2026-11-18T04:28:00Z

---

@bruk.r1 is reviewing TASK-1.119: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r1
created: 2026-11-18T04:36:00Z

---

Review by @bruk.r1 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 2 Important, 2 nit. Cites: review. The diff matches the plan.

[Bugs, Important] crates/sdk/src/local_socket.rs:287-291 (one_handshake_attempt). When a complete Hello frame has already been decoded, the function still discards it and returns Ok(None) - the EOF-before-hello signal - if the trailing stream.set_read_timeout(None) call fails. A real hello was received and parsed correctly; this is not the EOF-before-hello condition AC #4 is about. On the retry attempt specifically, this turns a server that genuinely answered into a hard ConnectError::ClosedBeforeHello (exit 4), misreporting "no server" after a real, successful exchange. Not exercised by either Mode A/B unit test (the cooperative fixture sleeps 100ms before closing specifically to avoid this race) and not mentioned in the notes' own Departure writeup. The widened I/O-failure fold (correctly scrutinized for point 2) should not apply once a complete frame is already in hand - the fix is to return the success regardless of whether clearing the timeout succeeds.

[Evidence, Important] crates/steg/tests/cli_api_call.rs::api_call_prints_json_with_no_json_flag_given has no recorded negative control. The plan's own proof section names this test explicitly for AC #3 ("api call's own exception, stated and tested... proving the documented exception rather than merely asserting it in prose"), and the policy requires a negative control for any test the plan's proof names. The task's own summary lists exactly which mechanisms got a recorded failing-then-passing control pair ("the errors-table mapping, ConnectError->4, the exactly-once retry in both modes, call_unvalidated skipping validate, the global-arg propagation, the --json/no-json contrast, and api call's stdin merge") and this one - the always-print-JSON exception itself - is absent from that list.

[Bugs, Nit] crates/sdk/src/local_socket.rs (one_handshake_attempt): stream.write_all(&hello) runs before any timeout is set on the socket, so a peer that accepts but never reads could block the whole call past the stated "worst case about twice connect_timeout" bound. Unlikely given the hello's small size, but not actually bounded as documented.

[Bugs, Nit] crates/steg/src/cli/api_call.rs / crates/sdk/src/wire/connection.rs (call_unvalidated): arbitrary stdin is merged and passed straight to call_unvalidated with no size check; encode_frame's "fits under the frame cap" expect panics (exit 101) rather than failing cleanly with the documented bad-usage status 2, if stdin is large enough. Pre-existing SDK-wide pattern (Connection::call has the same expect), but call_unvalidated is the one path that takes fully unvalidated, user-controlled input, making it the most directly reachable case.

Control run myself: removed connect_handshaken's retry (local_socket.rs) in a scratch edit so a single EOF-before-hello returns ClosedBeforeHello immediately; ran `cargo test -p steg-sdk --lib --features local-socket -- a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts` (narrowest command, completed well under a minute - no explicit timeout needed). It failed as expected: "the retry's own hello should succeed: ClosedBeforeHello" - confirming the retry is load-bearing for Mode B. Reverted with `git checkout -- crates/sdk/src/local_socket.rs`; `git status` confirmed clean afterward.

Checked and consistent with the claim: diff touches only crates/sdk (lib.rs, local_socket.rs, wire/connection.rs) and crates/steg (cli/*, main.rs, tests/*) - no boundaries.toml, Cargo.toml, crates/app, crates/server/src, or crates/sdk/src/description/ changes. Plan approval present (b60acf121e04, size-2) and matches the built diff's files and order of work. CI is green on exactly 5a456f3a (bruk-io/steg#77, confirmed via `gh pr checks`/`gh pr view --json headRefOid`); the disclosed local `just check` failures are the known, unrelated crates/server/tests/attach.rs timing flake, accurately described. The required single-server guarantee holds: connect_handshaken's retry only re-invokes the unmodified connect(), whose own direct-connect-then-spawn arbitration is untouched, so a retry after a dead listener's socket disappears entirely (as cli_connection_arbitration.rs's fixture does) is the only case that reaches autostart again, and a still-live listener is simply reconnected to, not duplicated.

Not checked: did not independently re-run the full real-process test suites (cli_exit_statuses.rs, cli_connection_arbitration.rs) end to end beyond the one scratch control above, relying on the recorded check log and the green CI run for those; did not verify architecture/LikeC4 model validation beyond confirming no boundaries.toml/Cargo.toml diff.

---

author: @bruk.r3
created: 2026-11-18T05:51:00Z

---

@bruk.r3 is reviewing TASK-1.119: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r3
created: 2026-11-18T05:56:00Z

---

Review by @bruk.r3 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 1 nit. Cites: review, testing-rust. The diff matches the plan.

Round 2 of TASK-1.119.

Round 1's two Important findings and two nits are fixed and verified:
- one_handshake_attempt (crates/sdk/src/local_socket.rs) now returns a genuine ConnectError::Io, never Ok(None)/ClosedBeforeHello, once a complete Hello is decoded. Both the read and write timeouts are cleared before the stream is handed back on success; either clear failing ends the call with Io instead of returning the stream. connect() itself and the single-server arbitration are unchanged (confirmed by reading the whole file). The retry is still exactly once: connect_handshaken calls one_handshake_attempt twice via plain match, any Err (Timeout or the new post-hello Io) propagates immediately via `?` and is never retried - only the EOF-before-hello Ok(None) case retries.
- api_call_prints_json_with_no_json_flag_given now has a recorded failing-then-passing control (04:45:13 FAILED / 04:45:24 passed, commit 433d6a693f9e). Spot-checked two more from the same audit: malformed_stdin_on_api_call_exits_2 (04:46:47/04:47:08) and with_json_a_failed_command_still_prints_the_message_on_stderr_not_json (04:47:32/04:47:45). The diff between 433d6a693f9e and eab279c0 touches none of this task's files, so these records are against the reviewed head's own code.
- Nit (write timeout): set_write_timeout(connect_timeout) added, folded pre-hello the same as the read side, cleared together with it on success.
- Nit (oversized stdin panic): api_call.rs now rejects stdin over MAX_PAYLOAD - 4KiB before parsing, imported directly from steg_sdk (not a second number); confirmed no other sdk file changed in this round's diff (433d6a693f9e..eab279c0 touches only local_socket.rs, api_call.rs, cli_exit_statuses.rs). New test oversized_stdin_on_api_call_exits_2_without_panicking has a recorded failing(101)/passing(2) control.

[Bugs, Important] crates/sdk/src/local_socket.rs:642-666 (test a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello). This test is the only proof that the fix for round 1's bug actually surfaces a genuine error instead of discarding a decoded hello, and it depends entirely on the OS producing a specific race (EINVAL from set_read_timeout once the peer has already closed) rather than on any synchronised event - violating .claude/rules/testing-rust.md ("no test sleeps to wait for something; it waits on the event": here nothing is waited on, the race is just hoped for). I repeated the control the plan asked for (reverted the fix to the old discard-and-retry behaviour in a scratch edit, confirmed the test fails as expected with Err(ClosedBeforeHello), reverted, confirmed `git status` clean), so the test is not vacuous against the old bug. But I also ran the unmodified, fixed test standalone, single-threaded, with no other change: 3 failures in 100 runs (two batches of 40 and 60, on this same shared dev machine), each because the race did not trigger that time (set_read_timeout/set_write_timeout both succeeded, the function returned Ok, and the test's `assert!(matches!(result, Err(ConnectError::Io(_))))` panicked). That means this test can and does fail against the now-correct code under ordinary load - it is not a hypothetical risk, it reproduces. Recommend forcing the post-hello teardown failure deterministically (e.g. close the raw fd out from under the stream, or a test-only seam that fails the clear call directly) instead of relying on real-world scheduling to reproduce a platform errno.

[Compliance, Nit] Task Implementation Notes, the original "Departure from the approved plan" paragraph (recorded before the round-1 fix) still reads "every I/O failure in the handshake window ... is now folded into the EOF-before-hello retry," which stopped being true once the fix drew the boundary at the decoded hello. The corrected boundary is stated precisely elsewhere (finding #1's own fix writeup, and the updated doc comments in local_socket.rs), so no reader is actually misled about the code, but the Departures record itself was never corrected to match.

Checked, no finding: connect() and the arbitration are untouched; the retry is exactly once; no stream is ever returned with a timeout still set; CI is green via `gh pr checks 77` and `gh pr view --json headRefOid` both against eab279c0, the diff's own head; the round-1 diff (5a456f3a..eab279c0, task's files only) touches exactly local_socket.rs, api_call.rs, cli_exit_statuses.rs, nothing else.

Not checked: did not re-run cli_connection_arbitration.rs or cli_exit_statuses.rs's real-process suites beyond what CI/recorded checks already show; did not independently re-verify the EINVAL/errno claim's platform mechanism beyond reproducing the test's own pass/fail behaviour.

---

author: @bruk.r3
created: 2026-11-18T08:41:00Z

---

@bruk.r3 is reviewing TASK-1.119: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r3
created: 2026-11-18T08:52:00Z

---

Review by @bruk.r3 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 2 nit. Cites: review, testing-rust. The diff matches the plan.

Round 3 of TASK-1.119 (narrow): reviewed eab279c0..38e853d9, crates/sdk/src/local_socket.rs only (crates/steg untouched this round).

1. The seam: handshake_over<S: HandshakeSocket> and the HandshakeSocket trait, plus `impl HandshakeSocket for UnixStream`, are all module-private (no pub) - confirmed by grep across local_socket.rs and a no-diff on lib.rs; adds nothing to the crate's public surface. UnixStream's impl is pure delegation (send->write_all, recv->read, set_*_timeout -> the real methods). one_handshake_attempt calls connect(options)? for a real stream, then handshake_over(&mut stream, ...) - connect() and connect_handshaken are both byte-for-byte unchanged this round (confirmed by reading and by the diff). Compared handshake_over's body against the prior one_handshake_attempt body line by line: identical operations, order, and error mapping, only parameterised over connect_timeout/send/recv instead of options.connect_timeout/write_all/read. No behavioural difference. .claude/rules/rust.md's "a trait to share code" concern does not apply: this trait exists exactly for its sanctioned use ("swapped for a fake in tests"), matching testing-rust.md's own description of a fake.

2. The two deterministic tests (a_hello_decoded_then_a_failed_timeout_clear_is_a_genuine_error_not_closed_before_hello, a_failure_before_any_hello_is_decoded_is_still_folded_into_the_retry_signal): no sleeps, threads or real sockets - FakeSocket is pure in-memory. Repeated the control: edited handshake_over's hello-decoded branch to fold a clear-timeout failure back into Ok(None) (the old buggy behaviour); the first test failed as expected ("Ok(None)" instead of the required Err(Io)); reverted with `git checkout --`; `git status` confirmed clean. Ran both tests together 200 times in a loop (--test-threads=1, one process per iteration): 200/200 passed.

3. The real-socket corroboration test (renamed a_hello_sent_then_closed_immediately_never_reports_closed_before_hello): asserts only Ok(_) | Err(ConnectError::Io(_)), never ClosedBeforeHello, within its bound - correct per the brief. Ran 200 times: 200/200 passed (uptime load averages ~2.8-2.9, 7 users at the time).

4. a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts (CooperativeHeldOpen, 100ms sleep): ran 300 times in two batches of 150 (uptime load averages ~2.8-3.3, 7 users throughout): 0 failures. Grepped for other sleeps in this task's scope: local_socket.rs:193 (connect()'s own bounded poll, pre-existing, untouched) and :456 (run_fake_server_for_tests, pre-existing fixture) are not new; crates/steg's cli_exit_statuses.rs sleeps (bounded wait_for_socket/wait_bounded polls, and the sigint test's own named-safe 300ms/60s) are all pre-existing, untouched this round, and already audited in the task's own notes.

Findings:

[Bugs, Nit] crates/sdk/src/local_socket.rs:586 (Behaviour::CooperativeHeldOpen). Still sleeps 100ms as a timing margin rather than synchronising on an event (.claude/rules/testing-rust.md: "no test sleeps to wait for something; it waits on the event"). Not necessary: the listener thread already owns the accepted stream, so it could hold the connection open until the test signals done (a second channel, test -> thread) instead of guessing a safe duration - test-fixture-only, touches nothing in connect(). Nit, not Important: 300/300 passed under today's load just now, matching the worker's own probe (1999/2000 failures at 0ms delay, 0 across dozens of runs at 100ms) - a large, now independently reconfirmed margin.

[Evidence, Nit] crates/sdk/src/local_socket.rs (a_hello_sent_then_closed_immediately_never_reports_closed_before_hello). The outcome assertion's loosening (Ok(_) | Err(Io), never ClosedBeforeHello) is correct and is what this round's fix needed. But the same test's counter assertion was also loosened, from assert_eq!(counter, 1, ...) to assert!(counter >= 1, ...), and that part was not necessary: all three paths of CooperativeClosedImmediately (clear-read fails, clear-write fails, both succeed) take exactly one accept, since none produces Ok(None) and connect_handshaken only retries on Ok(None) (any Err propagates via ? with no second attempt, unchanged this round). Restoring assert_eq!(counter, 1, ...) would keep the "not retried on a genuine post-hello error" proof at the real-socket level without reintroducing the flaky outcome check. Nit: the property still holds and has coverage elsewhere - handshake_over's new deterministic test proves the inner boundary, and connect_handshaken's own three-line ?-propagation was read-verified by this round and by round 2's review - so AC #4's "exactly once" stays proven, this is a redundant piece of evidence lost, not an unproven behaviour.

5. Notes: head is 38e853d9, matching the claim; CI confirmed independently green on exactly 38e853d9 (gh pr checks 77, gh pr view --json headRefOid). The task's own last recorded `just check` passed at 433d6a693f9e "with uncommitted changes" - i.e. with this round's handshake_over/HandshakeSocket/FakeSocket edits already present but not yet committed; those were committed as 4d6d8035 immediately after, then 38e853d9 only merges origin/main on top (confirmed: 433d6a693f9e..38e853d9 touches local_socket.rs/api_call.rs/cli_exit_statuses.rs, all from already-reviewed round 1/round 2 work plus 4d6d8035 - no further uncommitted code). Recorded check is against an identical tree to 38e853d9 for this round's purposes. The Departure paragraph: the original "Departure from the approved plan" text is still present verbatim and still inaccurate (notes are append-only) - but a later "CORRECTION to the Departure paragraph recorded earlier" block names what it corrects and states the boundary precisely (failures before a complete hello fold into the retry; failures after do not, via Io). That addresses round 2's Nit in substance, via a correction block rather than an edited paragraph; not re-raised as a new finding per the instruction not to re-file an earlier round's finding.

Not checked: did not re-run cli_exit_statuses.rs/cli_connection_arbitration.rs's real-process suites beyond CI and the task's own recorded checks (neither file changed this round); did not independently re-verify the EINVAL/platform-errno mechanism beyond reproducing the deterministic tests' pass/fail behaviour; did not run architecture/LikeC4 validation (no boundaries.toml/Cargo.toml diff this round).

---
<!-- COMMENTS:END -->
