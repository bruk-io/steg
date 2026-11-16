---
id: TASK-1.92
title: 'ui: the design''s named values as one data file, exported from the artifact'
status: In Progress
assignee:
  - '@bruk.w6'
created_date: '2026-11-16T00:52:00Z'
updated_date: '2026-11-16T00:55:00Z'
labels:
  - size-3
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.92-ui-the-design-s-named-values-as-one-data
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-17 (R121, R122, the section 'The design, and where its values are today'), decision-10, decision-11. The design artifact holds its foundations as drawn pages only; the data and generator scripts behind them were working files of the session that built it. The orchestrator checked on 2026-11-15 that the copies at /Users/brukhabtu/.claude/projects/-Users-brukhabtu-src-github-com-bruk-io-steg/design-canvas-backup-2026-11-15 (foundations.json, gen_system.py and the rest, and project/Tokens.dc.html and Details.dc.html) are identical to the artifact's version 1794702644-d50e for the two foundations pages and that all 17 colours appear in the artifact's page. A worker cannot read the artifact itself: work from that copy, and the orchestrator re-reads the artifact before the pull request merges. Whether the data is also published into the artifact is an open question in doc-17 for the engineer and not this task's.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 crates/ui/design/foundations.json holds every named value of the design's foundations under the design's own names: the 17 colours with their light and dark forms, the two ramps, type, sizes, corners, durations, both terminal themes with their 16 ANSI colours and base colours, and the focus ring's measurements; and it records the artifact version it was exported from (R121, R122)
- [ ] #2 Values the design's working data holds as sentences (type, sizes, the ramps, the themes' base colours) are given a structured form: each is a number with its unit, a colour, or a list of them, and nothing a program must parse out of prose. The plan lists each such value with the form it takes, for the engineer, and anything the design does not actually fix is listed as a question and not guessed
- [ ] #3 The working data and the scripts that drew the design's pages have a durable home in the repository, with a README saying what they are and that the artifact is the source; a tested tool checks that every colour in the data file appears in the saved copy of the artifact's foundations page under the same name and value
- [ ] #4 No value is changed in the export: the pull request says so, and a reviewer can compare the data file with the artifact's pages by name
<!-- AC:END -->
