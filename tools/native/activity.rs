use crate::automation::Automation;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::wait;
use crate::lifecycle::window;
use circadia::ActivityEvent;
use circadia::ActivityMeasurement;
use circadia::ActivityWait;
use circadia::CommandOutcome;
use circadia::ConfigInput;
use circadia::ControllerCommand;
use circadia::Foreground;
use circadia::GraphMeasurement;
use circadia::InstanceClaim;
use circadia::Recompute;
use circadia::RuntimeTime;
use circadia::SchedulingState;
use circadia::SimulatedNativeFault;
use circadia::WindowsZone;
use jiff::Timestamp;
use jiff::civil::Time;
use std::io;
use std::time::Duration;
use std::time::Instant;

pub(super) fn run() -> io::Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(2).collect();
    let quick = arguments.as_slice() == ["--quick"];
    require(
        arguments.is_empty() || quick,
        "usage: native-lifecycle --measure-activity [--quick]",
    )?;
    crate::environment::report(&std::env::current_exe()?, "Activity trace")?;
    report(
        "Native output is simulated. The idle phase uses real clocks, Windows waits, and foreground events. Later phases freeze application clocks, record selected deadlines, and wake the worker explicitly; they do not test Windows timeout delivery. Windows clocks and focus are not changed.",
    );
    let activity = ActivityMeasurement::start()?;
    let capture = activity.clone();
    let input = ConfigInput {
        start_minimized: true,
        manage_night_light: true,
        daytime_kelvin: 3400,
        middle_kelvin: 3400,
        sleep_kelvin: 3400,
        ordinary_fade_ms: 100,
        ..ConfigInput::default()
    };
    let result = crate::graph::desktop_check(input, &[], move |native, _directory| {
        let result = exercise(&capture, native, quick);
        for (index, call) in native.native_calls()?.iter().enumerate() {
            report(&format!(
                "SIMULATED-NATIVE {index} {:?} temperature={:?} failure={:?} resulting={:?}",
                call.operation, call.temperature, call.failure, call.snapshot
            ));
        }
        result
    });
    for (index, record) in activity.records()?.iter().enumerate() {
        report(&format!(
            "ACTIVITY {index} {:.6} {:?}",
            record.elapsed.as_secs_f64(),
            record.event
        ));
    }
    result
}

fn exercise(
    activity: &ActivityMeasurement,
    native: &GraphMeasurement,
    quick: bool,
) -> io::Result<()> {
    wait("constant-output controller ready", || {
        Ok(activity.controller()?.snapshot()?.filter(|snapshot| {
            snapshot.scheduling == SchedulingState::Available
                && snapshot.native_errors.is_empty()
                && snapshot
                    .accepted
                    .as_ref()
                    .is_some_and(|layer| layer.last_scheduled() == Some(3400))
        }))
    })?;
    quiet_wait(activity)?;
    require(
        window(std::process::id(), "Circadia", true)?.is_none(),
        "idle settings must be closed",
    )?;
    idle(activity, native, quick)?;
    activity.set_foreground(Foreground::Unknown)?;
    let mut zone = WindowsZone::current().map_err(io::Error::other)?;
    let current = latest_wait(activity)?.1.now;
    let date = zone
        .local(current.output.wall)
        .map_err(io::Error::other)?
        .date()
        .tomorrow()
        .map_err(io::Error::other)?;
    let start = zone
        .boundary(date.to_datetime(Time::new(20, 0, 0, 0).map_err(io::Error::other)?))
        .map_err(io::Error::other)?;
    let mut time = shifted(
        current,
        Duration::try_from(start.duration_since(current.output.wall)).map_err(io::Error::other)?,
    )?;
    activity.set_time(time)?;
    command(activity, ControllerCommand::Recompute(Recompute::Clock))?;
    let mut input = ConfigInput {
        start_minimized: true,
        manage_night_light: true,
        ordinary_fade_ms: 100,
        focus_delay_ms: 0,
        wake: "07:00".into(),
        bedtime: "21:00".into(),
        daytime_kelvin: 6500,
        middle_kelvin: 3400,
        sleep_kelvin: 2400,
        scheduled_transition_minutes: 60,
        ..ConfigInput::default()
    };
    configure(activity, &input)?;
    time = advance(activity, shifted(time, Duration::from_millis(100))?)?.now;
    wait_temperature(native, 6500)?;
    transition(activity, native, start, &mut time)?;
    let late = zone
        .boundary(date.to_datetime(Time::new(23, 30, 0, 0).map_err(io::Error::other)?))
        .map_err(io::Error::other)?;
    time = shifted(
        time,
        Duration::try_from(late.duration_since(time.output.wall)).map_err(io::Error::other)?,
    )?;
    advance(activity, time)?;
    input.bedtime = "00:30".into();
    input.suppression = vec![circadia::AppEntry::new(
        "activity-game.exe",
        "activity-game.exe",
    )];
    configure(activity, &input)?;
    time = advance(activity, shifted(time, Duration::from_millis(100))?)?.now;
    wait_temperature(native, 6500)?;
    automatic(activity, native, &mut time)?;
    activity.mark("controlled-manual-suppression-start");
    manual(activity, native, &mut time)?;
    activity.mark("controlled-manual-suppression-end");
    recovery(activity, native, &mut time)?;
    report(
        "PASS: real-time idle observations and controlled-clock transition, suppression, date, and manual-expiry traces. Controlled cases do not establish Windows clock or game-event delivery.",
    );
    Ok(())
}

