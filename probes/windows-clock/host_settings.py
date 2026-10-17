import json
import subprocess
import sys
from datetime import datetime

from windows_clock import NativeDates


def zone(*arguments: str) -> str:
    return subprocess.run(
        ["tzutil", *arguments], check=True, capture_output=True, text=True
    ).stdout.strip()


def main() -> None:
    if sys.argv[1:] != ["--exercise"]:
        raise SystemExit(
            "Use --exercise to temporarily change and restore the host time zone"
        )
    baseline = NativeDates()
    if not baseline.name or baseline.name not in baseline.clock.zones:
        raise ValueError("The original named time zone cannot be restored")
    original = baseline.name + ("_dstoff" if baseline.disabled else "")
    observations = []
    try:
        for name, expected_name, disabled, expected_hour in (
            ("UTC", "UTC", False, 17),
            ("Eastern Standard Time_dstoff", "Eastern Standard Time", True, 12),
            ("Eastern Standard Time", "Eastern Standard Time", False, 13),
        ):
            zone("/s", name)
            native = NativeDates()
            result = native.local(datetime(2026, 7, 1, 17))
            if (
                native.name != expected_name
                or native.disabled != disabled
                or result != datetime(2026, 7, 1, expected_hour)
            ):
                raise RuntimeError(
                    f"Unexpected snapshot: {native.name}, {native.disabled}, {result}"
                )
            observations.append(
                dict(
                    zone=native.name, disabled=native.disabled, local=result.isoformat()
                )
            )
    finally:
        zone("/s", original)
        snapshot = NativeDates()
        restored = snapshot.name + ("_dstoff" if snapshot.disabled else "")
        if (snapshot.name, snapshot.disabled) != (baseline.name, baseline.disabled):
            raise RuntimeError(
                f"Time zone restoration failed: {restored}; expected {original}"
            )
    print(json.dumps(dict(observations=observations, restored=restored), indent=2))


if __name__ == "__main__":
    main()
