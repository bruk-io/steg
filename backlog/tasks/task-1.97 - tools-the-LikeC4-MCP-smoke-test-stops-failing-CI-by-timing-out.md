---
id: TASK-1.97
title: 'tools: the LikeC4 MCP smoke test stops failing CI by timing out'
status: Done
assignee:
  - '@bruk.w5'
created_date: '2026-11-16T01:34:00Z'
updated_date: '2026-11-16T04:22:00Z'
labels:
  - size-2
  - review-passed
milestone: m-3
dependencies: []
references:
  - task-1.97-tools-the-likec4-mcp-smoke-test-stops
  - https://github.com/bruk-io/steg/pull/53
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.97-tools-the-likec4-mcp-smoke-test-stops
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '1b39eee49ff0'
asdlc_plan_approved_size: size-2
asdlc_review_kind: same-machine
asdlc_review_findings: 'important=0,nit=1'
asdlc_review_policies: ["review"]
asdlc_plan_matched: no
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pulled out of TASK-1.82 (housekeeping, m-0) on 2026-11-15 because it is costing a re-run on real pull requests while six tasks are in flight: tools/tests/adapter/test_likec4_mcp_smoke.py::test_a_session_can_list_this_repositorys_views_and_elements_through_the_mcp_server raises TimeoutError('likec4 mcp did not answer within 30.0s') from tools/src/steg_tools/likec4_mcp.py. Seen on the pull requests of TASK-1.40, TASK-1.76 and TASK-1.77 (twice, two heads), none of which touched tools/ or architecture/, green on re-run each time. doc-16 R108 is why the test exists (the architecture model is checked).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The cause of the timeouts is established from the failed runs' logs and a reproduction, not assumed: what the 30 seconds were spent on (downloading or starting the pinned likec4 through npx, the server indexing the model, a reply lost), on CI's runner and locally under load; the notes say which
- [x] #2 The fix addresses that cause: for example the tool is installed or cached before the test so no download happens inside the timed wait, or the test waits on the server's own readiness and not a fixed time. Raising the timeout alone is not a fix unless the cause is shown to be plain slowness with a bound, and then the bound is stated with its evidence. The test is not skipped, marked flaky or retried silently
- [x] #3 Shown by repeated runs: the test passes N times in a row on CI (a workflow run that repeats it, linked in the notes) and locally under load, with N chosen and justified against the failure rate seen so far (four failures on pull requests 40's, 76's task's, and 44 twice)
- [x] #4 If Node is not pinned in CI and that is part of the cause, pinning it is done here and removed from TASK-1.82's list
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
ONE reviewable change. Every edit below serves a single cause: npx always makes a live npm-registry round trip, and CI's runners never carry a warm npm cache, so that round trip lands inside the smoke test's 30s wait. The Node pin (AC #4) is folded in because it's near-free through the mise mechanism already wired in ci.yml, not because it is itself the cause (evidence below says it isn't).

CAUSE (established from logs + reproduction, not assumed)
- Failed runs (job 133035963248, run 43786738602, PR 44 attempt1; job for run 43782519189, PR 44's other head; both confirmed via `gh api .../actions/jobs/<id>/logs --allow-escape-sequences`): "tool tests" step takes 34.2s / 31.9s, with pytest itself reporting "1 failed, 51 passed in 33.17s" - i.e. the whole excess is the one TimeoutError at 30.0s in `likec4_mcp.py:147`, not a slow pytest collection or other test.
- Passing runs (e.g. job 133045320293): "tool tests" (all 52 tests, including the smoke test) takes 21.4s, and the *later* `check-architecture` step - which does its own `npx --yes likec4@1.61.0 validate` - takes only 1.7s. That gap (21.4s vs 1.7s) is the signature of a cost paid once per job, inside the first npx call, not repeatable within the same job once npm's own cache is warm.
- Reproduced the mechanism directly: cold npm cache, single `npx --yes likec4@1.61.0 --version`: 9.7-13.4s (3 trials). Same cold cache under 4-way concurrent contention (3 background cold npx fetches running at once, mimicking "four CI runs queued at once" - TASK-1.82 comment #2): rose to 22.1-22.9s across all 4 processes. This did not by itself exceed 30s locally, so I could not reproduce the exact 31-34s CI numbers through download time alone - said plainly, not papered over.
- The number that explains the rest: warm cache (package already resolved and on disk) + registry unreachable (`HTTP_PROXY=http://127.0.0.1:1`), plain `npx --yes likec4@1.61.0 --version`, NO_UPDATE_NOTIFIER=1 set: still took 70.7s. The npm debug log (`_logs/*-debug-0.log`) shows why: npm issues `GET https://registry.npmjs.org/likec4` (and `.../npm`) and retries 3 times with backoff before giving up - a live resolution call happens on *every* npx invocation regardless of a warm package cache, and its own retry ladder is what can push total wall time well past 30s under registry slowness or congestion, which is exactly the condition ("four CI runs queued at once") recorded against the real occurrences.
- Confirmed the fix mechanism: same warm cache, same unreachable registry, `npx --prefer-offline --yes likec4@1.61.0 --version`: 0.43s, zero network calls attempted. `--prefer-offline` with a *cold* cache and a real network still downloads normally (11.7s, unchanged) - so the flag is safe to leave on always; it only matters when the cache is already warm.
- Ruled out "the mcp server started but a reply was lost": forced `timeout=0.5` and `timeout=8.0` locally against a cold cache and read `exc.__notes__` - empty both times, same as the real CI failures (no "likec4 mcp stderr:" note in the logs). Confirmed `add_note`/stderr-capture itself works and would show content when npm actually writes any (forced an ETARGET error with a bogus version: the note appeared with the real npm error text). npm writes nothing to stderr during its own resolve/fetch phase when not attached to a tty, so the empty note is consistent with "time spent before the child's own code starts," not contradicting it.
- Node is not pinned anywhere today: not in `mise.toml` ([tools] only has `zig`), not in `.github/workflows/ci.yml` (no setup-node, no version check step). Node's version was not shown to be part of the cause above - the delay is in npm's own registry-retry behaviour, which the evidence above isolates independent of Node's version. Noting this plainly per AC #4's own condition; see the open question below.
- Two more CI runs in the last day retried after an initial failure (task-1.37's PR, 1 retry; task-1.42's PR, 6 retries before success) that are NOT in TASK-1.82's tracked list of four. Their attempt-1 logs are gone (log retention expired by the time I checked - `gh api .../logs` returns `BlobNotFound`), so I cannot say whether they were this same timeout or something else (maybe the TASK-1.78-reported arbiter flake). Flagging honestly rather than silently excluding or silently including them in the failure-rate arithmetic below.

FIX
1. New file `.npmrc` (repo root, one line: `prefer-offline=true`, with a comment pointing at this task). Confirmed empirically that an `.npmrc` in the process's cwd has the identical effect to the `--prefer-offline` CLI flag (0.45s under the same blocked-registry/warm-cache trial). A single file covers both of the repo's two npx-likec4 call sites (`tools/src/steg_tools/likec4_mcp.py:109`'s subprocess argv and `justfile:250`'s `check-architecture` recipe) without duplicating a flag in two places, and also covers `.mcp.json`'s own invocation (interactive Claude Code use) for free. Grepped the repo for every npm/npx usage first (`git grep -n "npx\|npm "`) - these are the only two, so a repo-wide setting has no other blast radius.
2. `.github/workflows/ci.yml`: add, before the existing "just check" step -
   a. "Restore the LikeC4 npm cache" (`actions/cache`, pinned by commit SHA looked up from the action's current GitHub release at build time, per this file's own stated practice of re-checking pins against upstream rather than reusing a remembered one) over npm's actual cache directory (confirm the real path with `npm config get cache` on the runner rather than assuming `~/.npm`), keyed on `${{ runner.os }}-likec4-npm-${{ hashFiles('.mcp.json', 'tools/src/steg_tools/likec4_mcp.py', 'justfile') }}`. Hashing the three files that already carry the version pin, instead of writing the version number a fourth time in this workflow, means this step can never drift out of step with the pin the way a hand-copied version string could.
   b. "Warm the LikeC4 npx cache": read `PINNED_LIKEC4_VERSION` out of `tools/src/steg_tools/likec4_mcp.py` (e.g. `uv run --project tools python -c 'from steg_tools.likec4_mcp import PINNED_LIKEC4_VERSION as v; print(v)'`) and run `npx --yes "likec4@$VERSION" --version` with it - cheap, no model validation, same resolve+download+cache cost as any other subcommand (confirmed: warming via `--version` leaves the npm cache warm for a later `mcp --stdio` or `validate` call too). This step has no artificial time bound of its own; on a cache miss it simply takes however long npm needs, bounded only by the job's existing 30-minute timeout, and that cost becomes visible on this named step instead of disguised as a flaky test. On a cache hit (the common case once the cache above is primed) it's near-instant.
3. `mise.toml`: add `node = "24.22.0"` to `[tools]`, comment modelled on the existing `zig` entry - satisfies likec4 1.61.0's own `engines.node >=22.23.3` (read from the cached package.json), matches the version already resolved locally. No new CI step needed to install it: `jdx/mise-action` (ci.yml, already present for zig) installs and caches whatever `[tools]` lists.
4. `.github/workflows/ci.yml`: add a "Confirm the pinned Node toolchain" step (`run: node --version`) right after the existing mise steps, matching the file's own pattern for Rust and Zig (makes the pin a checked fact in the log, not an assumption).
5. Optional, separable, explicitly droppable if the engineer wants a smaller diff: split `likec4_mcp.py`'s single `timeout=30.0` into a generous `startup_timeout` (covers the now-rare cold-cache/registry-hiccup path; first `lines.get()` only) and a tight `timeout` for every reply after the first (the exchange itself measured sub-2s even under contention once warm), and make `TimeoutError`'s message say which phase it was in. This doesn't address the cause (the fix above does); it only makes a future recurrence self-describing, per the spirit of AC #1's "the notes say which."

PROOF (each item gets its own `asdlc record-check` run, or is named dropped with why; CI-run items are linked in the notes instead, per AC #3's own wording)
- P1 cause evidence above: the CI log excerpts and local timings are cited by command and job/run id in the notes; no separate check to record (nothing to run, it's reading existing logs and the reproductions below).
- P2 negative control (mechanism absent): on a scratch, uncommitted tree at the pre-fix state (no `.npmrc`), warm npm cache, registry blocked (`HTTP_PROXY=http://127.0.0.1:1 HTTPS_PROXY=http://127.0.0.1:1`), run `uv run --project tools pytest -q tools/tests/adapter/test_likec4_mcp_smoke.py` - expect it to fail/time out, proving the live network call is really inside the timed wait today. Record with `record-check` while the tree is in that state, then revert, per the testing skill's own carve-out for a demonstration that is never committed.
- P3 positive control: same blocked registry, same warm cache, `.npmrc` fix applied - same pytest command passes in under ~1s. Record with `record-check`.
- P4 ordinary path, no regression: fix applied, npm cache freshly cleared (cold, real network) - `just test-tools` passes normally.
- P5 `check-architecture` after the fix: `just check-architecture` passes normally. (Its own blocked-registry negative/positive pair is the same mechanism as P2/P3; naming it dropped here rather than re-running it, unless review asks for it.)
- P6 local repeated runs under load: one script, ten consecutive `just test-tools` runs, each preceded by clearing the local npm cache and starting three background cold `npx --yes likec4@1.61.0 --version` processes to reproduce "several CI jobs queued at once" locally; all ten expected to pass, their individual timings in the recorded output.
- P7 CI repeated/concurrent runs (AC #3): a temporary `workflow_dispatch` workflow added for this task only, a matrix of 8 parallel jobs (double the worst documented real concurrency of 4), each a genuinely cold runner with no pre-existing cache, running `just test-tools`. Linked by run URL in the notes, removed from the branch again before the PR. N=8 chosen as corroboration of P2/P3's deterministic control, not as the primary proof - at the roughly 4% baseline failure rate observed (4 confirmed occurrences / ~100 recent runs), a run of green repeats alone is weak evidence (even 20 green runs happen by chance ~44% of the time if nothing were fixed); the deterministic blocked-registry control is what actually shows the timed wait no longer contains the network.
- P8 Node pin takes effect: the "Confirm the pinned Node toolchain" step's output in a real CI run (can be the same run(s) as P7, or an ordinary PR run) shows the pinned version, linked in the notes.
- P9, only if item 5 (the timeout split) lands: negative control showing the old single-timeout message carries no phase information vs. the new message naming the phase, via a scratch edit that stalls the exchange after the first reply; record then revert. Dropped along with item 5 if the engineer prefers the smaller diff.

OPEN FOR THE ENGINEER
- Folding the Node pin into this task (mise.toml) even though the evidence says Node's version isn't the cause - cheap via the existing mise mechanism, and removes that line from TASK-1.82. Flag if you'd rather keep it strictly separate.
- N=8 parallel (CI) / N=10 sequential-under-contention (local) as the repeat-run corroboration, with the blocked-registry negative/positive control as the real proof - agree, or want a different N.
- Item 5 (split timeout) - keep as a diagnostic improvement, or drop for a smaller diff.
- task-1.37's and task-1.42's extra CI retries (logs expired, cause unconfirmed) - noting as an open question, not silently folded into or excluded from the failure-rate arithmetic above.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Branch: task-1.97-tools-the-likec4-mcp-smoke-test-stops. PR: https://github.com/bruk-io/steg/pull/53 (ready for review, CI green at final head 1e63d7b, run https://github.com/bruk-io/steg/actions/runs/43794899362, 3m11s).

CAUSE (AC #1): established from CI logs + reproduction, both cited in the plan and repeated here briefly. Failed runs (PR 44, jobs 133035963248 and the job for run 43782519189) show the whole excess time (34.2s/31.9s "tool tests" steps) is the one 30s TimeoutError, not a slow suite. A passing run (job 133045320293) shows the *first* npx-likec4 call in a job (inside "tool tests", 21.4s) costing far more than the *second* one later in the same job ("architecture", 1.7s) - a cost paid once, inside the first call. Reproduced the mechanism directly: a warm npm cache with the registry made unreachable still took 70s with plain `npx --yes`, because npm issues a live `GET https://registry.npmjs.org/likec4` and retries 3x with backoff on every invocation regardless of a warm cache (confirmed from npm's own debug log); the same conditions with `--prefer-offline` took 0.43s, zero network attempted. Could not reproduce the exact 31-34s purely through local concurrent download contention (reached 22s at 4-way) - said so in the plan rather than papering over it; the registry-retry-ladder number is what explains the rest, and it is exactly what CI's own cold, unwarmed cache exposes every job to. Ruled out "server started, reply lost": forced short timeouts locally and confirmed `exc.add_note`/stderr-capture works (shows real content when npm actually writes any) but was empty in both the real failures and the repro, consistent with npm being silent during its own resolve/fetch phase, not contradicting the cause.

FIX (AC #2): one `.npmrc` (prefer-offline=true, covers both npx-likec4 call sites and .mcp.json with one file), a CI cache-restore + explicit "Warm the LikeC4 npx cache" step before `just check` (so a cache-miss's cost lands on a named, unbounded step, never inside the test), a `node` pin in mise.toml (AC #4, see below), and a split of the smoke test's one 30s timeout into a generous `startup_timeout=60.0` (first reply only) and a tight `timeout=5.0` (every reply after), so a stall now names which phase it was in. Not skipped, not marked flaky, not retried. Confirmed on PR 53's CI run (job 133059257453): npm cache key missed on this first run ("Cache not found for input keys: macOS-likec4-npm-..."), the warm step paid ~20.6s for the real download in its own step, and the smoke test itself then ran in 3.4s and check-architecture in 1.4s - the cold-start cost landed where it's supposed to, not inside either test.

AC #4 (Node pin): Node was unpinned everywhere (not in mise.toml, no setup-node/version step in ci.yml). Evidence above shows Node's version was not part of the cause - the delay is npm's own registry-retry behaviour - but it's pinned here anyway (mise.toml: node = "24.22.0", satisfies likec4 1.61.0's own engines.node >=22.23.3) since it's free through the mise-action step already installing and caching Zig, and TASK-1.82's engineer decision was to fold it in here. Confirmed in the same CI run: "Confirm the pinned Node toolchain" printed v24.22.0.

PROOF (AC #3), each item recorded, none skipped silently:
- P1 (cause evidence): no separate check; cited above and in the commands/logs referenced.
- P2 (negative control, mechanism absent): recorded. Moved `.npmrc` aside, warm cache, registry blocked (HTTP_PROXY/HTTPS_PROXY to a closed port), ran the real adapter test directly: failed with `TimeoutError: likec4 mcp did not answer (to start) within 60.0s` - the new phase-naming message working as designed. `.npmrc` restored immediately after; a following `git status --short` check, also recorded, shows the tree back to exactly the fix state with no residue.
- P3 (positive control): recorded. Same blocked registry, same warm cache, fix in place: passed in 0.65s.
- P4 (ordinary path, cold cache + real network, no regression): recorded locally (`just test-tools`, fresh NPM_CONFIG_CACHE dir, 11.9s) and shown on CI's own first run on this branch (the 20.6s warm step above, cache miss then warm, exactly the designed path).
- P5 (check-architecture after the fix): recorded locally (`just check-architecture`, 0.7s). Its own blocked-registry negative/positive pair is explicitly dropped - same mechanism as P2/P3, already shown once; CI's own run above additionally shows it passing fast (1.4s) right after the warm step on a genuine cache miss, which is the stronger version of this same confirmation.
- P6 (local repeated runs under load): recorded, one script, 10 consecutive `just test-tools` runs, each against a freshly cleared cache with 3 concurrent background cold npx fetches competing for network - all 10 passed, 13-25s each, comfortably under the 60s startup bound.
- P7 (CI repeated/concurrent runs): a temporary `pull_request`-triggered workflow (not `workflow_dispatch` - GitHub 404s dispatching a workflow that isn't yet on the default branch, confirmed), 8-way matrix, each a genuinely cold runner with no pre-existing cache. Run: https://github.com/bruk-io/steg/actions/runs/43794226923 - all 8 "repeat" jobs passed (45s-1m45s each). Removed again in commit 6d1da0b, before the PR was marked ready for review.
- P8 (Node pin takes effect): covered above, in the same CI run that also demonstrates P4/P5's CI side.
- P9 (timeout-split negative control): dropped. Item 5 (the startup/exchange split) landed, but P2 above already exercises it directly - the negative control's actual failure message ("did not answer (to start) within 60.0s") is the P9 demonstration; a separate mid-exchange-stall scratch edit would prove the same phase-naming mechanism a second time for no new information, so not run separately.

COVERAGE AGAINST THE FOUR ACCEPTANCE CRITERIA
- AC #1 (cause established, not assumed): covered by the CAUSE section above - CI logs for the failed and passing runs, and the warm-cache/blocked-registry/retry-ladder reproduction. The one gap said plainly: local concurrent-contention alone only reached 22s, short of CI's observed 31-34s; the registry-retry-ladder evidence is what closes that gap.
- AC #2 (fix addresses the cause, not a bare timeout raise, not skipped/flaky/retried): covered by FIX above. Raising the old 30s bound alone was explicitly not done; the new startup_timeout=60.0 is paired with removing the live-network dependency in the common case (the actual mechanism), and is itself evidenced (P2/P3/P6).
- AC #3 (shown by repeated runs, N justified against the failure rate): covered by P6 (N=10 local, under load) and P7 (N=8 CI, concurrent, linked above), both corroborating the deterministic P2/P3 control, which is the primary proof - at the ~4% baseline failure rate observed (4 confirmed occurrences / ~100 recent runs), repeated green runs alone would be weak evidence (even 20 in a row happen ~44% of the time by chance if nothing were fixed); the blocked-registry pair is what actually shows the timed wait no longer contains the network.
- AC #4 (Node pin if part of the cause, removed from TASK-1.82): covered above - pinned in mise.toml and confirmed in CI, done here per the engineer's decision even though the evidence says Node's version wasn't itself the cause.

OPEN NOTE CARRIED FROM THE PLAN: two more CI runs retried in the days before this task (task-1.37's PR, 1 retry; task-1.42's PR, 6 retries) are not in TASK-1.82's tracked list of four and their logs had already expired by the time I checked (gh api returned BlobNotFound) - cause unconfirmed, not folded into the failure-rate arithmetic above either way.

Built per the approved plan: a root .npmrc (prefer-offline=true), a CI cache-restore + warm step before just check, a node pin in mise.toml (AC #4), and a split of the smoke test's timeout into a startup bound and an exchange bound. Proof: deterministic blocked-registry negative/positive control (P2/P3), the ordinary cold-cache path unchanged (P4), check-architecture unaffected (P5), 10 local runs under simulated load (P6), 8-way concurrent on genuinely cold CI runners via a now-removed temporary workflow (P7, run 43794226923, all passed), the Node pin confirmed in CI (P8). just check green at the final head (1e63d7b) both locally and on CI (run 43794899362). Full writeup and coverage against all four acceptance criteria is in the task's notes.

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && uv run --project tools pytest -q tools/tests/functional/test_likec4_mcp.py -v'` exited 0 at 2026-11-16T03:17:17+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 44c6b2caac3d with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpl05YxY` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
============================= test session starts ==============================
platform darwin -- Python 3.15.0, pytest-9.2.0, pluggy-1.7.0
rootdir: /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97/tools
configfile: pyproject.toml
collected 2 items

tools/tests/functional/test_likec4_mcp.py ..                             [100%]

============================== 2 passed in 10.44s ==============================
```

Commits since the review: 58, 58 without the Asdlc-Worker trailer.

Check FAILED: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && uv run --project tools pytest -q tools/tests/functional/test_likec4_mcp.py -v'` exited 1 at 2026-11-16T03:18:22+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 44c6b2caac3d with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpeLSTK2` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
============================= test session starts ==============================
platform darwin -- Python 3.15.0, pytest-9.2.0, pluggy-1.7.0
rootdir: /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97/tools
configfile: pyproject.toml
collected 2 items

tools/tests/functional/test_likec4_mcp.py FF                             [100%]

=================================== FAILURES ===================================
____________ test_a_stall_before_any_reply_names_the_start_up_phase ____________

    def test_a_stall_before_any_reply_names_the_start_up_phase() -> None:
>       with pytest.raises(TimeoutError, match=r"\(to start\) within 0\.2s"):
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
E       AssertionError: Regex pattern did not match.
E         Expected regex: '\\(to start\\) within 0\\.2s'
E         Actual message: 'likec4 mcp did not answer within 5.0s'

tools/tests/functional/test_likec4_mcp.py:23: AssertionError
_______ test_a_stall_after_the_first_reply_names_the_mid_exchange_phase ________

    def test_a_stall_after_the_first_reply_names_the_mid_exchange_phase() -> None:
>       with pytest.raises(TimeoutError, match=r"\(mid-exchange\) within 0\.2s"):
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
E       AssertionError: Regex pattern did not match.
E         Expected regex: '\\(mid-exchange\\) within 0\\.2s'
E         Actual message: 'likec4 mcp did not answer within 0.2s'

tools/tests/functional/test_likec4_mcp.py:28: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/functional/test_likec4_mcp.py::test_a_stall_before_any_reply_names_the_start_up_phase
FAILED tools/tests/functional/test_likec4_mcp.py::test_a_stall_after_the_first_reply_names_the_mid_exchange_phase
============================== 2 failed in 15.26s ==============================
```

Commits since the review: 58, 58 without the Asdlc-Worker trailer.

Check FAILED: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && just check'` exited 2 at 2026-11-16T03:19:30+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 44c6b2caac3d with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.8s)
ok      tests (every crate) (5.5s)
ok      build (steg-sdk) (0.2s)
ok      test (steg-sdk) (0.4s)
ok      test (steg-sdk, local-socket) (0.5s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (4.3s)
ok      test (steg-server, race-fixture) (4.6s)
ok      build (steg) (0.4s)
ok      test (steg) (0.3s)
ok      test (steg, fixtures) (2.7s)
ok      build (steg-ui) (0.6s)
ok      test (steg-ui) (0.6s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.4s)
FAILED  tool tests (exit 2, 0.2s)

==================================== ERRORS ====================================
________________ ERROR collecting tests/unit/test_likec4_mcp.py ________________
import file mismatch:
imported module 'test_likec4_mcp' has this __file__ attribute:
  /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97/tools/tests/functional/test_likec4_mcp.py
which is not the same as the test file we want to collect:
  /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97/tools/tests/unit/test_likec4_mcp.py
HINT: remove __pycache__ / .pyc files and/or use a unique basename for your test file modules
=========================== short test summary info ============================
ERROR tools/tests/unit/test_likec4_mcp.py
!!!!!!!!!!!!!!!!!!!! Interrupted: 1 error during collection !!!!!!!!!!!!!!!!!!!!
1 error in 0.09s
full output: .quiet/tool-tests.log
error: recipe `test-tools` failed on line 92 with exit code 2
```

Commits since the review: 58, 58 without the Asdlc-Worker trailer.

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && just check'` exited 0 at 2026-11-16T03:20:26+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 44c6b2caac3d with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.5s)
ok      boundaries (0.1s)
ok      licences (0.9s)
ok      tests (every crate) (4.6s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.2s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.1s)
ok      test (steg-kernel) (0.2s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.0s)
ok      test (steg-server, race-fixture) (4.3s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.6s)
ok      build (steg-ui) (0.6s)
ok      test (steg-ui) (0.9s)
ok      build (steg-app) (0.8s)
ok      test (steg-app) (0.5s)
ok      tool tests (12.4s)
ok      architecture (0.8s)
ok      recipe names (0.1s)
```

Commits since the review: 58, 58 without the Asdlc-Worker trailer.

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && uv run --project tools pytest -q tools/tests/functional/test_likec4_mcp_phased_timeout.py -v'` exited 0 at 2026-11-16T03:20:48+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 44c6b2caac3d with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpEoHak6` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
============================= test session starts ==============================
platform darwin -- Python 3.15.0, pytest-9.2.0, pluggy-1.7.0
rootdir: /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97/tools
configfile: pyproject.toml
collected 2 items

tools/tests/functional/test_likec4_mcp_phased_timeout.py ..              [100%]

============================== 2 passed in 10.45s ==============================
```

Commits since the review: 58, 58 without the Asdlc-Worker trailer.

Check FAILED: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && uv run --project tools pytest -q tools/tests/functional/test_likec4_mcp_phased_timeout.py -v'` exited 1 at 2026-11-16T03:21:15+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 44c6b2caac3d with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmp3kQ7gM` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
============================= test session starts ==============================
platform darwin -- Python 3.15.0, pytest-9.2.0, pluggy-1.7.0
rootdir: /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97/tools
configfile: pyproject.toml
collected 2 items

tools/tests/functional/test_likec4_mcp_phased_timeout.py FF              [100%]

=================================== FAILURES ===================================
____________ test_a_stall_before_any_reply_names_the_start_up_phase ____________

    def test_a_stall_before_any_reply_names_the_start_up_phase() -> None:
>       with pytest.raises(TimeoutError, match=r"\(to start\) within 0\.2s"):
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
E       AssertionError: Regex pattern did not match.
E         Expected regex: '\\(to start\\) within 0\\.2s'
E         Actual message: 'likec4 mcp did not answer within 5.0s'

tools/tests/functional/test_likec4_mcp_phased_timeout.py:23: AssertionError
_______ test_a_stall_after_the_first_reply_names_the_mid_exchange_phase ________

    def test_a_stall_after_the_first_reply_names_the_mid_exchange_phase() -> None:
>       with pytest.raises(TimeoutError, match=r"\(mid-exchange\) within 0\.2s"):
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
E       AssertionError: Regex pattern did not match.
E         Expected regex: '\\(mid-exchange\\) within 0\\.2s'
E         Actual message: 'likec4 mcp did not answer within 0.2s'

tools/tests/functional/test_likec4_mcp_phased_timeout.py:28: AssertionError
=========================== short test summary info ============================
FAILED tools/tests/functional/test_likec4_mcp_phased_timeout.py::test_a_stall_before_any_reply_names_the_start_up_phase
FAILED tools/tests/functional/test_likec4_mcp_phased_timeout.py::test_a_stall_after_the_first_reply_names_the_mid_exchange_phase
============================== 2 failed in 15.26s ==============================
```

Commits since the review: 58, 58 without the Asdlc-Worker trailer.

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && git status --short'` exited 0 at 2026-11-16T03:21:29+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 44c6b2caac3d with uncommitted changes:

```text
 M .github/workflows/ci.yml
 M tools/src/steg_tools/likec4_mcp.py
?? tools/tests/functional/test_likec4_mcp_phased_timeout.py
```

Commits since the review: 58, 58 without the Asdlc-Worker trailer.

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && just check'` exited 0 at 2026-11-16T03:22:09+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 44c6b2caac3d with uncommitted changes:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (4.6s)
ok      build (steg-sdk) (0.2s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.3s)
ok      build (steg-server) (0.1s)
ok      test (steg-server) (4.1s)
ok      test (steg-server, race-fixture) (4.0s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.4s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.4s)
ok      architecture (0.9s)
ok      recipe names (0.1s)
```

Commits since the review: 58, 58 without the Asdlc-Worker trailer.

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && just check'` exited 0 at 2026-11-16T03:23:11+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 34311bf51cd9:

```text
ok      format check (0.2s)
ok      lint (0.4s)
ok      boundaries (0.1s)
ok      licences (0.7s)
ok      tests (every crate) (4.8s)
ok      build (steg-sdk) (0.1s)
ok      test (steg-sdk) (0.3s)
ok      test (steg-sdk, local-socket) (0.4s)
ok      build (steg-kernel) (0.2s)
ok      test (steg-kernel) (0.4s)
ok      build (steg-server) (0.2s)
ok      test (steg-server) (3.8s)
ok      test (steg-server, race-fixture) (4.1s)
ok      build (steg) (0.1s)
ok      test (steg) (0.1s)
ok      test (steg, fixtures) (2.4s)
ok      build (steg-ui) (0.3s)
ok      test (steg-ui) (0.3s)
ok      build (steg-app) (0.3s)
ok      test (steg-app) (0.3s)
ok      tool tests (11.3s)
ok      architecture (0.7s)
ok      recipe names (0.1s)
```

Commits since the review: 78, 77 without the Asdlc-Worker trailer.

REVIEW FINDINGS FIXED (@bruk.r2's review, same-machine), branch task-1.97-tools-the-likec4-mcp-smoke-test-stops, head 34311bf, CI green (run https://github.com/bruk-io/steg/actions/runs/43797888831, 3m35s):

1. Important, corrected: the notes' P9 claim was wrong. P2's blocked-registry control only ever reaches the "to start" branch (`first` is only set False after a successful `lines.get()`); the "mid-exchange" branch (reached after the first reply, bound=timeout) was never exercised by anything recorded on the task. Fixed by extracting the process-spawning/phased-timeout body out of `read_project_summary` into a new `summary_from_command(argv, *, startup_timeout, timeout)`, with `read_project_summary` now a thin wrapper building the real `npx` argv and handing it over. Added `tools/tests/functional/test_likec4_mcp_phased_timeout.py`: two deterministic tests driving `summary_from_command` against a fake child (a bare `python -c` script - never a real likec4 server, that stays the adapter test's job) -
   - `test_a_stall_before_any_reply_names_the_start_up_phase`: a child that never replies, `startup_timeout=0.2` - asserts the message names "to start".
   - `test_a_stall_after_the_first_reply_names_the_mid_exchange_phase`: a child that replies once (a valid id=1 JSON-RPC line) then hangs, `timeout=0.2` - asserts the message names "mid-exchange". This is the branch P9's drop reason wrongly claimed was already covered; it is now, directly.
   Negative control recorded: with the branch scratch-collapsed back to one bound and no phase name, both new tests fail for the expected reason (got "likec4 mcp did not answer within 5.0s" / "within 0.2s", no phase name, instead of the phase-naming messages); reverted immediately after, `git status --short` recorded showing the tree back to the fix state with no residue. Both tests pass again post-revert (recorded), and the full `just check` is green (recorded, and on CI).
   P9 is no longer dropped - replaced by these two tests and their negative control. The task's earlier notes calling it dropped were wrong and are superseded by this entry.

2. Nit fixed: `likec4_mcp.py`'s `summary_from_command` docstring now says the 70s unreachable-registry figure was measured *before* the `.npmrc` fix, and that the same scenario is under a second with `.npmrc` present (TASK-1.97's notes), so it no longer reads as a failure mode that can still occur alongside the 60s bound.

3. Nit fixed: `ci.yml`'s step installing Zig via mise is renamed "Install the pinned Zig and Node (mise.toml)" and its comment now says it installs the Node pin too, at no extra step/cost.

All proof items for these fixes recorded via `asdlc record-check`; none skipped or claimed without a run.

Fixed all 3 review findings. Important: added tools/tests/functional/test_likec4_mcp_phased_timeout.py with two deterministic tests (fake child process, not the real server) reaching both the 'to start' and 'mid-exchange' timeout branches, refactoring summary_from_command(argv, ...) out of read_project_summary to make this testable; negative control recorded (branch collapsed, both new tests fail for the right reason, then reverted, git status clean recorded). Nits: docstring now says the 70s figure predates the .npmrc fix; ci.yml's mise-install step renamed/commented to mention the Node pin it now also installs. just check green locally and on CI at head 34311bf (run 43797888831, 3m35s). Full corrected writeup appended to the task's notes.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T02:30:00Z

---

Plan approved by @bruk (1b39eee49ff0, size-2): its worker may build it.

---

author: @bruk.r2
created: 2026-11-16T03:02:00Z

---

@bruk.r2 is reviewing TASK-1.97: a same-machine review: it waited 3m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r2
created: 2026-11-16T03:12:00Z

---

Review by @bruk.r2 (same-machine): changes requested. Back to In Progress for the author's worker.

Findings: 1 Important, 2 nit. Cites: review, testing. The diff matches the plan.

Reviewed at head 1e63d7b907f04794bf93bd759058e4af6d59e1eb (matches PR 53's head and its green CI run 43794899362; the task's own recorded `just check` is also at this head).

[Evidence, Important] tools/src/steg_tools/likec4_mcp.py:161-171, task notes PROOF item P9 - Item 5's timeout split adds a distinct "mid-exchange" branch (reached only after the first line is read, bound=timeout=5.0s), separate from the "to start" branch (bound=startup_timeout=60.0s). The notes drop P9 ("negative control showing the old single-timeout message carries no phase information vs. the new message naming the phase") saying "P2 above already exercises it directly - the negative control's actual failure message ('did not answer (to start) within 60.0s') is the P9 demonstration." That's wrong: `first` is only set False after a successful `lines.get()`, so P2 (registry blocked, .npmrc removed) can only ever hit the "to start" branch - I reran P2 myself and got exactly "did not answer (to start) within 60.0s", matching commit b3dd24f. I then called `read_project_summary(..., timeout=0.0001)` directly (uncommitted, nothing changed on disk) and got a materially different message - "likec4 mcp did not answer (mid-exchange) within 0.0001s" - proving the mid-exchange branch exists and is reachable, but also that nothing recorded on this task has ever reached it. Per review.md ("one direction of a pair... needs a test that reaches that branch") and testing.md ("named as dropped and say why" / "citing the plan's list is not covering it"), P9's drop reason is false and the branch is unproven. This is the same mistake shape as the open draft DRAFT-1 ("record every negative control's failing run, don't just claim it") - not filing a new draft, this one already covers it.

[Evidence, Nit] tools/src/steg_tools/likec4_mcp.py:110-120 - the docstring cites "up to 70s when the registry itself is unreachable and npm's own retry ladder runs out" immediately next to the new startup_timeout=60.0, without noting that 70s figure was measured with .npmrc/prefer-offline absent; post-fix the same scenario is 0.43s per the plan. As written it reads as if the bound is smaller than a failure mode that can still occur, which isn't the case once .npmrc is always present. Worth a clarifying word.

[Compliance, Nit] .github/workflows/ci.yml:75-85 - "Install the pinned Zig (mise.toml)" (and its comment) now also installs the new `node` pin via the same jdx/mise-action step, but the name/comment weren't updated to mention it.

Verified myself, no findings: the cause (read failed-run log job 133035963248: "FAILED tool tests (exit 1, 34.2s)", "TimeoutError: ... within 30.0s", "1 failed, 51 passed in 33.17s"; passing-run log job 133045320293: "tool tests (21.4s)" / "architecture (1.7s)" - both match the notes verbatim); the deterministic blocked-registry pair myself in a fresh review worktree (warm cache, HTTP(S)_PROXY to a closed port, .npmrc moved aside: failed at "did not answer (to start) within 60.0s", 65.04s wall; .npmrc restored: passed in 0.60s); .npmrc's blast radius (grepped, only .mcp.json/justfile/likec4_mcp.py use npx, confirmed covered); the actions/cache pin (55cc8345863c7cc4c66a329aec7e433d2d1c52a9 is really v6.1.0); the CI step ordering (Node confirm -> cache restore -> warm step -> just check, nothing removed); the 5.0s exchange bound's safety margin (direct call with timeout=0.5 against the full response - 7 views, 18 elements - passed, ~10x headroom); the merge commit 1e63d7b (only pulls in unrelated backlog/ files, no code the plan hadn't seen); and the Node pin in CI logs (v24.22.0 printed). Cleaned up: git status clean in the review worktree, no stray npx/likec4 processes, proxy env vars unset.

---

author: @bruk.r3
created: 2026-11-16T03:27:00Z

---

@bruk.r3 is reviewing TASK-1.97: a same-machine review: it waited 0m with no other engineer's reviewer, longer than review_fallback_after (0m), so a fresh reviewer on its author's machine takes it.

---

author: @bruk.r3
created: 2026-11-16T03:40:00Z

---

Review by @bruk.r3 (same-machine): clean. It stays In Review for a person to merge.

Findings: 0 Important, 1 nit. Cites: review. The diff does not match the plan.

Reviewed at head 34311bf51cd9e5b8822faad05aff3573fbe3778d (matches PR 53's head; PR's CI is green at this head, run https://github.com/bruk-io/steg/actions/runs/43797888831, 3m35s; the task's own recorded `just check` is also at this head, and I reran `just check` myself in a fresh worktree at this head - green, including "tool tests (11.7s)" and "architecture (0.7s)").

Verified the fix for @bruk.r2's prior Important finding (P9/mid-exchange untested): the refactor extracts `summary_from_command(argv, *, startup_timeout, timeout)` out of `read_project_summary`, which is now a thin wrapper passing both bounds through unchanged and building the identical `npx` argv (byte-for-byte the same list as before the refactor). The two new tests in `tools/tests/functional/test_likec4_mcp_phased_timeout.py` drive a real fake child process (`python -c`, never a real likec4 server - that stays `tests/adapter/test_likec4_mcp_smoke.py`'s job): one that never replies (reaches the "to start" branch) and one that replies once then hangs (reaches "mid-exchange", only reachable after `first` is set False by a successfully consumed first line). Both deterministic (bounded `queue.get`, no sleep standing in for a condition), ~10.4s combined only because of the pre-existing 5s `wait(timeout=5)` grace period in the unchanged `finally` block before `kill()` - not new, and not a flaw in these tests' own bounds (0.2s/5.0s).

I repeated the negative control myself: collapsed the phase-naming/two-bound branch to one generic message in a scratch edit - both new tests failed for the stated reason (`AssertionError: ... Actual message: 'likec4 mcp did not answer within 0.2s'`), then reverted (`git status --short` clean after). This is the same branch the first review found unproven; it is now proven.

Child cleanup (Bugs pass, since a process is started): the `finally` block (close stdin -> `wait(timeout=5)` -> `kill()` on timeout) is unchanged by this diff and runs on every path - success, each timeout, and any exception - confirmed with `ps` after running the new tests and after the real adapter test's blocked-registry pair (none left). `kill()` isn't followed by a second `wait()`, so a killed child could zombie until the parent exits, but that's pre-existing, not introduced here, and out of this task's scope.

Repeated the deterministic blocked-registry pair on this head directly against the real pinned server (`tests/adapter/test_likec4_mcp_smoke.py`): `.npmrc` moved aside, warm cache, registry blocked (closed-port HTTP(S)_PROXY) - failed at "likec4 mcp did not answer (to start) within 60.0s", 65.04s wall, matching the first review's numbers; `.npmrc` restored - passed in 0.64s. That test isn't in the diff's stat against main, so it still asserts exactly what it did before. Cleaned up after: proxy vars were command-scoped (none left in env), `git status --short` clean, no likec4/npx process left.

`summary_from_command` being public matches the module's existing convention (`take_replies`, `parse_project_summary` are also public with callers in both the module and its tests) - nothing newly exposed that shouldn't be.

Taxonomy (testing-python.md only names unit/adapter for this repo's Python tools, but the team's testing.md and this repo's existing `tools/tests/functional/test_gallery_check.py` establish `functional` as the right layer for "behaviour, with the outside world faked at the boundary" - a fake child process at the process boundary, not a mock of the code under test): `functional` is the right place and name; the `_phased_timeout` suffix is load-bearing (distinguishes it from the pre-existing, unrelated `tests/unit/test_likec4_mcp.py`, whose colliding basename with an earlier draft of this file caused the recorded "import file mismatch" failure now fixed by this name).

P5 (check-architecture) drop reason confirmed sound: same `.npmrc`, same cwd-root `prefer-offline` resolution, same mechanism as P2/P3; this head's CI shows "architecture (1.6s)" right after a genuine cache miss on the warm step, which is the stronger version of the same confirmation.

Read `.github/workflows/ci.yml` and `mise.toml` in full in the review worktree (not just hunks): step order is Rust confirm -> mise install (Zig+Node) -> Zig confirm -> Node confirm -> uv setup -> npm cache-dir lookup -> cache restore -> warm step -> just/cargo-deny install -> cargo-cache -> just check -> just test-all (push only). Matches the notes; nothing removed or reordered since the first review (ci.yml's only change since 1e63d7b is the step-name/comment nit fix below).

Both nits from the first review are fixed as described: the docstring now says the 70s unreachable-registry figure predates the `.npmrc` fix (well under a second with it present); `ci.yml`'s mise-install step is renamed "Install the pinned Zig and Node (mise.toml)" with a comment naming the Node pin.

One new nit: `likec4_mcp.py`'s `summary_from_command` docstring says "`tests/functional/test_likec4_mcp.py` is the other one" (the fake-child caller) - that file doesn't exist. The real file, deliberately renamed to avoid the collision recorded in this task's own notes (03:19:30 "import file mismatch" between `functional/test_likec4_mcp.py` and the pre-existing `unit/test_likec4_mcp.py`), is `tests/functional/test_likec4_mcp_phased_timeout.py`. A stale pointer, not a behaviour change - Nit per review.md's wording skip, with the "would make a reader or a model do the wrong thing" carve-out: a reader following it finds nothing, or a model recreating it reintroduces the same collision.

Every proof item P2-P9 has a recorded run or a named-and-sound drop reason (P5, unchanged from the first review); P9 is no longer dropped, replaced by the two tests above and their negative control, which the task's notes correctly say supersedes the earlier (wrong) drop claim.

`--plan-matched no`: the diff since the plan's approval includes a file the plan doesn't name (`tools/tests/functional/test_likec4_mcp_phased_timeout.py`) and a refactor (extracting `summary_from_command` to take `argv`) that item 5 didn't specify, both added in response to the first review's finding rather than the plan's own order of work. The notes explain this fully and it's the right fix, but the flag records the departure regardless of the explanation, per its own wording.

No drafts filed: `asdlc board draft list --plain` shows no open drafts, and this review's one finding (the stale docstring path) isn't a repeat of a mistake an earlier review cited - it's new.

Cleanup: worktree left clean (`git status --short` empty), no likec4/npx processes left, no proxy env vars left set.

---

created: 2026-11-16T03:47:00Z

---

Post-review nit fixed: tools/src/steg_tools/likec4_mcp.py's docstring pointed at tests/functional/test_likec4_mcp.py, which doesn't exist; corrected to test_likec4_mcp_phased_timeout.py (its real name, renamed earlier to avoid a basename collision). Nothing else changed.

Ran: `just check` in the foreground at the merged head (9bee526) - all green (format check, lint, boundaries, licences, every crate's build/test, tool tests 11.3s, architecture 0.7s, recipe names). Pushed to the same branch (34311bf..9bee526) and watched `gh pr checks 53 --watch` to green on the new head (run https://github.com/bruk-io/steg/actions/runs/43799855944, 3m48s). No record-check or finish run, per instruction that the board doesn't take those after a clean review.

---
<!-- COMMENTS:END -->