fn idle(activity: &ActivityMeasurement, native: &GraphMeasurement, quick: bool) -> io::Result<()> {
    let seconds = if quick { 2 } else { 600 };
    let before = activity.records()?.len();
    activity.wake()?;
    wait("idle observation barrier", || {
        Ok((latest_wait(activity)?.0 >= before).then_some(()))
    })?;
    activity.mark("real-idle-start");
    let start = activity.records()?.len();
    let calls = native.native_calls()?.len();
    let began = Instant::now();
    for elapsed in 1..=seconds {
        std::thread::sleep(Duration::from_secs(1));
        if elapsed % 60 == 0 {
            report(&format!("Idle observation: {elapsed}/{seconds} seconds"));
        }
    }
    let records = activity.records()?;
    let slice = records
        .get(start..)
        .ok_or_else(|| io::Error::other("activity capture shrank"))?;
    require(
        native.native_calls()?.len() == calls,
        "constant idle output performed native work",
    )?;
    require(
        !slice.iter().any(|record| {
            matches!(
                record.event,
                ActivityEvent::SettingsTick
                    | ActivityEvent::SettingsView
                    | ActivityEvent::NativeDue { .. }
            )
        }),
        "closed constant-output settings performed UI or native work",
    )?;
    let events = slice
        .iter()
        .filter(|record| matches!(record.event, ActivityEvent::ForegroundEvent))
        .count();
    let queries = slice
        .iter()
        .filter(|record| matches!(record.event, ActivityEvent::ForegroundQuery))
        .count();
    let dates = slice
        .iter()
        .filter(|record| matches!(record.event, ActivityEvent::DateRecompute))
        .count();
    require(
        queries <= events + dates,
        "foreground queries exceeded input and date events",
    )?;
    let deadline_wakes = slice
        .iter()
        .filter(|record| {
            matches!(
                record.event,
                ActivityEvent::WorkerWake(circadia::WakeReason::Deadline)
            )
        })
        .count();
    require(
        deadline_wakes <= dates,
        "idle worker woke periodically without a required date boundary",
    )?;
    if events == 0 && dates == 0 {
        require(
            !slice.iter().any(|record| {
                matches!(
                    record.event,
                    ActivityEvent::CurveEvaluation
                        | ActivityEvent::Calculation
                        | ActivityEvent::WorkerWake(_)
                )
            }),
            "stable constant output performed recurring worker work",
        )?;
    }
    let stable = events == 0 && dates == 0 && !quick;
    report(&format!(
        "PASS: idle {:.3}s; foreground events {events}; queries {queries}; date recomputations {dates}; native calls 0; settings ticks/views 0; strict stable ten-minute interval: {stable}",
        began.elapsed().as_secs_f64()
    ));
    if !stable {
        report(
            "LIMIT: this observation does not establish a full ten-minute stable-focus interval; event counts and raw trace remain available.",
        );
    }
    activity.mark("real-idle-end");
    Ok(())
}

