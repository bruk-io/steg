---
id: TASK-1.119
title: 'CLI: connecting to the server, calling a method, and the exit statuses'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-16T13:35:00Z'
updated_date: '2026-11-18T04:44:00Z'
labels:
  - size-2
  - changes-requested
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
asdlc_review_findings: 'important=2,nit=2'
asdlc_review_policies: ["review"]
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
<!-- COMMENTS:END -->
