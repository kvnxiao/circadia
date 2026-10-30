"""Exercise and restore the host time zone only with explicit CLI consent."""

import json
import shutil
import subprocess
import sys
from datetime import date, datetime, time
from pathlib import Path

from windows_clock import NativeDates


def _set_zone(executable: str, name: str) -> None:
    subprocess.run(  # noqa: S603 -- executable is resolved; names are fixed cases or the validated original zone
        [executable, "/s", name],
        check=True,
        capture_output=True,
        encoding="oem",
        cwd=Path(executable).parent,
        timeout=30,
    )


def main() -> None:
    """Check explicit host-zone changes and restore the original named zone."""
    if sys.argv[1:] != ["--exercise"]:
        message = "use --exercise to temporarily change and restore the host time zone"
        raise SystemExit(message)
    executable = shutil.which("tzutil")
    if executable is None:
        message = "tzutil is unavailable"
        raise FileNotFoundError(message)
    baseline = NativeDates()
    if not baseline.name or baseline.name not in baseline.clock.zones:
        message = "the original named time zone cannot be restored"
        raise ValueError(message)
    original = baseline.name + ("_dstoff" if baseline.disabled else "")
    observations = []
    try:
        for name, expected_name, disabled, expected_hour in (
            ("UTC", "UTC", False, 17),
            ("Eastern Standard Time_dstoff", "Eastern Standard Time", True, 12),
            ("Eastern Standard Time", "Eastern Standard Time", False, 13),
        ):
            _set_zone(executable, name)
            native = NativeDates()
            result = native.local(datetime.combine(date(2026, 7, 1), time(17)))
            if (
                native.name != expected_name
                or native.disabled != disabled
                or result != datetime.combine(date(2026, 7, 1), time(expected_hour))
            ):
                message = (
                    f"unexpected snapshot: {native.name}, {native.disabled}, {result}"
                )
                raise RuntimeError(message)
            observations.append(
                {
                    "zone": native.name,
                    "disabled": native.disabled,
                    "local": result.isoformat(),
                }
            )
    finally:
        _set_zone(executable, original)
        snapshot = NativeDates()
        restored = snapshot.name + ("_dstoff" if snapshot.disabled else "")
        if (snapshot.name, snapshot.disabled) != (baseline.name, baseline.disabled):
            message = f"time zone restoration failed: {restored}; expected {original}"
            raise RuntimeError(message)
    print(  # noqa: T201 -- CLI prints the exercise report
        json.dumps({"observations": observations, "restored": restored}, indent=2)
    )


if __name__ == "__main__":
    try:
        main()
    except subprocess.TimeoutExpired as error:
        raise SystemExit(str(error)) from error