fn transition(
    activity: &ActivityMeasurement,
    native: &GraphMeasurement,
    start: Timestamp,
    time: &mut RuntimeTime,
) -> io::Result<()> {
    activity.mark("controlled-one-hour-transition-start");
    let before = native.native_calls()?.len();
    for step in 1..=410_u16 {
        let expected = 6500 - step * 10;
        let waiting = latest_wait(activity)?.1;
        let due = waiting
            .output
            .ok_or_else(|| io::Error::other("transition has no output deadline"))?;
        require(
            due > time.output.elapsed,
            "transition repeated an already due output",
        )?;
        let delta = due.saturating_sub(time.output.elapsed);
        let expected_at = start
            .checked_add(Duration::from_nanos(
                (u64::from(step) * 10 - 5) * 3_600_000_000_000 / 4100 + 1,
            ))
            .map_err(io::Error::other)?;
        require(
            time.output
                .wall
                .checked_add(delta)
                .map_err(io::Error::other)?
                == expected_at,
            "transition deadline disagrees with the independent one-hour linear reference",
        )?;
        let middle = shifted(*time, delta / 2)?;
        activity.set_time(middle)?;
        let records = activity.records()?.len();
        std::thread::sleep(Duration::from_millis(5));
        let observed = activity.records()?;
        let between = observed
            .get(records..)
            .ok_or_else(|| io::Error::other("activity capture shrank"))?;
        if !between
            .iter()
            .any(|record| matches!(record.event, ActivityEvent::ForegroundEvent))
        {
            require(
                !between.iter().any(|record| {
                    matches!(
                        record.event,
                        ActivityEvent::CurveEvaluation | ActivityEvent::NativeDue { .. }
                    )
                }),
                "transition evaluated between due instants without an input event",
            )?;
        }
        *time = advance(activity, shifted(*time, delta)?)?.now;
        wait_temperature(native, expected)?;
    }
    let end = start
        .checked_add(Duration::from_secs(3600))
        .map_err(io::Error::other)?;
    *time = advance(
        activity,
        shifted(
            *time,
            Duration::try_from(end.duration_since(time.output.wall)).map_err(io::Error::other)?,
        )?,
    )?
    .now;
    let calls = native.native_calls()?;
    let temperatures: Vec<_> = calls
        .get(before..)
        .ok_or_else(|| io::Error::other("native capture shrank"))?
        .iter()
        .filter_map(|call| call.temperature.map(|(temperature, _)| temperature))
        .collect();
    let expected: Vec<u16> = (240..650).rev().map(|value| value * 10).collect();
    require(
        temperatures == expected,
        "one-hour transition did not issue exactly 410 descending 10 K steps",
    )?;
    report(
        "PASS: controlled one-hour 6500-to-2400 K transition issued exactly 410 distinct 10 K steps at independently verified deadlines; intermediate instants did not generate output.",
    );
    activity.mark("controlled-one-hour-transition-end");
    Ok(())
}

