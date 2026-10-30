"""Measure fixed solar cases with a locally built calculation executable."""

import json
import math
import os
import platform
import shutil
import statistics
import subprocess
import winreg
from datetime import UTC, date, datetime, time, timedelta, timezone
from pathlib import Path
from time import perf_counter
from typing import TypedDict


class Measurement(TypedDict):
    """Store the validated JSON measurement emitted by the timing binary."""

    first_micros: float
    samples_micros: list[float]
    rises: int
    sets: int


def _nonnegative_integer(value: object) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        message = "expected a nonnegative integer"
        raise ValueError(message)
    return value


def _duration_micros(value: object) -> float:
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        result = float(value)
        if math.isfinite(result) and result >= 0:
            return result
    message = "expected a finite nonnegative duration"
    raise ValueError(message)


def decode_measurement(output: str, sample_count: int) -> Measurement:
    """Validate one JSON measurement with the requested sample count."""
    decoded: object = json.loads(output)
    match decoded:
        case [dict() as record]:
            pass
        case _:
            message = "expected exactly one measurement object"
            raise ValueError(message)
    samples: object = record.get("samples_micros")
    if not isinstance(samples, list) or len(samples) != sample_count:
        message = f"Expected {sample_count} timing samples"
        raise ValueError(message)
    return Measurement(
        first_micros=_duration_micros(record.get("first_micros")),
        samples_micros=[_duration_micros(value) for value in samples],
        rises=_nonnegative_integer(record.get("rises")),
        sets=_nonnegative_integer(record.get("sets")),
    )


def main() -> None:
    """Measure fixed cases and write their inputs, timings, and environment."""
    root = Path(__file__).resolve().parent
    binary = root / "target/debug/solar-timing.exe"
    rustc = shutil.which("rustc")
    if rustc is None:
        message = "rustc is unavailable"
        raise FileNotFoundError(message)
    repetitions = 1000
    cases = [
        ("new-york-ordinary", "2026-10-17", 40.7128, -74.006, -4, (1, 1)),
        ("tromso-polar-day", "2026-06-21", 69.6492, 18.9553, 2, (0, 0)),
        ("tromso-polar-night", "2026-12-21", 69.6492, 18.9553, 1, (0, 0)),
        ("tromso-double-sunset", "2026-07-27", 69.6492, 18.9553, 2, (1, 2)),
    ]
    results = []
    inputs = []
    for name, date_text, latitude, longitude, offset, expected in cases:
        day = date.fromisoformat(date_text)
        start = datetime.combine(day, time(), timezone(timedelta(hours=offset)))
        end = start + timedelta(days=1)
        request = {
            "year": day.year,
            "month": day.month,
            "latitude": latitude,
            "longitude": longitude,
            "start": start.timestamp(),
            "end": end.timestamp(),
        }
        inputs.append(
            {
                "name": name,
                "date": date_text,
                "offset_hours": offset,
                "request": request,
                "expected_counts": expected,
            }
        )
        started = perf_counter()
        completed = subprocess.run(  # noqa: S603 -- binary path and arguments come from fixed probe cases
            [str(binary), str(repetitions)],
            input=json.dumps([request]),
            capture_output=True,
            text=True,
            check=True,
            timeout=120,
            cwd=root,
        )
        process_ms = (perf_counter() - started) * 1000
        measurement = decode_measurement(completed.stdout, repetitions)
        actual = measurement["rises"], measurement["sets"]
        if actual != expected:
            message = f"{name}: expected counts {expected}, got {actual}"
            raise ValueError(message)
        values = measurement["samples_micros"]
        summary = {
            "name": name,
            "first_ms": measurement["first_micros"] / 1000,
            "median_ms": statistics.median(values) / 1000,
            "min_ms": min(values) / 1000,
            "max_ms": max(values) / 1000,
            "sample_count": len(values),
            "rises": actual[0],
            "sets": actual[1],
            "process_wall_ms": process_ms,
        }
        results.append({"summary": summary, "measurement": measurement})
        print(json.dumps(summary), flush=True)  # noqa: T201 -- CLI emits each timing summary
    startup_samples = []
    for _ in range(30):
        started = perf_counter()
        completed = subprocess.run(  # noqa: S603 -- binary path and arguments come from fixed probe cases
            [str(binary), "1"],
            input="[]",
            capture_output=True,
            text=True,
            check=True,
            timeout=30,
            cwd=root,
        )
        startup_samples.append((perf_counter() - started) * 1000)
        if json.loads(completed.stdout) != []:
            message = "empty input returned calculations"
            raise ValueError(message)
    with winreg.OpenKey(
        winreg.HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"
    ) as key:
        cpu_value: object = winreg.QueryValueEx(key, "ProcessorNameString")[0]
        if not isinstance(cpu_value, str):
            message = "processorNameString must be text"
            raise TypeError(message)
        cpu = cpu_value.strip()
    with winreg.OpenKey(
        winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"
    ) as key:
        revision_value: object = winreg.QueryValueEx(key, "UBR")[0]
        os_revision = _nonnegative_integer(revision_value)
    environment = {
        "measured_at_utc": datetime.now(UTC).isoformat(),
        "cpu": cpu,
        "logical_cpus": os.cpu_count(),
        "platform": platform.platform(),
        "os_revision": os_revision,
        "python": platform.python_version(),
        "rustc": subprocess.run(  # noqa: S603 -- rustc is resolved and arguments are fixed
            [rustc, "+stable", "--version"],
            text=True,
            capture_output=True,
            check=True,
            timeout=30,
            cwd=root,
        ).stdout.strip(),
        "profile": "Cargo dev: unoptimized + debuginfo",
    }
    startup = {
        "median_ms": statistics.median(startup_samples),
        "min_ms": min(startup_samples),
        "max_ms": max(startup_samples),
        "samples_ms": startup_samples,
    }
    (root / "timing-inputs.json").write_text(
        json.dumps(inputs, indent=2) + chr(10), encoding="utf-8"
    )
    (root / "timing-results.json").write_text(
        json.dumps(
            {"environment": environment, "cases": results, "empty_process": startup},
            indent=2,
        )
        + chr(10),
        encoding="utf-8",
    )
    print(  # noqa: T201 -- CLI emits the environment and startup timing report
        json.dumps({"environment": environment, "empty_process": startup}), flush=True
    )


if __name__ == "__main__":
    try:
        main()
    except subprocess.TimeoutExpired as error:
        raise SystemExit(str(error)) from error
