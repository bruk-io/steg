---
id: TASK-1.48
title: 'Server: remember settings changed from the palette'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T01:33:00Z'
updated_date: '2026-11-15T18:45:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.75
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-18
asdlc_branch: task-1.48-server-remember-settings-changed-from
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-10: R22, R80. decision-9. In the first release there is no config file, so every setting is an override the server remembers.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A setting can be read, changed and listed with its current value through the API
- [ ] #2 A change applies at once and is still there after the app is quit and reopened
- [ ] #3 No config file is read or written
<!-- AC:END -->
