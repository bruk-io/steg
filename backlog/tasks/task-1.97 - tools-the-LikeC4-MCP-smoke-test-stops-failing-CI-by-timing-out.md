---
id: TASK-1.97
title: 'tools: the LikeC4 MCP smoke test stops failing CI by timing out'
status: In Progress
assignee:
  - '@bruk.w5'
created_date: '2026-11-16T01:34:00Z'
updated_date: '2026-11-16T02:39:00Z'
labels:
  - size-2
milestone: m-3
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.97-tools-the-likec4-mcp-smoke-test-stops
asdlc_plan_approved_by: bruk
asdlc_plan_approved_hash: '1b39eee49ff0'
asdlc_plan_approved_size: size-2
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pulled out of TASK-1.82 (housekeeping, m-0) on 2026-11-15 because it is costing a re-run on real pull requests while six tasks are in flight: tools/tests/adapter/test_likec4_mcp_smoke.py::test_a_session_can_list_this_repositorys_views_and_elements_through_the_mcp_server raises TimeoutError('likec4 mcp did not answer within 30.0s') from tools/src/steg_tools/likec4_mcp.py. Seen on the pull requests of TASK-1.40, TASK-1.76 and TASK-1.77 (twice, two heads), none of which touched tools/ or architecture/, green on re-run each time. doc-16 R108 is why the test exists (the architecture model is checked).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause of the timeouts is established from the failed runs' logs and a reproduction, not assumed: what the 30 seconds were spent on (downloading or starting the pinned likec4 through npx, the server indexing the model, a reply lost), on CI's runner and locally under load; the notes say which
- [ ] #2 The fix addresses that cause: for example the tool is installed or cached before the test so no download happens inside the timed wait, or the test waits on the server's own readiness and not a fixed time. Raising the timeout alone is not a fix unless the cause is shown to be plain slowness with a bound, and then the bound is stated with its evidence. The test is not skipped, marked flaky or retried silently
- [ ] #3 Shown by repeated runs: the test passes N times in a row on CI (a workflow run that repeats it, linked in the notes) and locally under load, with N chosen and justified against the failure rate seen so far (four failures on pull requests 40's, 76's task's, and 44 twice)
- [ ] #4 If Node is not pinned in CI and that is part of the cause, pinning it is done here and removed from TASK-1.82's list
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
Plan checked by @bruk.o: ok. The cause is established from the failed and passing runs' timings and from reproduction: npx makes a live round trip to the npm registry even when the pinned package is already cached, CI's runners start with no npm cache, and when the registry is slow or retried that round trip lands inside the test's 30 second wait. The fix addresses it: prefer-offline for npm in this repository (one .npmrc, covering both npx call sites and the editor's MCP entry), and in CI a restored cache with an explicit warm-up step before just check, so a cold fetch is paid in a named step and not inside a timed wait. The test is not skipped, retried or loosened. Proof is a deterministic pair of runs with the registry blocked (without the fix the test fails; with it, it passes in under a second), with repeated cold CI jobs and local runs under load as corroboration only, since at the failure rate seen a run of green results alone proves little. Stated honestly in the plan: the 31 to 34 seconds seen in CI were not reproduced by contention alone. Orchestrator's answers to the three open points, which the engineer may overrule when approving: pin Node here through mise.toml (cheap, removes it from TASK-1.82's list, though it is not the cause); the proof framing and run counts are right; keep the split of the one timeout into a start-up bound and an exchange bound, so a future failure says which phase was slow.

Check passed: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && just test-tools'` exited 0 at 2026-11-16T02:31:21+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 108747d79566:

```text
ok      tool tests (1.1s)
```

Check FAILED: `sh -c 'cd /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 && HTTP_PROXY=http://127.0.0.1:1 HTTPS_PROXY=http://127.0.0.1:1 uv run --project tools pytest -q tools/tests/adapter/test_likec4_mcp_smoke.py'` exited 1 at 2026-11-16T02:39:12+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.97 on task-1.97-tools-the-likec4-mcp-smoke-test-stops at 108747d79566 with uncommitted changes:

```text
warning: `VIRTUAL_ENV=/Users/brukhabtu/.cache/uv/builds-v0/.tmpsISfbL` does not match the project environment path `tools/.venv` and will be ignored; use `--active` to target the active environment instead
F                                                                        [100%]
=================================== FAILURES ===================================
_ test_a_session_can_list_this_repositorys_views_and_elements_through_the_mcp_server _

    def test_a_session_can_list_this_repositorys_views_and_elements_through_the_mcp_server() -> None:
        root = workspace.repo_root()
        assert root is not None, "this test runs inside a git checkout"
    
>       summary = read_project_summary(root / "architecture", likec4_version=PINNED_LIKEC4_VERSION)
                  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

tools/tests/adapter/test_likec4_mcp_smoke.py:46: 
_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ 
tools/src/steg_tools/likec4_mcp.py:178: in read_project_summary
    responses = take_replies(_received_messages(), count=2)
                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
tools/src/steg_tools/likec4_mcp.py:64: in take_replies
    for message in messages:
                   ^^^^^^^^
_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ 

    def _received_messages() -> Iterator[Mapping[str, object]]:
        first = True
        while True:
            bound = startup_timeout if first else timeout
            try:
                line = lines.get(timeout=bound)
            except queue.Empty:
                phase = "to start" if first else "mid-exchange"
>               raise TimeoutError(f"likec4 mcp did not answer ({phase}) within {bound}s") from None
E               TimeoutError: likec4 mcp did not answer (to start) within 60.0s

tools/src/steg_tools/likec4_mcp.py:169: TimeoutError
=========================== short test summary info ============================
FAILED tools/tests/adapter/test_likec4_mcp_smoke.py::test_a_session_can_list_this_repositorys_views_and_elements_through_the_mcp_server
1 failed in 65.04s (0:01:05)
```
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @bruk
created: 2026-11-16T02:30:00Z

---

Plan approved by @bruk (1b39eee49ff0, size-2): its worker may build it.

---
<!-- COMMENTS:END -->
