"""`likec4 mcp --stdio` against this repository's real model, not a made-up fixture.

The unit suite in tests/unit/test_likec4_mcp.py proves `parse_project_summary` against
responses nobody asked the server to produce. This is the other half: the real LikeC4
MCP server, pinned to the version `.mcp.json` runs, answering for the real
`architecture/` model - the mechanical proof that a session can list this model's
views and elements through it (TASK-1.72, doc-16 R108).
"""

from steg_tools import workspace
from steg_tools.likec4_mcp import PINNED_LIKEC4_VERSION, read_project_summary

# The dynamic views and the four C4-level views doc-16 R108 asks the model for, plus the
# system landscape ("index"). Read from the live server, not copied from the .c4 sources,
# so this test fails if the model ever drops one.
EXPECTED_VIEW_IDS = frozenset(
    {
        "index",  # System Landscape
        "stegContext",  # System Context - steg
        "stegContainers",  # Containers - steg
        "stegPackages",  # Components - the packages
        "stegDeployment",  # Deployment - steg
        "attachToAPane",  # Dynamic - attaching to a pane
        "focusAPane",  # Dynamic - focusing a pane
    }
)

# One component id per package doc-16's Level 3 table lists (sdk, kernel, server, `steg`,
# app), confirming every package the architecture model describes shows up as an element,
# not just the containers around them.
EXPECTED_PACKAGE_ELEMENT_IDS = frozenset(
    {
        "steg.server.sdk",
        "steg.server.kernel",
        "steg.server.serverPkg",
        "steg.cli.stegPkg",
        "steg.app.appPkg",
    }
)


def test_a_session_can_list_this_repositorys_views_and_elements_through_the_mcp_server() -> None:
    root = workspace.repo_root()
    assert root is not None, "this test runs inside a git checkout"

    summary = read_project_summary(root / "architecture", likec4_version=PINNED_LIKEC4_VERSION)

    assert summary.title == "architecture"  # the project name .likec4rc declares
    assert EXPECTED_VIEW_IDS <= summary.view_ids
    assert EXPECTED_PACKAGE_ELEMENT_IDS <= summary.element_ids