fn automatic(
    activity: &ActivityMeasurement,
    native: &GraphMeasurement,
    time: &mut RuntimeTime,
) -> io::Result<()> {
    activity.mark("controlled-automatic-suppression-start");
    let foreground = Foreground::Application {
        executable: "activity-game.exe".into(),
        path: std::path::PathBuf::from("activity-game.exe"),
        title: None,
    };
    activity.set_foreground(foreground.clone())?;
    command(
        activity,
        ControllerCommand::Foreground {
            foreground,
            previous: None,
        },
    )?;
    *time = advance(activity, shifted(*time, Duration::from_millis(100))?)?.now;
    let stopped = quiet_wait(activity)?;
    require(
        stopped.logical.is_none(),
        "automatic suppression invented a logical timer",
    )?;
    require(
        activity
            .controller()?
            .snapshot()?
            .is_some_and(|snapshot| snapshot.suppression.automatic),
        "automatic suppression did not activate",
    )?;
    let before = native.native_calls()?.len();
    let records = activity.records()?;
    let date = records
        .iter()
        .rev()
        .find_map(|record| match record.event {
            ActivityEvent::DateDeadline(date) => date,
            _ => None,
        })
        .ok_or_else(|| io::Error::other("suppression lost its date deadline"))?;
    let mark = records.len();
    let delta =
        Duration::try_from(date.duration_since(time.output.wall)).map_err(io::Error::other)?;
    *time = advance(activity, shifted(*time, delta)?)?.now;
    wait("one date recalculation during suppression", || {
        let records = activity.records()?;
        let count = records
            .iter()
            .skip(mark)
            .filter(|record| matches!(record.event, ActivityEvent::DateRecompute))
            .count();
        let following_date = records
            .iter()
            .rev()
            .find_map(|record| match record.event {
                ActivityEvent::DateDeadline(date) => Some(date),
                _ => None,
            })
            .flatten();
        Ok((count == 1
            && following_date.is_some_and(|following| following > date)
            && activity
                .controller()?
                .snapshot()?
                .is_some_and(|snapshot| snapshot.scheduling == SchedulingState::Available)
            && latest_wait(activity)?.1.output.is_none())
        .then_some(()))
    })?;
    quiet_wait(activity)?;
    require(
        activity
            .records()?
            .iter()
            .skip(mark)
            .filter(|record| matches!(record.event, ActivityEvent::Calculation))
            .count()
            == 1,
        "suppressed date change did not calculate exactly once",
    )?;
    require(
        native.native_calls()?.len() == before,
        "suppressed curve emitted native work at the date boundary",
    )?;
    report(
        "PASS: automatic suppression removed curve-output deadlines while preserving one date-boundary recomputation and 6500 K.",
    );
    activity.mark("controlled-automatic-suppression-end");
    Ok(())
}

fn manual(
    activity: &ActivityMeasurement,
    native: &GraphMeasurement,
    time: &mut RuntimeTime,
) -> io::Result<()> {
    match InstanceClaim::acquire()? {
        InstanceClaim::Existing(existing) => existing.activate()?,
        InstanceClaim::Primary(_) => {
            return Err(io::Error::other("measurement instance is missing"));
        }
    }
    let automation = Automation::new()?;
    let process = std::process::id();
    let settings = wait("settings for manual suppression", || {
        window(process, "Circadia", true)
    })?;
    crate::automation::wait("manual suppression control", || {
        Ok(automation
            .enabled(&settings, process, "manual-suppression")?
            .filter(|enabled| *enabled))
    })?;
    automation.invoke(&settings, process, "manual-suppression")?;
    wait("manual suppression active", || {
        Ok(activity
            .controller()?
            .snapshot()?
            .filter(|snapshot| snapshot.suppression.manual()))
    })?;
    activity.set_foreground(Foreground::Unknown)?;
    command(
        activity,
        ControllerCommand::Foreground {
            foreground: Foreground::Unknown,
            previous: None,
        },
    )?;
    let deadline = wait("manual suppression expiry selected", || {
        Ok(latest_wait(activity)?.1.logical)
    })?;
    require(
        deadline == time.output.elapsed + Duration::from_secs(3600),
        "manual suppression did not schedule one hour",
    )?;
    open_settings_idle(activity, &automation, &settings)?;
    automation.close(&settings, process)?;
    wait("settings closed with manual suppression", || {
        Ok(window(process, "Circadia", true)?.is_none().then_some(()))
    })?;
    quiet_wait(activity)?;
    let calls = native.native_calls()?.len();
    closed_settings_idle(activity)?;
    require(
        native.native_calls()?.len() == calls,
        "hidden suppression caused native work",
    )?;
    let before = deadline
        .checked_sub(Duration::from_nanos(1))
        .ok_or_else(|| io::Error::other("invalid manual deadline"))?;
    for (elapsed, expected) in [
        (before.saturating_sub(time.output.elapsed), true),
        (Duration::from_nanos(1), false),
    ] {
        *time = advance(activity, shifted(*time, elapsed)?)?.now;
        require(
            activity
                .controller()?
                .snapshot()?
                .is_some_and(|snapshot| snapshot.suppression.manual() == expected),
            &format!(
                "manual suppression at {:?} must be {expected}",
                time.output.elapsed
            ),
        )?;
    }
    *time = advance(activity, shifted(*time, Duration::from_millis(100))?)?.now;
    wait_temperature(native, 2400)?;
    report(
        "PASS: open Settings updated its graph indicator once per whole minute; closing Settings stopped its recurring UI work; controlled manual expiry remained active until exactly one hour and restored 2400 K.",
    );
    Ok(())
}

