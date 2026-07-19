#!/usr/bin/env python3
"""Require a process to remain alive for the complete launch-smoke interval."""

from __future__ import annotations

import subprocess
import sys
import time


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: check-process-survival.py <seconds> <command> [args...]", file=sys.stderr)
        return 2

    try:
        required_seconds = float(sys.argv[1])
    except ValueError:
        print("seconds must be numeric", file=sys.stderr)
        return 2
    if required_seconds <= 0:
        print("seconds must be greater than zero", file=sys.stderr)
        return 2

    started = time.monotonic()
    try:
        process = subprocess.Popen(sys.argv[2:])
    except OSError as error:
        print(f"failed to launch process: {error}", file=sys.stderr)
        return 2

    deadline = started + required_seconds
    while True:
        elapsed = time.monotonic() - started
        return_code = process.poll()
        if return_code is not None:
            print(
                f"process exited with code {return_code} after {elapsed:.3f}s, "
                f"before the required {required_seconds:.3f}s interval",
                file=sys.stderr,
            )
            return 1
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        time.sleep(min(0.05, remaining))

    process.terminate()
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
    elapsed = time.monotonic() - started
    print(f"process remained alive for the required {required_seconds:.3f}s ({elapsed:.3f}s observed)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
