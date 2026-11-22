---
id: TASK-1.154
title: >-
  Move steg to a GPUI that can draw off screen, pinned to a commit of Zed's own
  repository
status: To Do
assignee: []
created_date: '2026-11-22T07:18:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-3
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split out of TASK-1.151 by its plan (22 November 2026). TASK-1.151's spike showed that drawing off screen and reading the frame back works on a newer GPUI (HeadlessAppContext::capture_screenshot gave a PNG with exact pixels, no window shown) and is closed on the pinned gpui 0.2.2, whose test window does not draw. The official gpui crate on crates.io is still 0.2.2. The spike used gpui-pre 0.3.13, a republish owned by one account that is not Zed's; two of its files were checked byte-identical to zed-industries/zed at the commit it claims, which is a spot check, not an audit. Recommended route, for the engineer to confirm on accepting: a git dependency pinned to that commit of Zed's own repository, so no third party sits in the chain, at the cost of a large first clone (the repository is about 546 MiB) and of tracking an unreleased GPUI. gpui-pre is the fallback only if the git source proves unworkable, and choosing it comes back to the engineer first. Not established: whether steg-boundaries handles a git source; the full size of the API difference (public functions in window, app and canvas looked additive in a diff). Licence rule: GPUI is Apache-2.0; nothing is taken from Zed's GPL crates, and a git dependency on the monorepo must not pull any of them into the build - the plan shows that from the licence check's own output.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The workspace builds against GPUI from zed-industries/zed at one pinned commit, named in the root Cargo.toml with the reason; the app and the gallery build and every existing test passes; just check passes
- [ ] #2 The licence check and the boundaries check both cope with the new source and its split platform crates, with every new boundaries.toml entry given its reason; no exception is added to either check to make it pass
- [ ] #3 The off-screen drawing support is reachable only from a test or tool target: the shipped app binary is shown not to carry the test-support feature, by a check that fails if it does
- [ ] #4 The notes list every source change the move forced in crates/app and crates/ui, and anything in the newer GPUI that changes how the app looks or behaves; the engineer does a live run before merge
<!-- AC:END -->
