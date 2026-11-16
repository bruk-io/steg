---
id: DRAFT-5
title: >-
  Repository plugin: a layer and a rule for each of foundations, atoms,
  molecules and organisms
status: Draft
assignee: []
created_date: '2026-11-16T00:52:00Z'
labels:
  - size-2
milestone: m-4
dependencies: []
parent_task_id: TASK-1
project: bruk-io-steg
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
doc-17 (the four levels, R118, R125, R126). The plugin is plugins/bruk-io-steg, changed only through asdlc-plugin (the onboard-repo skill). The molecule-or-organism question is the one people will dispute: the rule's wording for it is doc-17's (one unit, or an arrangement of several that could each stand alone). Needs at least one real component per level to point at, or says which it will point at when it exists.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The repository plugin has a layer for crates/ui as a whole and one for each of its four levels, narrow before wide, and one rule per level delivered when a file of that level is touched (R126)
- [ ] #2 Each rule leads with its level's test from doc-17, so that reading it settles whether a thing is an atom, a molecule or an organism; then what the level may use, that a component is given plain values and reports what the user did (R118), accessibility for anything a person acts on (R125), and one file in the repository that shows the pattern
- [ ] #3 No rule restates what a check enforces (the dependency boundary, import direction, colour literals, the generated file); each names the check instead. The rules are changed through asdlc-plugin and pass its check against the repository
<!-- AC:END -->
