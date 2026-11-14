#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.15"
# dependencies = []
# ///

"""Time the two candidates, so doc-2 section 7's proposals can be replaced with numbers.

The method, so the numbers can be reproduced or disbelieved:

- Release builds, run as separate processes, exactly as a person or an agent runs them.
- What is timed is the whole process: `time.perf_counter()` around `subprocess.run`, from
  spawn to exit. That includes the fork and exec the shell pays for too.
- 50 runs each by default, after 5 discarded warm-up runs.
- Reported as the lowest, the median and the 95th percentile, in milliseconds. The lowest
  is the honest "how fast can this be"; the 95th percentile is what doc-2 section 7 and
  R19 are written against.
- The machine, the Rust version and the date are printed with the numbers.

Usage: measure.py <directory of the release binaries> <socket>
"""

import json
import os
import platform
import statistics
import subprocess
import sys
import time
from datetime import UTC, datetime
from pathlib import Path

RUNS = 50
WARMUP = 5


def timings(command: list[str], environment: dict[str, str], runs: int = RUNS) -> list[float]:
    """Run a command repeatedly and return each run's wall time in milliseconds."""
    for _ in range(WARMUP):
        subprocess.run(command, env=environment, capture_output=True, check=False)
    taken: list[float] = []
    for _ in range(runs):
        started = time.perf_counter()
        subprocess.run(command, env=environment, capture_output=True, check=False)
        taken.append((time.perf_counter() - started) * 1000)
    return taken


def summary(taken: list[float]) -> dict[str, float]:
    """The lowest, median and 95th percentile of a set of timings, in milliseconds."""
    ordered = sorted(taken)
    at_95 = ordered[min(len(ordered) - 1, int(round(0.95 * len(ordered))) - 1)]
    return {
        "lowest_ms": round(ordered[0], 2),
        "median_ms": round(statistics.median(ordered), 2),
        "p95_ms": round(at_95, 2),
    }


def status_of(command: list[str], environment: dict[str, str]) -> int:
    """Run a command once and return its exit status."""
    return subprocess.run(command, env=environment, capture_output=True, check=False).returncode


def payload(binaries: Path, socket: str) -> dict[str, int]:
    """Measure the description a run-time client has to fetch and parse on every run.

    doc-2 section 8 lists 30 commands for the first release against this spike's 3, so
    the figure for a full API is given as the measured size scaled by the method count.
    That is an extrapolation, not a measurement.
    """
    done = subprocess.run(
        [str(binaries / "spike_docs"), "--json", "--from-server"],
        env={"STEG_SERVER": socket},
        capture_output=True,
        text=True,
        check=True,
    )
    api = json.loads(done.stdout)
    bytes_now = len(done.stdout.encode())
    methods = len(api["methods"])
    return {
        "methods": methods,
        "bytes": bytes_now,
        "bytes_per_method": round(bytes_now / methods),
        "bytes_extrapolated_to_30_methods": round(bytes_now / methods * 30),
    }


def main() -> int:
    """Measure both candidates and print one JSON object of results."""
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    binaries = Path(sys.argv[1])
    socket = sys.argv[2]
    a = str(binaries / "spike_cli_a")
    b = str(binaries / "spike_cli_b")

    live = {"STEG_SERVER": socket}
    dead = {"STEG_SERVER": "/tmp/steg-no-server-at-all.sock"}
    split = ["pane", "split", "logs", "--direction", "right"]

    results = {
        "measured_at": datetime.now(UTC).isoformat(timespec="seconds"),
        "machine": f"{platform.machine()} {platform.system()} {platform.release()}",
        "rustc": subprocess.run(
            ["rustc", "--version"], capture_output=True, text=True, check=False
        ).stdout.strip(),
        "runs": RUNS,
        "warmup": WARMUP,
        # Every run carries the load it was taken under, because this machine also builds
        # other tasks' worktrees. The comparison between the candidates is within one run
        # and stays valid; the absolute numbers only mean something beside this.
        "load_average_before": [round(value, 2) for value in os.getloadavg()],
        "description_payload": payload(binaries, socket),
        "help_with_no_server": {
            "candidate_a": {
                "exit_status": status_of([a, "pane", "split", "--help"], dead),
                **summary(timings([a, "pane", "split", "--help"], dead)),
            },
            "candidate_b": {
                "exit_status": status_of([b, "pane", "split", "--help"], dead),
                **summary(timings([b, "pane", "split", "--help"], dead)),
            },
        },
        "help_with_a_server_up": {
            "candidate_a": summary(timings([a, "pane", "split", "--help"], live)),
            "candidate_b": summary(timings([b, "pane", "split", "--help"], live)),
        },
        "version": {
            "candidate_a": summary(timings([a, "--version"], dead)),
            "candidate_b": {
                "exit_status": status_of([b, "--version"], dead),
                **summary(timings([b, "--version"], dead)),
            },
        },
        "a_call_against_a_live_server": {
            "candidate_a": summary(timings([a, *split], live)),
            "candidate_b": summary(timings([b, *split], live)),
        },
    }
    results["load_average_after"] = [round(value, 2) for value in os.getloadavg()]
    print(json.dumps(results, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