fn settings_updates(activity: &ActivityMeasurement, mark: usize) -> io::Result<(usize, usize)> {
    let records = activity.records()?;
    let slice = records
        .get(mark..)
        .ok_or_else(|| io::Error::other("activity capture shrank"))?;
    let count = |event: fn(&ActivityEvent) -> bool| {
        slice.iter().filter(|record| event(&record.event)).count()
    };
    Ok((
        count(|event| matches!(event, ActivityEvent::SettingsTick)),
        count(|event| matches!(event, ActivityEvent::SettingsView)),
    ))
}

fn until_next_minute(margin: Duration) -> io::Result<Duration> {
    let into = Duration::from_millis(
        u64::try_from(Timestamp::now().as_millisecond().rem_euclid(60_000))
            .map_err(io::Error::other)?,
    );
    Ok(Duration::from_secs(60).saturating_sub(into) + margin)
}

// The graph's time indicator is the only timer on an idle Schedule page and
// fires at each whole minute, so an observation that crosses one boundary
// must see exactly one tick.
fn open_settings_idle(
    activity: &ActivityMeasurement,
    automation: &Automation,
    settings: &winsafe::HWND,
) -> io::Result<()> {
    std::thread::sleep(until_next_minute(Duration::from_secs(2))?);
    let mark = activity.records()?.len();
    let began = Instant::now();
    let observed = until_next_minute(Duration::from_secs(2))?;
    std::thread::sleep(observed);
    let (ticks, views) = settings_updates(activity, mark)?;
    require(
        automation
            .text(settings, std::process::id(), "status-sentence")?
            .is_some(),
        "the open Settings window does not show the Schedule page",
    )?;
    require(
        ticks == 1,
        &format!("open idle Settings ticked {ticks} times across one minute boundary"),
    )?;
    require(
        views <= 2,
        &format!("open idle Settings rebuilt its view {views} times across one minute boundary"),
    )?;
    report(&format!(
        "PASS: open idle Settings over {:.3}s: graph indicator ticks {ticks}; view updates {views}",
        began.elapsed().as_secs_f64()
    ));
    Ok(())
}

fn closed_settings_idle(activity: &ActivityMeasurement) -> io::Result<()> {
    let mark = activity.records()?.len();
    let began = Instant::now();
    std::thread::sleep(until_next_minute(Duration::from_secs(2))?);
    let (ticks, views) = settings_updates(activity, mark)?;
    require(
        ticks == 0 && views == 0,
        &format!("hidden Settings updated {ticks} ticks and {views} views"),
    )?;
    report(&format!(
        "PASS: closed Settings over {:.3}s across a minute boundary: ticks 0; view updates 0",
        began.elapsed().as_secs_f64()
    ));
    Ok(())
}

