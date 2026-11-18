---
id: DRAFT-1
title: 'Rule: record checks and controls at the final head, not a pre-rebase commit'
status: Draft
assignee: []
created_date: '2026-11-18T23:50:00Z'
labels:
  - rule-change
dependencies: []
references:
  - TASK-1.141
  - TASK-1.135
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found twice in review: a recorded check or negative control is timestamped at an earlier, pre-rebase/pre-format-fix commit rather than the branch's final pushed head, in TASK-1.141 and TASK-1.135.

TASK-1.135's review (#3, @bruk.r2) flagged this as [Evidence, Nit] after verifying the pre-merge commit's code was byte-identical to the final head. TASK-1.141's case is worse: the plan itself (step 5) explicitly required each new functional test's negative control to be run "at the final head... not earlier in the sequence," and the task's own notes claimed this was done there; the board record instead shows both the generic not-yet-arm control and the dedicated --all-filter control recorded at commit 18da1c97edb1 "with uncommitted changes," before that same commit's own `just check` failed on formatting in the very same test file, and before three further rebases to the final pushed head. No control was ever re-run after the format fix or at the final head.

Add to plugins/team/rules/evidence.md (new file, applies to every repository): When a plan or an acceptance criterion requires a negative control or a check "at the final head," record it only after the branch's last content-changing commit (including a formatting fix) and after any further rebase, immediately before push - not mid-sequence. If a check is recorded against an intermediate commit for any reason, diff that commit's code against the final head (not just compare hashes) before relying on it as current evidence, and re-run it if the diff is non-empty. The reason: the board's shared, constantly-rebased commit history makes commit hashes alone misleading evidence of "when" a check ran; only a content diff against the final head tells a reviewer whether recorded evidence still applies.
<!-- SECTION:DESCRIPTION:END -->
