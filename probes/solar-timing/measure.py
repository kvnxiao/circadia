import json
import math
import os
import platform
import statistics
import subprocess
import winreg
from datetime import UTC, date, datetime, time, timedelta, timezone
from pathlib import Path
from time import perf_counter
from typing import TypedDict


class Measurement(TypedDict):
    first_micros: float
    samples_micros: list[float]
    rises: int
    sets: int


def nonnegative_integer(value: object) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError("Expected a nonnegative integer")
    return value


def duration_micros(value: object) -> float:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise ValueError("Expected a numeric duration")
    result = float(value)
    if not math.isfinite(result) or result < 0:
        raise ValueError("Expected a finite nonnegative duration")
    return result


def decode_measurement(output: str, sample_count: int) -> Measurement:
    decoded: object = json.loads(output)
    if not isinstance(decoded, list) or len(decoded) != 1:
        raise ValueError("Expected exactly one measurement")
    record: object = decoded[0]
    if not isinstance(record, dict):
        raise ValueError("Expected a measurement object")
    samples: object = record.get("samples_micros")
    if not isinstance(samples, list) or len(samples) != sample_count:
        raise ValueError(f"Expected {sample_count} timing samples")
    return Measurement(
        first_micros=duration_micros(record.get("first_micros")),
        samples_micros=[duration_micros(value) for value in samples],
        rises=nonnegative_integer(record.get("rises")),
        sets=nonnegative_integer(record.get("sets")),
    )


def main() -> None:
    root = Path(__file__).resolve().parent
    binary = root / "target/debug/solar-timing.exe"
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
        completed = subprocess.run(
            [str(binary), str(repetitions)],
            input=json.dumps([request]),
            capture_output=True,
            text=True,
            check=True,
        )
        process_ms = (perf_counter() - started) * 1000
        measurement = decode_measurement(completed.stdout, repetitions)
        actual = measurement["rises"], measurement["sets"]
        if actual != expected:
            raise ValueError(f"{name}: expected counts {expected}, got {actual}")
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
        print(json.dumps(summary), flush=True)
    startup_samples = []
    for _ in range(30):
        started = perf_counter()
        completed = subprocess.run(
            [str(binary), "1"], input="[]", capture_output=True, text=True, check=True
        )
        startup_samples.append((perf_counter() - started) * 1000)
        if json.loads(completed.stdout) != []:
            raise ValueError("empty input returned calculations")
    with winreg.OpenKey(
        winreg.HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"
    ) as key:
        cpu_value: object = winreg.QueryValueEx(key, "ProcessorNameString")[0]
        if not isinstance(cpu_value, str):
            raise ValueError("ProcessorNameString must be text")
        cpu = cpu_value.strip()
    with winreg.OpenKey(
        winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"
    ) as key:
        revision_value: object = winreg.QueryValueEx(key, "UBR")[0]
        os_revision = nonnegative_integer(revision_value)
    environment = {
        "measured_at_utc": datetime.now(UTC).isoformat(),
        "cpu": cpu,
        "logical_cpus": os.cpu_count(),
        "platform": platform.platform(),
        "os_revision": os_revision,
        "python": platform.python_version(),
        "rustc": subprocess.check_output(
            ["rustc", "+stable", "--version"], text=True
        ).strip(),
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
    print(
        json.dumps({"environment": environment, "empty_process": startup}), flush=True
    )


if __name__ == "__main__":
    main()