fn configure(activity: &ActivityMeasurement, input: &ConfigInput) -> io::Result<()> {
    let config = input.clone().validate().map_err(io::Error::other)?;
    command(
        activity,
        ControllerCommand::Configure {
            config: std::sync::Arc::new(config.clone()),
            revision: 1,
        },
    )?;
    wait("configured schedule calculation applied", || {
        Ok(activity.controller()?.snapshot()?.filter(|snapshot| {
            snapshot
                .accepted
                .as_ref()
                .is_some_and(|layer| layer.config() == &config)
                && snapshot.scheduling == SchedulingState::Available
                && snapshot.control_error.is_none()
        }))
    })?;
    Ok(())
}

fn recovery(
    activity: &ActivityMeasurement,
    native: &GraphMeasurement,
    time: &mut RuntimeTime,
) -> io::Result<()> {
    activity.mark("controlled-recovery-start");
    native.fail_native(&[SimulatedNativeFault::SettingsRead])?;
    let input = ConfigInput {
        daytime_kelvin: 2500,
        middle_kelvin: 2500,
        sleep_kelvin: 2500,
        manage_night_light: true,
        ordinary_fade_ms: 100,
        ..ConfigInput::default()
    };
    configure(activity, &input)?;
    let step = wait("first fade step toward the recovery temperature", || {
        Ok(latest_wait(activity)?.1.output)
    })?;
    *time = advance(
        activity,
        shifted(*time, step.saturating_sub(time.output.elapsed))?,
    )?
    .now;
    for delay in [100, 200, 400, 800, 800] {
        let waiting = wait("native recovery deadline selected", || {
            let waiting = latest_wait(activity)?.1;
            Ok(waiting.recovery.map(|_| waiting))
        })?;
        let due = waiting
            .recovery
            .ok_or_else(|| io::Error::other("retry deadline disappeared"))?;
        require(
            due == time.output.elapsed + Duration::from_millis(delay),
            "native backoff differs from the required 100, 200, 400, then 800 ms sequence",
        )?;
        require(
            waiting.output.is_none() && waiting.cleanup.is_none(),
            "blocked recovery retained an independent output or cleanup timer",
        )?;
        let calls = native.native_calls()?.len();
        activity.set_time(shifted(*time, Duration::from_millis(delay / 2))?)?;
        std::thread::sleep(Duration::from_millis(5));
        require(
            native.native_calls()?.len() == calls,
            "native recovery occurred before its deadline",
        )?;
        *time = advance(activity, shifted(*time, Duration::from_millis(delay))?)?.now;
        let calls_after = native.native_calls()?;
        let failures = calls_after
            .iter()
            .skip(calls)
            .filter(|call| call.failure.is_some())
            .count();
        require(
            failures == 1,
            "native recovery did not perform exactly one failed read at its deadline",
        )?;
    }
    native.fail_native(&[])?;
    *time = advance(activity, shifted(*time, Duration::from_millis(800))?)?.now;
    *time = advance(activity, shifted(*time, Duration::from_millis(100))?)?.now;
    wait_temperature(native, 2500)?;
    quiet_wait(activity)?;
    let calls = native.native_calls()?.len();
    std::thread::sleep(Duration::from_secs(1));
    require(
        native.native_calls()?.len() == calls,
        "native recovery continued after success",
    )?;
    report(
        "PASS: controlled recovery used 100, 200, 400, then 800 ms delays with one failed read per deadline, no independent curve-output timer, and no repeated native work after success.",
    );
    activity.mark("controlled-recovery-end");
    Ok(())
}

fn command(activity: &ActivityMeasurement, command: ControllerCommand) -> io::Result<()> {
    let receipt = activity.controller()?.submit(command)?;
    let result = receipt
        .completion
        .recv_timeout(Duration::from_secs(20))
        .map_err(io::Error::other)?;
    require(
        matches!(result, CommandOutcome::Applied),
        &format!("activity command was not applied: {result:?}"),
    )
}

