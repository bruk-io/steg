"""Refuse an import inside `crates/ui/src` that reaches from one of the four levels
doc-17 names - foundations, atoms, molecules, organisms, in that order - into a level
that comes after it (R119).

Reads each file's real syntax tree (`ast_grep_py.SgRoot(text, "rust")`), not its text:
a `use` or a path inside a comment or a string literal is never mistaken for one. The
gallery (`crates/ui/gallery`) is not scanned at all - it isn't one of the four levels
(doc-17's own table), and CLAUDE.md already says its chrome is exempt from the level
rules; this check simply never looks there, since `crates/ui/src` is a sibling
directory, not a parent of it.

What this reads, and what it cannot, is in `resolve_path`'s and `crate_root_exports`'s
own docstrings.

**What this cannot see, named outright**: a path written as a macro's *argument*, such
as `some_macro!(crate::atoms::Button)`. Tree-sitter hands a macro invocation's
parenthesised body back as one opaque `token_tree` of bare tokens (`crate`, `::`,
`atoms`, ...), never reconstructed into the `scoped_identifier` node this module
searches for - confirmed empirically, not assumed: parsing that exact line finds zero
`scoped_identifier` nodes anywhere in it. This is a different gap from a macro that
*expands to* a cross-level reference (which no static parse can see either, since
seeing it needs the expansion, not just the source as written) - this one is a path
already sitting in the source, verbatim, that this check simply never looks inside.

Usage: steg-ui-levels
"""

import sys
from dataclasses import dataclass

from ast_grep_py import SgNode, SgRoot

from steg_tools import workspace
from steg_tools.rust_source import RustFile, rust_files

LEVELS = ("foundations", "atoms", "molecules", "organisms")
RANK = {level: index for index, level in enumerate(LEVELS)}

UI_SRC = "crates/ui/src"
UI_LIB_RS = f"{UI_SRC}/lib.rs"


@dataclass(frozen=True)
class Violation:
    """An import that reaches from `importing_level` into `reached_level`, which comes after it."""

    path: str
    line: int
    use_text: str
    importing_level: str
    reached_level: str


def level_of_file(path: str) -> str | None:
    """Say which of the four levels owns a `crates/ui/src` file, or `None`.

    `None` for `lib.rs` itself and for anything outside `crates/ui/src` - neither is a
    component of a level. `None` too for a `.rs` file placed directly under `src/`
    instead of inside one of the four level directories; `unplaced_files` below is what
    refuses that loudly, rather than this function treating it the same as a deliberate
    exemption.
    """
    prefix = f"{UI_SRC}/"
    if not path.startswith(prefix) or path == UI_LIB_RS:
        return None
    first_segment = path[len(prefix) :].split("/", 1)[0]
    return first_segment if first_segment in RANK else None


def unplaced_files(files: tuple[RustFile, ...]) -> tuple[str, ...]:
    """Say which files are under `crates/ui/src` but inside none of the four level directories."""
    prefix = f"{UI_SRC}/"
    return tuple(
        sorted(
            file.path
            for file in files
            if file.path.startswith(prefix) and file.path != UI_LIB_RS and level_of_file(file.path) is None
        )
    )


def first_level_segment(path_text: str) -> str | None:
    """The first segment of a `crate::`-rooted path, when it names one of the four levels."""
    segments = path_text.removeprefix("crate::").split("::")
    return segments[0] if segments[0] in RANK else None


def crate_root_exports(lib_rs_text: str) -> dict[str, str]:
    """Map a name `lib.rs` re-exports at crate root to the level it names it from.

    Reads every `pub use <level>::<path>;` and `pub use <level>::<path> as <alias>;` at
    crate root, keyed by the re-exported name (the alias when there is one, else the
    path's last segment). This closes the one loophole a per-file text match alone would
    miss entirely: without it, `foundations` could reach `atoms::Button` through
    `use crate::Button;` the moment `lib.rs` re-exports it, and nothing in that file's
    own text would ever say "atoms".

    What this cannot see, deliberately: a glob re-export (`pub use atoms::*;`) isn't
    expanded - that needs to know every item `atoms` exports, which needs the compiler,
    not a parse - and neither is a grouped re-export
    (`pub use crate::{atoms::Button, molecules::Tab};`), a narrower cut than the
    per-file `use` reader below takes. `lib.rs` has no `pub use` of either shape today
    (only `pub mod`), so closing either form costs nothing to add later, the moment the
    app needs its first grouped or glob re-export.
    """
    exports: dict[str, str] = {}
    for use in SgRoot(lib_rs_text, "rust").root().find_all(kind="use_declaration"):
        if not any(child.kind() == "visibility_modifier" for child in use.children()):
            continue  # a plain `use`, not `pub use` - doesn't change what crate root exports
        argument = use.field("argument")
        if argument is None:
            continue
        match argument.kind():
            case "use_as_clause":
                path_node, alias_node = argument.field("path"), argument.field("alias")
                if path_node is None or alias_node is None:
                    continue
                path_text, name = path_node.text(), alias_node.text()
            case "scoped_identifier" | "identifier":
                path_text = argument.text()
                name = path_text.rsplit("::", 1)[-1]
            case _:
                continue  # a glob or a grouped re-export - not expanded, see this docstring
        level = first_level_segment(path_text)
        if level is not None:
            exports[name] = level
    return exports


