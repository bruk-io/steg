#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.15"
# dependencies = []
# ///

"""Call steg's API from outside Rust, knowing only what the server describes.

This stands in for a plugin (R42, R75): a separate Python process on the public API, with
no private hook and nothing about the API written down on this side. It asks the server
what the API is, finds the method, builds the call from the parameters the description
declares, and prints what came back.

Usage: client.py <socket> [method]
"""

import json
import socket
import sys
from typing import Any

DESCRIBE = "api.describe"


def call(path: str, method: str, params: dict[str, Any]) -> Any:
    """Make one call over the stand-in transport and return its result.

    Raises:
        RuntimeError: The server answered with an error.
    """
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.connect(path)
        connection.sendall((json.dumps({"method": method, "params": params}) + "\n").encode())
        with connection.makefile("r") as lines:
            answer = json.loads(lines.readline())
    if "error" in answer:
        raise RuntimeError(answer["error"]["message"])
    return answer["ok"]


def arguments_for(method: dict[str, Any], target: str) -> dict[str, Any]:
    """Build a call's parameters from what the description says the method takes.

    Nothing here names a parameter of steg's API: a required choice takes the first word
    the description offers, a required target takes the one given, and anything optional
    is left out. That is the test of whether the description is enough on its own.
    """
    given: dict[str, Any] = {}
    for param in method["params"]:
        kind = param["value"]["kind"]
        if param["role"] == "target":
            given[param["name"]] = target
        elif param["required"] and kind == "choice":
            given[param["name"]] = param["value"]["of"][0]
        elif param["required"]:
            raise RuntimeError(f"this script cannot invent a {kind} for {param['name']}")
    return given


def main() -> int:
    """Describe the API, call one method, and print what it returned."""
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    path = sys.argv[1]
    wanted = sys.argv[2] if len(sys.argv) > 2 else "pane.split"

    api = call(path, DESCRIBE, {})
    print(f"the server speaks API {api['api_version']['major']}.{api['api_version']['minor']}")
    print(f"it has {len(api['methods'])} methods, {len(api['events'])} events, {len(api['errors'])} errors")

    by_name = {f"{m['name']['noun']}.{m['name']['verb']}": m for m in api["methods"]}
    method = by_name.get(wanted)
    if method is None:
        print(f"{wanted} is not in the description: {', '.join(sorted(by_name))}", file=sys.stderr)
        return 2

    print(f"\ncalling {wanted}: {method['summary']}")
    for param in method["params"]:
        need = "required" if param["required"] else "optional"
        print(f"  {param['name']} ({param['role']}, {param['value']['kind']}, {need}): {param['summary']}")

    params = arguments_for(method, target="logs")
    print(f"\nsending {json.dumps(params)}")
    print(f"got {json.dumps(call(path, wanted, params))}")

    print("\nthe events a plugin could subscribe to, from the same description:")
    for event in api["events"]:
        print(f"  {event['name']}: {event['summary']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
