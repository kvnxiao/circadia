use crate::OutputDeadlines;
use std::fmt::Write;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

static ACTIVE: OnceLock<OutputMeasurement> = OnceLock::new();
const RECORD_LIMIT: usize = 100_000;

#[derive(Clone, Copy, Debug)]
struct Attempt {
    due: Duration,
    eligible: Duration,
    ready: Duration,
    started: Duration,
    discrete: bool,
    hertz: u16,
    label: usize,
    temperatures: usize,
}

#[derive(Debug)]
struct Call {
    operation: &'static str,
    kelvin: Option<u16>,
    started: Duration,
    completed: Duration,
    eligible: Option<Duration>,
    ready: Option<Duration>,
    attempt: Option<Attempt>,
    success: bool,
}

struct Records {
    labels: Vec<String>,
    calls: Vec<Call>,
    endpoints: Vec<Endpoint>,
    attempt: Option<Attempt>,
    last_attempt: Option<Duration>,
    last_call: Duration,
    error: Option<String>,
}

impl Default for Records {
    fn default() -> Self {
        Self {
            labels: vec!["startup".into()],
            calls: Vec::new(),
            endpoints: Vec::new(),
            attempt: None,
            last_attempt: None,
            last_call: Duration::ZERO,
            error: None,
        }
    }
}

struct Endpoint {
    label: usize,
    kind: &'static str,
    due: Duration,
    kelvin: u16,
}

#[derive(Default)]
struct Shared {
    origin: OnceLock<Instant>,
    records: Mutex<Records>,
    lifecycle: Mutex<Lifecycle>,
    ready: Condvar,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Lifecycle {
    #[default]
    Starting,
    Ready,
    Stopped,
}

/// Record output eligibility and live native calls in an instrumented desktop
/// run.
#[derive(Clone, Default)]
pub struct OutputMeasurement(Arc<Shared>);

impl OutputMeasurement {
    /// Run the complete desktop application with live Night light and isolated
    /// data.
    ///
    /// # Errors
    /// Report repeated initialization or application failure.
    pub fn run(&self, directory: PathBuf) -> io::Result<()> {
        let result = ACTIVE
            .set(self.clone())
            .map_err(|_error| io::Error::other("output measurement already started"))
            .and_then(|()| crate::run_desktop(Some(directory), false, None));
        self.0
            .lifecycle
            .lock()
            .map(|mut state| *state = Lifecycle::Stopped)
            .map_err(|_error| io::Error::other("output lifecycle lock failed"))?;
        self.0.ready.notify_all();
        result
    }

    /// Wait until the application can receive a graceful exit request.
    ///
    /// # Errors
    /// Report startup failure or unavailable synchronization.
    pub fn wait_ready(&self) -> io::Result<()> {
        let state = self
            .0
            .lifecycle
            .lock()
            .map_err(|_error| io::Error::other("output lifecycle lock failed"))?;
        let state = self
            .0
            .ready
            .wait_while(state, |state| *state == Lifecycle::Starting)
            .map_err(|_error| io::Error::other("output lifecycle lock failed"))?;
        if *state == Lifecycle::Ready {
            Ok(())
        } else {
            Err(io::Error::other(
                "application stopped before measurement input started",
            ))
        }
    }

    /// Label subsequent output attempts with the current scenario.
    ///
    /// # Errors
    /// Report unavailable synchronization or too many scenario labels.
    pub fn mark(&self, label: &str) -> io::Result<()> {
        let mut records = self
            .0
            .records
            .lock()
            .map_err(|_error| io::Error::other("output trace lock failed"))?;
        if records.labels.len() >= RECORD_LIMIT {
            return Err(io::Error::other("output scenario record limit exceeded"));
        }
        records.labels.push(label.into());
        Ok(())
    }

    /// Return raw calls, scenario summaries, and whether every measured
    /// scenario passed.
    ///
    /// # Errors
    /// Report unavailable synchronization or formatting failure.
    pub fn report(&self) -> io::Result<(String, bool)> {
        let records = self
            .0
            .records
            .lock()
            .map_err(|_error| io::Error::other("output trace lock failed"))?;
        records.report().map_err(io::Error::other)
    }

