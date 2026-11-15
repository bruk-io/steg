"""Parsing a `read-project-summary` response into a `ProjectSummary`."""

import pytest

from steg_tools.likec4_mcp import ProjectSummary, parse_project_summary

INITIALIZE_RESPONSE = {"jsonrpc": "2.0", "id": 1, "result": {"serverInfo": {"name": "LikeC4"}}}

SUMMARY_RESPONSE = {
    "jsonrpc": "2.0",
    "id": 2,
    "result": {
        "structuredContent": {
            "title": "architecture",
            "views": [{"id": "index", "title": "System Landscape", "type": "element"}],
            "elements": [{"id": "steg", "kind": "softwareSystem", "title": "steg", "tags": []}],
        }
    },
}


def test_pulls_title_view_ids_and_element_ids_regardless_of_response_order() -> None:
    expected = ProjectSummary(title="architecture", view_ids=frozenset({"index"}), element_ids=frozenset({"steg"}))
    assert parse_project_summary([INITIALIZE_RESPONSE, SUMMARY_RESPONSE]) == expected
    assert parse_project_summary([SUMMARY_RESPONSE, INITIALIZE_RESPONSE]) == expected


def test_missing_summary_response_raises_lookup_error() -> None:
    with pytest.raises(LookupError, match="id 2"):
        parse_project_summary([INITIALIZE_RESPONSE])


def test_tool_error_raises_lookup_error() -> None:
    error_response = {"jsonrpc": "2.0", "id": 2, "result": {"isError": True, "content": [{"type": "text", "text": "boom"}]}}
    with pytest.raises(LookupError, match="did not return a result"):
        parse_project_summary([INITIALIZE_RESPONSE, error_response])
