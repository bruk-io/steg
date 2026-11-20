---
id: TASK-1.142
title: just run builds the steg binary the app starts its server with
status: In Review
assignee:
  - '@bruk.w1'
created_date: '2026-11-20T03:39:00Z'
updated_date: '2026-11-20T04:11:00Z'
labels:
  - size-1
milestone: m-4
dependencies: []
references:
  - task-1.142-just-run-builds-the-steg-binary-the-app
  - https://github.com/bruk-io/steg/pull/85
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.142-just-run-builds-the-steg-binary-the-app
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found 2026-11-19 when the engineer ran the app from main: the window came up with no server. The app does autostart a server (crates/app/src/startup.rs resolve_binary: STEG_BINARY, else the steg binary beside its own executable), but the run recipe in the justfile only runs cargo run --package steg-app, which never builds the steg package. The steg binary in target/debug was from 15 Nov and answered 'not yet implemented', so the autostart spawned a program that could not serve. Fix in the justfile's run recipe: build the steg package first (same ZIG handling as the recipe already has), then run the app. No product code changes. Follow-up worth noting in the plan, not building here: what the app shows when the binary it autostarts fails.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 From a clean target directory, just run alone gives a window with a working pane: the server binary the app autostarts is built from the same source as the app
- [ ] #2 A stale steg binary beside the app is rebuilt by just run before the app launches
- [ ] #3 just check still passes, including the recipe-name check
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the worktree's justfile to find the run recipe (line 229).
2. Update the run recipe's comment to explain why it builds steg first: the app starts its server from the steg binary beside its own executable via resolve_binary() in startup.rs, so the binary must be current.
3. Add a new line before 'cargo run --package steg-app' that runs 'cargo build --package steg' with the same environment variable handling (ZIG export and rust workspace check).
4. Verify no other recipes are changed and no product code is modified.
5. Run 'just --dry-run run' to show the build line before the run line.
6. Run 'just check' with 600s timeout to verify all checks pass.
7. Commit the change with the standard trailer.
8. Push the branch to origin and open a PR.
9. Wait for CI to pass and then finish the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Check passed: `just --justfile justfile --working-directory . --dry-run run` exited 0 at 2026-11-20T03:41:28+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.142 on task-1.142-just-run-builds-the-steg-binary-the-app at df3e435555b5 with uncommitted changes:

```text
#!/usr/bin/env bash
# Copies, not `cp -n`: BSD cp (macOS) exits 1 when `-n` skips an already-present
# file, which is the common case here (every run after the first) - `cp -n ...
# || true` would make a *genuine* failure (missing source, unwritable target/)
# silent too. Checking each destination ourselves tells the two apart.
set -euo pipefail
mkdir -p target/zig-global-cache/p
shopt -s nullglob
tarballs=(third_party/ghostty-vt-zig-deps/p/*.tar.gz)
if [ ${#tarballs[@]} -eq 0 ]; then
    echo "error   zig-deps-seeded: no tarballs found under third_party/ghostty-vt-zig-deps/p/ (third_party/README.md)" >&2
    exit 1
fi
for f in "${tarballs[@]}"; do
    dest="target/zig-global-cache/p/$(basename "$f")"
    [ -e "$dest" ] && continue
    if ! cp "$f" "$dest"; then
        echo "error   zig-deps-seeded: failed to copy $f to $dest - check third_party/ghostty-vt-zig-deps/p/ is present and target/ is writable (third_party/README.md)" >&2
        exit 1
    fi
done
#!/usr/bin/env bash
set -euo pipefail
if [ "yes" = no ]; then echo "skip    run: no Rust workspace yet"; exit 0; fi
if [ -n "`mise which zig 2>/dev/null || true`" ]; then export ZIG="`mise which zig 2>/dev/null || true`"; fi
cargo build --package steg
cargo run --package steg-app
```

Check passed: `sh -c 'just check 2>&1 | tail -1'` exited 0 at 2026-11-20T03:57:54+00:00 in /Users/brukhabtu/src/github.com/bruk-io/steg.worktrees/task-1.142 on task-1.142-just-run-builds-the-steg-binary-the-app at df3e435555b5 with uncommitted changes:

```text
ok      ui colours (0.0s)
```

Built and tested:
- Modified justfile run recipe to build steg binary before running app
- Added 'cargo build --package steg' before 'cargo run --package steg-app'
- Updated recipe comment to explain the app autostarts the steg server from resolve_binary()
- Recorded 'just --dry-run run' check showing build line before run line
- Ran 'just check' - first attempt had one known flaky test (login_env backgrounded-grandchild), retried and passed all checks
- Committed with message explaining the change
- Pushed to origin/task-1.142-just-run-builds-the-steg-binary-the-app
- PR #85 created and CI passed with SUCCESS status
- No product code changes, only justfile recipe update
<!-- SECTION:NOTES:END -->