    /// Record an independently calculated scheduled endpoint.
    ///
    /// # Errors
    /// Report an unavailable clock or trace synchronization.
    pub fn scheduled_endpoint(&self, at: Instant, kelvin: u16) -> io::Result<()> {
        let origin = self
            .0
            .origin
            .get()
            .ok_or_else(|| io::Error::other("controller clock is unavailable"))?;
        let mut records = self
            .0
            .records
            .lock()
            .map_err(|_error| io::Error::other("output trace lock failed"))?;
        records.endpoint("scheduled", at.duration_since(*origin), kelvin);
        Ok(())
    }
}

impl Records {
    fn begin_due(
        &mut self,
        started: Duration,
        deadlines: OutputDeadlines,
        correction: Option<Duration>,
        hertz: u16,
    ) {
        let candidates = deadlines
            .change
            .map(|due| (due, deadlines.discrete))
            .into_iter()
            .chain(correction.map(|due| (due, false)));
        let mut first = None;
        for (due, discrete) in candidates {
            let eligible =
                match eligibility(due, self.last_attempt, self.last_call, hertz, discrete) {
                    Ok(eligible) => eligible,
                    Err(error) => {
                        self.error = Some(error.to_string());
                        return;
                    }
                };
            if first.is_none_or(|(earliest, _, _)| eligible < earliest) {
                first = Some((eligible, due, discrete));
            }
        }
        if let Some((_, due, discrete)) = first {
            self.begin(started, due, hertz, discrete);
        } else {
            self.error = Some("temperature attempt has no original due timestamp".into());
        }
    }