def resolve_path(path_text: str, exports: dict[str, str]) -> str | None:
    """Say which level a `use` path or a fully-qualified expression path reaches into, or `None`.

    `crate::<level>::...` resolves directly. `crate::<name>` where `<name>` isn't one of
    the four level names is looked up in `exports` (`crate_root_exports`, built once from
    `lib.rs`). `super::<level>::...` resolves the same way a `crate::<level>::...` path
    would, with no importing-file context needed: every real file today is a level's own
    `mod.rs`, one hop below crate root, so a single `super::` already names a sibling
    level directly in its very next segment - the same segment a `crate::<level>::...`
    path names in its first.

    **Known limitation**: a `super::super::` chain's meaning depends on the importing
    file's nesting depth under its own level directory - information this function
    doesn't have and doesn't ask for, so it returns `None` for a second `super::` hop
    rather than guess. Revisit the moment a level first grows its own submodule
    (TASK-1.95 or later), at which point this function's signature likely needs the
    importing file's own level or depth added back.

    Anything else (`self::...`, an external crate such as `gpui::...`, a bare name with
    no `crate`/`super` prefix) is `None` - not one of the four levels, or not something
    this function can attribute without the compiler.
    """
    if path_text == "crate" or path_text.startswith("crate::"):
        level = first_level_segment(path_text)
        if level is not None:
            return level
        name = path_text.removeprefix("crate::").split("::", 1)[0]
        return exports.get(name)
    if path_text.startswith("super::"):
        rest = path_text.removeprefix("super::")
        if rest.startswith("super::"):
            return None
        segment = rest.split("::", 1)[0]
        return segment if segment in RANK else None
    return None


def _flatten_use_argument(node: SgNode) -> tuple[str, ...]:
    """Every path one `use` argument names, flattened out of a grouped list, any alias dropped."""
    match node.kind():
        case "use_wildcard":
            children = node.named_children()
            return (children[0].text(),) if children else ()
        case "use_as_clause":
            path = node.field("path")
            return _flatten_use_argument(path) if path is not None else ()
        case "scoped_identifier" | "identifier":
            return (node.text(),)
        case "use_list":
            return tuple(path for child in node.named_children() for path in _flatten_use_argument(child))
        case "scoped_use_list":
            prefix, listing = node.field("path"), node.field("list")
            if prefix is None or listing is None:
                return ()
            return tuple(f"{prefix.text()}::{tail}" for tail in _flatten_use_argument(listing))
        case _:
            return ()


def use_paths(declaration: SgNode) -> tuple[str, ...]:
    """Every path one `use_declaration` names."""
    argument = declaration.field("argument")
    return _flatten_use_argument(argument) if argument is not None else ()


def expression_paths(root: SgNode) -> tuple[SgNode, ...]:
    """Every fully-qualified path (`crate::...`, `super::...`) written directly in an
    expression or a type, outside any `use` declaration.

    Only the outermost `scoped_identifier` at each site: `crate::atoms::Button::new`
    nests `crate::atoms::Button`, which nests `crate::atoms` - taking all three would
    report the same crossing three times for one reference.
    """
    return tuple(
        node
        for node in root.find_all(kind="scoped_identifier")
        if (node.parent() is None or node.parent().kind() != "scoped_identifier")
        and not any(ancestor.kind() == "use_declaration" for ancestor in node.ancestors())
    )


def _line(node: SgNode) -> int:
    """A node's 1-indexed starting line."""
    return node.range().start.line + 1


def violations(files: tuple[RustFile, ...], exports: dict[str, str]) -> tuple[Violation, ...]:
    """Say which files import a level that comes after their own (R119)."""
    found = []
    for file in files:
        level = level_of_file(file.path)
        if level is None:
            continue
        root = SgRoot(file.text, "rust").root()
        references: list[tuple[SgNode, str]] = [
            (declaration, path)
            for declaration in root.find_all(kind="use_declaration")
            for path in use_paths(declaration)
        ]
        references.extend((node, node.text()) for node in expression_paths(root))
        for node, path_text in references:
            reached = resolve_path(path_text, exports)
            if reached is not None and RANK[reached] > RANK[level]:
                found.append(Violation(file.path, _line(node), node.text(), level, reached))
    return tuple(sorted(found, key=lambda violation: (violation.path, violation.line)))


def format_violation(violation: Violation) -> str:
    """Report one violation, naming both levels, the import, and R119's own rule."""
    return (
        f"{violation.path}:{violation.line}: `{violation.use_text}` reaches into "
        f"{violation.reached_level} from {violation.importing_level}, which doc-17 R119 refuses "
        f"(a level uses only the levels before it: {' < '.join(LEVELS)})."
    )


def format_unplaced(path: str) -> str:
    """Report a `.rs` file under `crates/ui/src` that is in none of the four level directories."""
    return (
        f"{path} is not inside one of the four level directories doc-17 names ({', '.join(LEVELS)}) "
        "- give it one before this check can tell which rule binds it."
    )


def main() -> int:
    """Check the real `crates/ui/src` tree against R119."""
    root = workspace.repo_root()
    if root is None:
        print("steg-ui-levels: not in a git repository", file=sys.stderr)
        return 2
    lib_rs = root / UI_LIB_RS
    exports = crate_root_exports(lib_rs.read_text()) if lib_rs.is_file() else {}
    files = rust_files(root, UI_SRC)
    unplaced = unplaced_files(files)
    for path in unplaced:
        print(format_unplaced(path))
    found = violations(files, exports)
    for violation in found:
        print(format_violation(violation))
    return 1 if found or unplaced else 0
