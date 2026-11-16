"""`summary_from_command`'s two timeout bounds, against a fake child in place of a real
likec4 server (`tests/adapter/test_likec4_mcp_smoke.py` is the one test that runs the
real thing). TASK-1.97's review: the timeout split added a "mid-exchange" branch that
nothing on the task had ever reached - these two tests reach both branches, each
driven by a real but fake child process rather than a sleep-and-hope.
"""

import sys

import pytest

from steg_tools.likec4_mcp import summary_from_command

_NEVER_REPLIES = "import time; time.sleep(60)"

_REPLIES_ONCE_THEN_HANGS = (
    'import json, sys, time; print(json.dumps({"jsonrpc": "2.0", "id": 1, "result": {}})); '
    "sys.stdout.flush(); time.sleep(60)"
)


def test_a_stall_before_any_reply_names_the_start_up_phase() -> None:
    with pytest.raises(TimeoutError, match=r"\(to start\) within 0\.2s"):
        summary_from_command([sys.executable, "-c", _NEVER_REPLIES], startup_timeout=0.2, timeout=5.0)


def test_a_stall_after_the_first_reply_names_the_mid_exchange_phase() -> None:
    with pytest.raises(TimeoutError, match=r"\(mid-exchange\) within 0\.2s"):
        summary_from_command([sys.executable, "-c", _REPLIES_ONCE_THEN_HANGS], startup_timeout=5.0, timeout=0.2)
