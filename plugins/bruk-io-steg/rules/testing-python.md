---
paths:
  - "tools/tests/**/*.py"
  - "tools/src/**/*.py"
---
# Testing the Python tools

Applies to the tools in tools/ and their tests.

- `just test-tools` runs them. One file: `uv run --project tools pytest -q tools/tests/unit/test_affected.py`.
- `tools/tests/unit/` tests the decision functions with plain data, as a table of cases with `pytest.mark.parametrize` (see `test_affected.py`).
- `tools/tests/adapter/` runs a tool against this repository's real workspace, with no doubles. A new tool that reads git or cargo gets one test there.
