---
id: TASK-1.73
title: 'Rule: a just recipe is named with a verb'
status: In Progress
assignee:
  - '@bruk.w2'
created_date: '2026-11-15T03:12:00Z'
updated_date: '2026-11-15T03:19:00Z'
labels:
  - size-2
milestone: m-0
dependencies:
  - TASK-1.33
parent_task_id: TASK-1
type: task
project: bruk-io-steg
asdlc_accepted_from: DRAFT-2
asdlc_branch: task-1.73-rule-a-just-recipe-is-named-with-a-verb
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Asked for by Bruk on 2026-11-15: this applies to justfiles, and a just recipe needs to be a verb. A recipe is something you tell the repository to do, so its name should say the action: `just check`, `just build`, `just test`. Today several are nouns or adjectives (`boundaries`, `packages`, `affected`, and `architecture` once TASK-1.33 merges), which read as things, not commands.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A rule scoped to the justfile says a recipe's name starts with a verb in the imperative, with the reason, and gives the pattern for a variant (verb first, then what it acts on, such as test-all)
- [ ] #2 Every existing recipe that does not start with a verb is renamed, and the plan lists each old and new name for the engineer to approve before anything is renamed
- [ ] #3 Everything that names a renamed recipe is updated in the same change: CLAUDE.md, the rules, the CI workflow, the tools and their tests; the repository plugin's commands are listed for the orchestrator to change through asdlc-plugin
- [ ] #4 A check in tools/, run by just check, fails when a recipe's first word is not on a short list of verbs kept beside it, with a message that gives the rule; the list is extended by editing it, never by skipping the check
- [ ] #5 just check passes, and the old names are gone: no alias is left behind
<!-- AC:END -->
