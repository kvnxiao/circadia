# Night Light runtime probe

This isolated Rust probe measures public temperature-operation latency and checks decoded registry readback. It does not implement Circadia's controller. The [research report](../../docs/research/nightlight-runtime.md) records the results and evidence limits.

Verified on 2026-10-16 with Rust 1.99.0, `win-nightlight-lib` 0.4.0, and Windows 11 x64 build 26300.9711, using Cargo's default debug profile. The manifest and lockfile record dependencies. The live run passed 15 direct operations, one installed `wnl temp` command, and restoration. The native-control script also passed: Strength 72 to 0 read back as 6500 K, then the original 2700 K was restored. The installed CLI must be on PATH; its version was not recorded in that run.

The native preview flag temporarily requests application of the stored temperature. A completed refresh clears it; the baseline guard rejects an active preview.

## Commands and side effects

From this directory in MSYS2 Bash:

```bash
just --list
just check
just check-msrv
just inspect
just exercise
```

`check` runs formatting, lint, compilation/test harness, documentation, dependency audit, and unused-dependency checks. There are no unit tests; this probe's assertions require live registry operations. `inspect` only reads settings and state. Builds and dependency checks access Cargo caches and may use the network.

`exercise` changes the current user's Night Light temperature, on/off state, and schedule. Run it as the interactive desktop user with other temperature controllers stopped. It captures valid temperature and state, refuses an active preview, and restores the captured user-visible fields after success or a returned error. It does not restore timestamps. Forced termination or a blocked operation can prevent cleanup. Do not run this command as part of default application checks.

The native-control script opens Windows Night Light settings, changes its strength slider, checks a changed decoded temperature, and restores the baseline temperature through the library's unconditional refresh operation. It requires Night Light on, schedule off, and preview off. Run it after `exercise` in an interactive desktop:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass -File verify-native-controls.ps1
```

Both native scripts close the Settings page they open, including after failures. The control script attempts window cleanup and temperature restoration independently. It changes Night Light temporarily. The probes verified registry data without measuring display output and do not require a colorimeter. Circadia's visible benchmark separately requires inspection of warming and cooling under the [spec](../../docs/specs/application.md#req-preview-benchmark--measure-and-visibly-exercise-temperature-updates). See the [probe index](../README.md) for other verification work.

## Check the Windows slider after an external write

An open Windows Settings page can retain a stale Strength slider after a successful external temperature write. The recorded check restored 4910 K while the slider stayed at 0; reopening the page displayed 30 with registry readback still 4910 K.

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass -File verify-settings-refresh.ps1 -Temperature 4910
```

This script opens the page, applies the explicitly supplied temperature, compares its slider before and after reopening, and closes the page in `finally`. It leaves the requested temperature in place; it does not restore the prior temperature. `native-settings.ps1` supplies shared window discovery and verified closure for both scripts. The Rust `--restore-temperature K` command uses the public `Always` refresh operation and checks temperature and preview readback; the control script uses it for cleanup.