fn latest_wait(activity: &ActivityMeasurement) -> io::Result<(usize, ActivityWait)> {
    activity
        .records()?
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, record)| match record.event {
            ActivityEvent::Wait(waiting) => Some((index, waiting)),
            _ => None,
        })
        .ok_or_else(|| io::Error::other("controller has not selected a wait"))
}

fn quiet_wait(activity: &ActivityMeasurement) -> io::Result<ActivityWait> {
    wait("no native output, cleanup, or recovery deadline", || {
        let (_, waiting) = latest_wait(activity)?;
        Ok(
            (waiting.output.is_none() && waiting.cleanup.is_none() && waiting.recovery.is_none())
                .then_some(waiting),
        )
    })
}

fn advance(activity: &ActivityMeasurement, time: RuntimeTime) -> io::Result<ActivityWait> {
    activity.set_time(time)?;
    let before = activity.records()?.len();
    activity.wake()?;
    wait("controller waiting after controlled clock advance", || {
        let records = activity.records()?;
        let advanced = records
            .get(before..)
            .ok_or_else(|| io::Error::other("activity capture shrank"))?;
        Ok(completed_wait(advanced, time))
    })
}

fn completed_wait(records: &[circadia::ActivityRecord], time: RuntimeTime) -> Option<ActivityWait> {
    let wake = records
        .iter()
        .position(|record| matches!(record.event, ActivityEvent::WorkerWake(_)))?;
    records
        .iter()
        .skip(wake + 1)
        .find_map(|record| match record.event {
            ActivityEvent::Wait(waiting)
                if waiting.now.output.elapsed == time.output.elapsed
                    && waiting.now.output.wall == time.output.wall
                    && waiting.now.uptime == time.uptime =>
            {
                Some(waiting)
            }
            _ => None,
        })
}

fn shifted(mut time: RuntimeTime, elapsed: Duration) -> io::Result<RuntimeTime> {
    time.output.wall = time
        .output
        .wall
        .checked_add(elapsed)
        .map_err(io::Error::other)?;
    time.output.elapsed = time
        .output
        .elapsed
        .checked_add(elapsed)
        .ok_or_else(|| io::Error::other("output clock overflow"))?;
    time.uptime = time
        .uptime
        .checked_add(elapsed)
        .ok_or_else(|| io::Error::other("uptime clock overflow"))?;
    Ok(time)
}

fn wait_temperature(native: &GraphMeasurement, expected: u16) -> io::Result<()> {
    let expected = i16::try_from(expected).map_err(io::Error::other)?;
    wait(&format!("simulated temperature {expected}"), || {
        Ok(native
            .native_calls()?
            .last()
            .filter(|call| {
                call.snapshot.settings.temperature == Some(expected)
                    && !call.snapshot.settings.previewing
            })
            .map(|_| ()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use circadia::ActivityRecord;
    use circadia::ControlTime;
    use circadia::WakeReason;

    #[test]
    fn clock_advance_wait_requires_a_preceding_worker_wake() {
        let time = RuntimeTime {
            output: ControlTime {
                wall: Timestamp::UNIX_EPOCH,
                elapsed: Duration::from_secs(10),
            },
            uptime: Duration::from_secs(20),
        };
        let waiting = ActivityRecord {
            elapsed: Duration::ZERO,
            event: ActivityEvent::Wait(ActivityWait {
                now: time,
                output: None,
                cleanup: None,
                recovery: None,
                logical: None,
            }),
        };
        let wake = ActivityRecord {
            elapsed: Duration::ZERO,
            event: ActivityEvent::WorkerWake(WakeReason::Command),
        };
        assert!(completed_wait(&[waiting], time).is_none());
        assert!(completed_wait(&[waiting, wake], time).is_none());
        assert!(completed_wait(&[waiting, wake, waiting], time).is_some());
        let later = shifted(time, Duration::from_secs(1)).expect("representable later time");
        assert!(completed_wait(&[wake, waiting], later).is_none());
    }
}