    fn begin(&mut self, started: Duration, due: Duration, hertz: u16, discrete: bool) {
        let ready = eligibility(
            Duration::ZERO,
            self.last_attempt,
            self.last_call,
            hertz,
            discrete,
        );
        match ready {
            Ok(ready) => {
                let eligible = due.max(ready);
                if started < eligible {
                    self.error = Some("output attempt started before eligibility".into());
                }
                self.attempt = Some(Attempt {
                    due,
                    eligible,
                    ready,
                    started,
                    discrete,
                    hertz,
                    label: self.labels.len().saturating_sub(1),
                    temperatures: 0,
                });
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        self.last_attempt = Some(started);
    }

    fn endpoint(&mut self, kind: &'static str, due: Duration, kelvin: u16) {
        if self.endpoints.len() >= RECORD_LIMIT {
            self.error = Some("output endpoint record limit exceeded".into());
            return;
        }
        self.endpoints.push(Endpoint {
            label: self.labels.len().saturating_sub(1),
            kind,
            due,
            kelvin,
        });
    }

    fn call(
        &mut self,
        operation: &'static str,
        kelvin: Option<u16>,
        started: Duration,
        completed: Duration,
        success: bool,
    ) {
        if self.calls.len() >= RECORD_LIMIT {
            self.error = Some("native call record limit exceeded".into());
            return;
        }
        if completed < started || started < self.last_call {
            self.error = Some("native call timestamps overlap or run backward".into());
        }
        let (eligible, ready) = if let (Some(_), Some(attempt)) = (kelvin, self.attempt.as_mut()) {
            let (eligible, ready) = if attempt.temperatures == 0 {
                (attempt.eligible, attempt.ready)
            } else {
                (
                    attempt.eligible.max(self.last_call),
                    attempt.ready.max(self.last_call),
                )
            };
            attempt.temperatures += 1;
            (Some(eligible), Some(ready))
        } else {
            (None, None)
        };
        if kelvin.is_some() && eligible.is_none() {
            self.error = Some("temperature call has no attempt eligibility".into());
        }
        self.calls.push(Call {
            operation,
            kelvin,
            started,
            completed,
            eligible,
            ready,
            attempt: self.attempt,
            success,
        });
        self.last_call = completed;
        if !success {
            self.error = Some("a native call failed during output measurement".into());
        }
    }

    fn report(&self) -> Result<(String, bool), std::fmt::Error> {
        let mut text = String::from(
            "CALL columns: scenario,operation,kelvin,due_ms,eligible_ms,attempt_start_ms,call_start_ms,call_end_ms,discrete,rate_hz,success\n",
        );
        for call in &self.calls {
            let attempt = call.attempt;
            let label = attempt
                .and_then(|attempt| self.labels.get(attempt.label))
                .map_or("outside-attempt", String::as_str);
            writeln!(
                text,
                "CALL,{label},{},{},{},{},{},{:.6},{:.6},{},{},{}",
                call.operation,
                call.kelvin
                    .map_or_else(String::new, |value| value.to_string()),
                attempt.map_or_else(String::new, |value| milliseconds(value.due).to_string()),
                call.eligible
                    .map_or_else(String::new, |value| milliseconds(value).to_string()),
                attempt.map_or_else(String::new, |value| milliseconds(value.started).to_string()),
                milliseconds(call.started),
                milliseconds(call.completed),
                attempt.is_some_and(|value| value.discrete),
                attempt.map_or(0, |value| value.hertz),
                call.success
            )?;
        }
        let mut passed = self.error.is_none();
        for (index, label) in self.labels.iter().enumerate() {
            let calls = self
                .calls
                .iter()
                .filter(|call| {
                    call.kelvin.is_some()
                        && call.attempt.is_some_and(|attempt| attempt.label == index)
                })
                .collect::<Vec<_>>();
            if calls.is_empty() {
                passed = false;
                writeln!(text, "FAIL,{label},no temperature calls")?;
                continue;
            }
            let mut lateness = calls
                .iter()
                .filter_map(|call| call.eligible.and_then(|due| call.started.checked_sub(due)))
                .collect::<Vec<_>>();
            lateness.sort_unstable();
            let within = lateness
                .iter()
                .filter(|delay| **delay <= Duration::from_millis(20))
                .count();
            let complete = lateness.len() == calls.len();
            let timely = within * 100 >= calls.len() * 95;
            passed &= complete && timely;
            let rank = (lateness.len() * 95).div_ceil(100).saturating_sub(1);
            let p95 = lateness.get(rank).copied().unwrap_or_default();
            let max = lateness.last().copied().unwrap_or_default();
            let latency = calls
                .iter()
                .map(|call| call.completed.saturating_sub(call.started))
                .max()
                .unwrap_or_default();
            writeln!(
                text,
                "RESULT,{label},calls={},within_20ms={within},p95_start_lateness_ms={:.6},max_start_lateness_ms={:.6},max_call_ms={:.6},pass={}",
                calls.len(),
                milliseconds(p95),
                milliseconds(max),
                milliseconds(latency),
                complete && timely
            )?;
        }
        if let Some(error) = &self.error {
            writeln!(text, "FAIL,{error}")?;
        }
        passed &= self.report_endpoints(&mut text)?;
        Ok((text, passed))
    }

    fn report_endpoints(&self, text: &mut String) -> Result<bool, std::fmt::Error> {
        let mut passed = true;
        for endpoint in &self.endpoints {
            let label = self
                .labels
                .get(endpoint.label)
                .map_or("missing", String::as_str);
            let calls = self
                .calls
                .iter()
                .filter(|call| {
                    call.kelvin.is_some()
                        && call.success
                        && call
                            .attempt
                            .is_some_and(|attempt| attempt.label == endpoint.label)
                })
                .collect::<Vec<_>>();
            let restoration = endpoint.kind == "preview-restoration";
            let preceding = calls
                .iter()
                .rev()
                .find(|call| call.started < endpoint.due)
                .filter(|call| !restoration && call.kelvin == Some(endpoint.kelvin));
            let following = calls.iter().find(|call| {
                call.started >= endpoint.due
                    && call.kelvin == Some(endpoint.kelvin)
                    && (!restoration || call.operation == "apply_color_temperature")
            });
            let final_call = preceding.or(following);
            let already_stored = final_call.is_some_and(|call| call.completed <= endpoint.due);
            let delay = final_call.and_then(|call| endpoint_delay(call, endpoint.due));
            let timely = delay.is_some_and(|delay| delay <= Duration::from_millis(20));
            passed &= timely;
            writeln!(
                text,
                "ENDPOINT,{label},{},due_ms={:.6},kelvin={},already_stored={already_stored},final_start_lateness_ms={},pass={timely}",
                endpoint.kind,
                milliseconds(endpoint.due),
                endpoint.kelvin,
                delay.map_or_else(String::new, |value| milliseconds(value).to_string())
            )?;
        }
        Ok(passed)
    }
}

fn endpoint_delay(call: &Call, due: Duration) -> Option<Duration> {
    let operation_delay = call.started.checked_sub(call.eligible?)?;
    if call.started < due {
        return Some(operation_delay);
    }
    let endpoint_delay = call.started.checked_sub(due.max(call.ready?))?;
    Some(operation_delay.max(endpoint_delay))
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn eligibility(
    due: Duration,
    last_attempt: Option<Duration>,
    last_call: Duration,
    hertz: u16,
    discrete: bool,
) -> io::Result<Duration> {
    if !(1..=240).contains(&hertz) {
        return Err(io::Error::other("measurement rate is outside 1-240 Hz"));
    }
    let rate = if discrete {
        Duration::ZERO
    } else {
        last_attempt
            .map(|last| {
                last.checked_add(Duration::from_nanos(
                    1_000_000_000_u64.div_ceil(u64::from(hertz)),
                ))
                .ok_or_else(|| io::Error::other("measurement rate deadline overflow"))
            })
            .transpose()?
            .unwrap_or_default()
    };
    Ok(due.max(rate).max(last_call))
}

pub(crate) fn bind_origin(origin: Instant) {
    if let Some(active) = ACTIVE.get()
        && active.0.origin.set(origin).is_err()
    {
        fail("output measurement has more than one controller origin");
    }
}

pub(crate) fn desktop_ready() {
    if let Some(active) = ACTIVE.get() {
        if let Ok(mut state) = active.0.lifecycle.lock() {
            *state = Lifecycle::Ready;
        }
        active.0.ready.notify_all();
    }
}

pub(crate) fn elapsed() -> Duration {
    ACTIVE
        .get()
        .and_then(|active| active.0.origin.get())
        .map_or(Duration::ZERO, Instant::elapsed)
}

pub(crate) fn begin(
    started: Duration,
    deadlines: OutputDeadlines,
    correction: Option<Duration>,
    hertz: u16,
) {
    let Some(active) = ACTIVE.get() else { return };
    let Ok(mut records) = active.0.records.lock() else {
        return;
    };
    records.begin_due(started, deadlines, correction, hertz);
}

pub(crate) fn end(attempt: &crate::NativeAttempt, control_failed: bool) {
    if let Some(active) = ACTIVE.get()
        && let Ok(mut records) = active.0.records.lock()
    {
        if attempt.error.is_some()
            || attempt.cleanup_error.is_some()
            || !attempt.observation_errors.is_empty()
            || control_failed
        {
            records.error = Some("control attempt failed during output measurement".into());
        }
        records.attempt = None;
    }
}

pub(crate) fn endpoint(kind: &'static str, due: Duration, kelvin: u16) {
    if let Some(active) = ACTIVE.get()
        && let Ok(mut records) = active.0.records.lock()
    {
        records.endpoint(kind, due, kelvin);
    }
}

pub(crate) fn fade(fade: crate::TemperatureFade) {
    match fade.rounded(fade.end()) {
        Ok(value) => endpoint("fade", fade.end(), value.kelvin()),
        Err(error) => fail(&format!("measuring fade endpoint: {error}")),
    }
}

pub(crate) fn call(operation: &'static str, kelvin: Option<u16>, started: Instant, success: bool) {
    let completed = Instant::now();
    let Some(active) = ACTIVE.get() else { return };
    let Some(origin) = active.0.origin.get() else {
        fail("native call preceded controller clock initialization");
        return;
    };
    if let Ok(mut records) = active.0.records.lock() {
        records.call(
            operation,
            kelvin,
            started.duration_since(*origin),
            completed.duration_since(*origin),
            success,
        );
    }
}

pub(crate) fn fail(error: &str) {
    if let Some(active) = ACTIVE.get()
        && let Ok(mut records) = active.0.records.lock()
    {
        records.error = Some(error.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(5, Some(0), 0, 100, false, 10)]
    #[test_case(5, Some(0), 120, 100, false, 120)]
    #[test_case(5, Some(0), 0, 1, true, 5)]
    #[test_case(200, Some(0), 120, 100, false, 200)]
    fn original_due_rate_and_prior_completion_define_eligibility(
        due: u64,
        previous: Option<u64>,
        completed: u64,
        rate: u16,
        discrete: bool,
        expected: u64,
    ) {
        assert_eq!(
            eligibility(
                Duration::from_millis(due),
                previous.map(Duration::from_millis),
                Duration::from_millis(completed),
                rate,
                discrete
            )
            .expect("valid sample"),
            Duration::from_millis(expected)
        );
    }

    #[test]
    fn slow_wakeup_and_fresh_reads_remain_in_start_lateness() {
        let mut records = Records::default();
        records.begin(
            Duration::from_millis(90),
            Duration::from_millis(10),
            60,
            false,
        );
        records.call(
            "settings",
            None,
            Duration::from_millis(91),
            Duration::from_millis(100),
            true,
        );
        records.call(
            "set",
            Some(3400),
            Duration::from_millis(101),
            Duration::from_millis(103),
            true,
        );
        let call = records.calls.last().expect("temperature record");
        assert_eq!(call.eligible, Some(Duration::from_millis(10)));
        assert_eq!(
            call.started.checked_sub(call.eligible.expect("eligible")),
            Some(Duration::from_millis(91))
        );
        assert!(!records.report().expect("report").1);
    }

    #[test_case(103_000_000, 103_000_000, true; "discrete output precedes correction eligibility")]
    #[test_case(110_000_000, 104_166_667, false; "correction precedes discrete output")]
    fn concurrent_output_and_correction_use_the_first_eligible_opportunity(
        output: u64,
        eligible: u64,
        discrete: bool,
    ) {
        let mut records = Records {
            last_attempt: Some(Duration::from_millis(100)),
            last_call: Duration::from_millis(101),
            ..Records::default()
        };
        records.begin_due(
            Duration::from_nanos(eligible),
            OutputDeadlines {
                change: Some(Duration::from_nanos(output)),
                discrete: true,
                ..OutputDeadlines::default()
            },
            Some(Duration::from_millis(101)),
            240,
        );
        let attempt = records.attempt.expect("correlated attempt");
        assert_eq!(attempt.eligible, Duration::from_nanos(eligible));
        assert_eq!(attempt.discrete, discrete);
        assert!(records.error.is_none(), "{:?}", records.error);
    }

    #[test_case(0)]
    #[test_case(241)]
    fn invalid_output_rates_fail_measurement(rate: u16) {
        assert!(eligibility(Duration::ZERO, None, Duration::ZERO, rate, false).is_err());
    }

    #[test]
    fn early_output_cannot_pass_as_zero_lateness() {
        let mut records = Records::default();
        records.begin(Duration::ZERO, Duration::from_millis(10), 60, false);
        records.call(
            "set",
            Some(3400),
            Duration::ZERO,
            Duration::from_millis(1),
            true,
        );
        assert!(!records.report().expect("report").1);
    }

    #[test]
    fn missing_attempt_correlation_fails_even_with_valid_samples() {
        let mut records = Records::default();
        records.call("set", Some(3400), Duration::ZERO, Duration::ZERO, true);
        records.begin(Duration::from_secs(1), Duration::from_secs(1), 60, false);
        records.call(
            "set",
            Some(3400),
            Duration::from_secs(1),
            Duration::from_secs(1),
            true,
        );
        assert!(!records.report().expect("report").1);
    }

    #[test]
    fn native_call_record_overflow_invalidates_the_run() {
        let mut records = Records::default();
        for _ in 0..=RECORD_LIMIT {
            records.call("read", None, Duration::ZERO, Duration::ZERO, true);
        }
        assert_eq!(records.calls.len(), RECORD_LIMIT);
        assert_eq!(
            records.error.as_deref(),
            Some("native call record limit exceeded")
        );
    }

    #[test_case("preview-restoration", 100, 10, 11, Some((100, 130, 131)), false; "late restoration despite prior matching storage")]
    #[test_case("scheduled", 100, 40, 41, None, false; "late final storage before logical endpoint")]
    #[test_case("fade", 20, 11, 30, None, true; "timely final call spans logical endpoint")]
    fn final_request_timing_is_checked_independently(
        kind: &'static str,
        due: u64,
        start: u64,
        completed: u64,
        restoration: Option<(u64, u64, u64)>,
        expected: bool,
    ) {
        let mut records = Records::default();
        records.begin(
            Duration::from_millis(start),
            Duration::from_millis(10),
            60,
            true,
        );
        records.call(
            "apply_color_temperature",
            Some(3400),
            Duration::from_millis(start),
            Duration::from_millis(completed),
            true,
        );
        if let Some((due, start, end)) = restoration {
            records.begin(
                Duration::from_millis(start),
                Duration::from_millis(due),
                60,
                true,
            );
            records.call(
                "apply_color_temperature",
                Some(3400),
                Duration::from_millis(start),
                Duration::from_millis(end),
                true,
            );
        }
        records.endpoint(kind, Duration::from_millis(due), 3400);
        let mut text = String::new();
        assert_eq!(
            records
                .report_endpoints(&mut text)
                .expect("endpoint report"),
            expected,
            "{text}"
        );
    }

    #[test_case(Lifecycle::Ready, true; "startup succeeds")]
    #[test_case(Lifecycle::Stopped, false; "startup fails")]
    fn readiness_reports_the_startup_outcome(state: Lifecycle, expected_ok: bool) {
        let measurement = OutputMeasurement::default();
        let input = measurement.clone();
        let (started, beginning) = std::sync::mpsc::sync_channel(1);
        let (completed, results) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            started.send(()).expect("test receives startup");
            completed
                .send(input.wait_ready())
                .expect("test receives outcome");
        });
        beginning
            .recv_timeout(Duration::from_secs(5))
            .expect("input thread starts");
        *measurement.0.lifecycle.lock().expect("lifecycle lock") = state;
        measurement.0.ready.notify_all();
        let result = results
            .recv_timeout(Duration::from_secs(5))
            .expect("readiness completes");
        thread.join().expect("input thread returns");
        assert_eq!(result.is_ok(), expected_ok, "{result:?}");
    }

    #[test]
    fn a_late_controller_deadline_does_not_make_the_endpoint_timely() {
        let mut records = Records::default();
        records.begin(
            Duration::from_millis(500),
            Duration::from_millis(500),
            60,
            true,
        );
        records.call(
            "apply_color_temperature",
            Some(3400),
            Duration::from_millis(500),
            Duration::from_millis(501),
            true,
        );
        records.endpoint("scheduled", Duration::from_millis(100), 3400);
        let mut text = String::new();
        assert!(
            !records
                .report_endpoints(&mut text)
                .expect("endpoint report"),
            "{text}"
        );
    }

    #[test_case(100, 110, 110, true; "rate limited endpoint")]
    #[test_case(100, 200, 200, true; "serialized endpoint")]
    #[test_case(100, 200, 221, false; "late after serialized call")]
    fn endpoint_timing_preserves_rate_and_serialization_allowances(
        previous_start: u64,
        previous_end: u64,
        started: u64,
        expected: bool,
    ) {
        let mut records = Records::default();
        records.begin(
            Duration::from_millis(previous_start),
            Duration::from_millis(previous_start),
            100,
            true,
        );
        records.call(
            "apply_color_temperature",
            Some(3500),
            Duration::from_millis(previous_start),
            Duration::from_millis(previous_end),
            true,
        );
        records.begin(
            Duration::from_millis(started),
            Duration::from_millis(105),
            100,
            false,
        );
        records.call(
            "apply_color_temperature",
            Some(3400),
            Duration::from_millis(started),
            Duration::from_millis(started + 1),
            true,
        );
        records.endpoint("scheduled", Duration::from_millis(105), 3400);
        let mut text = String::new();
        assert_eq!(
            records
                .report_endpoints(&mut text)
                .expect("endpoint report"),
            expected,
            "{text}"
        );
    }
}
