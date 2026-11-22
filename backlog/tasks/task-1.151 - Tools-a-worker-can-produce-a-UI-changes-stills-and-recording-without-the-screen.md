---
id: TASK-1.151
title: >-
  Tools: a worker can produce a UI change's stills and recording without the
  screen
status: In Progress
assignee:
  - '@bruk.w3'
created_date: '2026-11-22T04:52:00Z'
updated_date: '2026-11-22T07:00:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-1
asdlc_branch: task-1.151-tools-a-worker-can-produce-a-ui-change-s
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Asked for by the engineer on 22 November 2026: UI pull requests need an image, and a recording for motion, but workers must not launch the app, inject input or capture the screen on the engineer's machine, so today every UI change waits for the engineer to record it by hand (TASK-1.127, steg PR 95). Unknowns dominate, so the plan starts with a spike and comes back with the route before building. Routes, in the order to try: (A) draw off screen in process and read the frames back, driven by a controlled clock. Checked today: the pinned gpui 0.2.2 has only the test context that does not really draw; newer GPUI lines carry a visual test context and a headless app context that look like this, so A may mean a GPUI upgrade, which is its own decision for the engineer. (B) run the real app on a macOS CI runner against a scenario and capture only its own window there, uploading the result as a build artefact: nothing of the engineer's is on that machine, at the cost of a slow loop. (C) on the engineer's machine, capture only the app's own window by window id in a throwaway home: still puts a window on their screen and needs screen-recording permission, so it is the fallback. The plan names the route, what it costs, and anything that is the engineer's to decide. Licence: read Zed and GPUI to learn the API, copy nothing from Zed's GPL crates.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One just recipe takes a named scenario and writes stills, and a recording when the scenario has motion, of the app's own drawing, with no window on the user's screen, no keyboard or mouse input injected, no screen capture and no clipboard use on the machine it runs on
- [ ] #2 A scenario is data in the repository: an explicit pane program with fixed content, a throwaway home, never the login shell, and a scripted timeline (keys, waits, focus changes) on a clock the tool controls, so the same scenario gives the same frames every run and nothing personal can appear
- [ ] #3 The cursor dissolve of TASK-1.127 is the first scenario: its recording and its stills of each step come out of the tool and match what the engineer recorded by hand
- [ ] #4 The team's rules say a UI pull request's image comes from this tool, and the worker brief's ban on launching the app stays
<!-- AC:END -->
