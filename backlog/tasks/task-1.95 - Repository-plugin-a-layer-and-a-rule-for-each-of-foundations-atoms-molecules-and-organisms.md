---
id: TASK-1.95
title: >-
  Repository plugin: a layer and a rule for each of foundations, atoms,
  molecules and organisms
status: To Do
assignee: []
created_date: '2026-11-16T00:52:00Z'
updated_date: '2026-11-16T06:21:00Z'
labels:
  - size-2
milestone: m-4
dependencies:
  - TASK-1.91
  - TASK-1.93
parent_task_id: TASK-1
project: bruk-io-steg
asdlc_accepted_from: DRAFT-5
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-15, @bruk.o: the first real components are TASK-1.100 (atoms), TASK-1.103 (molecules) and TASK-1.106 (organisms); each level's rule points at one file from those when it exists.

2026-11-15, @bruk.o, from TASK-1.94's review: the repository plugin's tools rule (plugins/bruk-io-steg/rules/tools.md) says the tools use only the standard library. TASK-1.94 adds ast-grep-py as the tools' first runtime dependency, approved by the engineer. This task changes the plugin through asdlc-plugin anyway: bring that line up to date here (the tools may take a dependency the engineer has approved; ast-grep-py is the one today, for parsing Rust), so the rule and the code agree.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-11-16T06:21:00Z

---

2026-11-16, @bruk.o: from TASK-1.93's third review. The check that no public function returns a bare colour (foundations_public_surface.py) does not see a pub const or static of the colour type, a renamed import of it, or a type alias; nothing in the tree uses those today. When this task writes the foundations rule, either the rule states that limit for a reader, or the check is widened first (an ast-grep rule over public items whose type resolves to the colour type would do it). Also: the terminal themes' colour sets are public with public fields, which the reviewer judged outside R123 because each is one fixed set and not a light and dark pair: say so in the rule.

---
<!-- COMMENTS:END -->
