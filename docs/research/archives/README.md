# Probe source archive

[Download the source snapshot](probes-before-mvp-consolidation.zip) to recover the experiments retired during MVP probe consolidation on 2026-10-17. The ZIP preserves 76 files from the working tree, including untracked source, plus `SNAPSHOT.json` with a SHA-256 digest for each file. Every archived file was compared byte-for-byte with its source before removal. Generated builds, caches, and routine run output are excluded.

Archive SHA-256: `192cfe9b21d3fc7cd771ad88ee3d85f97b8f9f9b5018683d406e3ee2ea1fd8f7`.

## Contents

| Path inside the ZIP | Preserved experiment or context |
| :-- | :-- |
| `probes/schedule-resolution/` | Algorithms for adjusting solar reference times and placing edited points while preserving order across dates; Windows clock code, solar integration, and host-setting exercise. |
| `probes/solar-calculation/` | Rust comparison, all 36 saved United States Naval Observatory (USNO) fixtures, solar-events source, Cargo manifest and lockfile. |
| `probes/windows-reactor/` | Published 0.100.0 comparison and nested pinned development probe, including location code, icon, build helper, lockfiles, and scripts. |
| `probes/nightlight-runtime/` | Native Night Light source and manual scripts as recorded before consolidation. |
| `docs/research/`, `docs/specs/`, `AGENTS.md` | Matching evidence, draft requirements, and repository instructions. |
| `probes/justfile`, `probes/pyproject.toml`, `probes/.gitignore`, `probes/README.md` | Shared Python tooling and probe inventory. |

## Recovery

Extract into a new directory outside the active repository; the command below creates a sibling directory. From the repository root in MSYS2 Bash:

```bash
uv run --no-project --python 3.15 python -B -m zipfile -e docs/research/archives/probes-before-mvp-consolidation.zip ../circadia-probe-snapshot
```

Read the extracted probe's README before running its commands. Resolve its relative paths from the extracted tree. The archive records the experiment at that time; its spec is a draft snapshot, and its dependency results do not establish compatibility with later versions.

The archived GUI scripts open windows, and pointer scripts move the cursor. Location requests can show consent UI. Night Light scripts change color temperature; the host-setting exercise changes the time zone temporarily. Run such commands only after the user confirms the desktop is available. Extraction alone runs none of them.

The active [probe index](../../../probes/README.md) lists the retained checks. Current product behavior remains defined by the [active spec](../../specs/SPEC.md).
