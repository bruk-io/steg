---
paths:
  - "justfile"
---
# A recipe's name is a verb

Applies to steg's justfile.

- A recipe is something you tell the repository to do, so its name is the command: `just build`, `just test`, not `just packages`. Start it with a verb in the imperative.
- A variant names the verb first and what it acts on after: `test-all`, `test-tools`, `check-boundaries`.
- `check-recipes` (`tools/src/steg_tools/recipes.py`) is the mechanical half of this: it fails on any public recipe whose first word isn't on its list, with the reason in its message. Extend that list there when a recipe is genuinely a new action; never skip the check to make a name fit.
- A private recipe, such as just's own `default` (the bare `just`'s entry point, never typed by name), is exempt: the rule is about the word a person types, and nobody types that one.
