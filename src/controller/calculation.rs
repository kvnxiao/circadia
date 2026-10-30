use crate::ActivityInterval;
use crate::Config;
use crate::Curve;
use crate::EventKind;
use crate::Gridline;
use crate::Playback;
use crate::Schedule;
use crate::ScheduleError;
use crate::TimeInterval;
use crate::WindowsZone;
use crate::graph_interval;
use crate::gridlines;
use crate::solar_today;
use jiff::SignedDuration;
use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::civil::Time;
use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread::JoinHandle;

/// Select whether a calculation also captures a new Preview interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalculationPurpose {
    /// Recalculate the current graph and native schedule.
    Schedule,
    /// Capture a new Preview and calculate its complete interval.
    Preview,
}

/// Describe immutable inputs for independent graph and schedule calculation.
#[derive(Clone, Debug)]
pub struct CalculationRequest {
    /// Validated active or captured configuration.
    pub config: Config,
    /// Actual instant at the request's logical entry.
    pub at: Timestamp,
    /// Select current scheduling or captured playback.
    pub purpose: CalculationPurpose,
}

/// Share one calculated curve between native output and visible graph
/// rendering.
#[derive(Debug)]
pub struct CalculatedSchedule {
    /// Complete generated curve, including interpolation neighbors.
    pub curve: Arc<Curve>,
    /// Selected Wake, Sunrise, Sunset, and Bedtime targets of the curve.
    pub events: Vec<LocalEvent>,
    /// Local clocks at changing segment starts, using the calculation's zone.
    pub transition_starts: BTreeMap<Timestamp, DateTime>,
    /// Adjusted Wake for the activity interval starting on the request's local
    /// date.
    pub today_wake: DateTime,
    /// Adjusted Bedtime following today's Wake, including after midnight.
    pub today_bedtime: DateTime,
    /// Local clock hours divisible by three across all graph views.
    pub gridlines: Vec<Gridline>,
    /// Bounds for which solar and clock inputs were evaluated.
    pub valid: TimeInterval,
    /// Current Wake-minus-four-hours graph interval.
    pub graph: TimeInterval,
    /// Contiguous graph intervals needed throughout captured playback.
    pub views: Vec<TimeInterval>,
    /// Captured playback, present only for a new Preview request.
    pub preview: Option<Arc<Playback>>,
    /// Next local date boundary requiring fresh input calculation.
    pub next_date: Timestamp,
    /// Required solar calculation failure when the curve uses manual anchors.
    pub solar_error: Option<ScheduleError>,
    /// Activity intervals whose solar events could not form an ordered pair.
    pub conflicts: Vec<SolarConflict>,
    /// Today's selected solar targets, or none when their calculation failed.
    pub today: Option<SolarToday>,
}

/// Give a selected schedule target with its clock in the calculation's Windows
/// time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalEvent {
    /// Event that the target belongs to.
    pub kind: EventKind,
    /// Actual instant before minimum-spacing merges.
    pub instant: Timestamp,
    /// Local date and clock of the instant.
    pub local: DateTime,
}

/// Give the selected Sunrise and Sunset targets of the activity interval that
/// starts on the request's local date, in the calculation's Windows time zone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SolarToday {
    /// Local Sunrise target, absent when disabled, absent, or conflicting.
    pub sunrise: Option<DateTime>,
    /// Local Sunset target, absent when disabled, absent, or conflicting.
    pub sunset: Option<DateTime>,
}

/// Describe a solar ordering conflict in the calculation's Windows time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolarConflict {
    /// Actual bounds used to select conflicts for a displayed graph interval.
    pub interval: ActivityInterval,
    /// Adjusted Wake date and clock in the calculation's Windows time zone.
    pub local_wake: DateTime,
    /// Adjusted Bedtime date and clock in the calculation's Windows time zone.
    pub local_bedtime: DateTime,
}

/// Tag a calculation so consumers reject results from superseded requests.
#[derive(Debug)]
pub struct CalculationResult {
    /// Monotonic request number assigned when the request was accepted.
    pub revision: u64,
    /// Immutable inputs that produced this result.
    pub request: CalculationRequest,
    /// Current curve or clock-conversion failure.
    pub result: Result<CalculatedSchedule, ScheduleError>,
}

#[derive(Default)]
struct Work {
    revision: u64,
    pending: Option<CalculationRequest>,
    latest: Option<Arc<CalculationResult>>,
    stopping: bool,
}

#[derive(Default)]
struct Shared {
    work: Mutex<Work>,
    ready: Condvar,
}

struct CloseAdmission<'a>(&'a Shared);

impl Drop for CloseAdmission<'_> {
    fn drop(&mut self) {
        let mut work = self
            .0
            .work
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        work.stopping = true;
        work.pending = None;
        drop(work);
        self.0.ready.notify_all();
    }
}

/// Submit replaceable calculation requests without waiting for native output.
#[derive(Clone)]
pub struct CalculationClient(Arc<Shared>);

impl fmt::Debug for CalculationClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CalculationClient").finish_non_exhaustive()
    }
}

impl CalculationClient {
    /// Check whether a prepared graph request still owns the calculation slot.
    ///
    /// # Errors
    /// Report poisoned synchronization.
    pub fn is_current(&self, revision: u64) -> io::Result<bool> {
        self.0
            .work
            .lock()
            .map(|work| !work.stopping && work.revision == revision)
            .map_err(|_error| io::Error::other("calculation state lock failed"))
    }

    /// Replace queued calculation work and return its revision.
    ///
    /// # Errors
    /// Reject a stopped worker, poisoned synchronization, or revision
    /// exhaustion.
    pub fn submit(&self, request: CalculationRequest) -> io::Result<u64> {
        let mut work = self
            .0
            .work
            .lock()
            .map_err(|_error| io::Error::other("calculation state lock failed"))?;
        if work.stopping {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "calculation worker stopped",
            ));
        }
        work.revision = work
            .revision
            .checked_add(1)
            .ok_or_else(|| io::Error::other("calculation revision exhausted"))?;
        work.pending = Some(request);
        let revision = work.revision;
        drop(work);
        self.0.ready.notify_one();
        Ok(revision)
    }

    /// Read the last completed result; compare its revision before applying it.
    ///
    /// # Errors
    /// Report poisoned synchronization.
    pub fn latest(&self) -> io::Result<Option<Arc<CalculationResult>>> {
        self.0
            .work
            .lock()
            .map(|work| work.latest.clone())
            .map_err(|_error| io::Error::other("calculation state lock failed"))
    }
}

/// Own one calculation thread and join it during shutdown.
pub struct CalculationWorker {
    client: CalculationClient,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl fmt::Debug for CalculationWorker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CalculationWorker")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl CalculationWorker {
    /// Start independent calculation and notify consumers after publishing a
    /// result.
    ///
    /// - Keep `notify` nonblocking and free of panics.
    /// - At most one calculation runs and one latest request waits.
    ///
    /// # Errors
    /// Report failure to create the worker thread.
    pub fn start(notify: impl Fn() -> io::Result<()> + Send + 'static) -> io::Result<Self> {
        Self::with_calculator(calculate, notify)
    }

    fn with_calculator(
        calculate: impl Fn(&CalculationRequest) -> Result<CalculatedSchedule, ScheduleError>
        + Send
        + 'static,
        notify: impl Fn() -> io::Result<()> + Send + 'static,
    ) -> io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let worker = Arc::clone(&shared);
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        let thread = std::thread::Builder::new()
            .name("circadia-calculation".into())
            .spawn(move || {
                let _log = tracing::dispatcher::set_default(&dispatch);
                run_calculations(&worker, &calculate, &notify)
            })?;
        Ok(Self {
            client: CalculationClient(shared),
            thread: Some(thread),
        })
    }

    /// Clone access to the bounded request and result slots.
    pub fn client(&self) -> CalculationClient {
        self.client.clone()
    }

    /// Stop admission, discard queued work, and join the current calculation.
    ///
    /// # Errors
    /// Report synchronization, notification, or worker panic failures.
    pub fn shutdown(mut self) -> io::Result<()> {
        self.finish()
    }

    fn finish(&mut self) -> io::Result<()> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        {
            let mut work = self
                .client
                .0
                .work
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            work.stopping = true;
            work.pending = None;
        }
        self.client.0.ready.notify_one();
        thread
            .join()
            .map_err(|_panic| io::Error::other("calculation worker panicked"))?
    }
}

impl Drop for CalculationWorker {
    fn drop(&mut self) {
        drop(self.finish());
    }
}

fn run_calculations(
    worker: &Shared,
    calculate: &impl Fn(&CalculationRequest) -> Result<CalculatedSchedule, ScheduleError>,
    notify: &impl Fn() -> io::Result<()>,
) -> io::Result<()> {
    let _admission = CloseAdmission(worker);
    loop {
        let guard = worker
            .work
            .lock()
            .map_err(|_error| io::Error::other("calculation state lock failed"))?;
        let mut work = worker
            .ready
            .wait_while(guard, |work| work.pending.is_none() && !work.stopping)
            .map_err(|_error| io::Error::other("calculation state wait failed"))?;
        if work.stopping {
            return Ok(());
        }
        let revision = work.revision;
        let request = work
            .pending
            .take()
            .ok_or_else(|| io::Error::other("calculation request unavailable"))?;
        drop(work);
        let result = Arc::new(CalculationResult {
            revision,
            result: calculate(&request),
            request,
        });
        let mut work = worker
            .work
            .lock()
            .map_err(|_error| io::Error::other("calculation state lock failed"))?;
        if work.stopping {
            return Ok(());
        }
        if revision != work.revision {
            continue;
        }
        work.latest = Some(result);
        drop(work);
        notify()?;
    }
}

fn calculate(request: &CalculationRequest) -> Result<CalculatedSchedule, ScheduleError> {
    #[cfg(feature = "graph-measurement")]
    crate::activity::record(crate::ActivityEvent::Calculation);
    let mut zone = WindowsZone::current()?;
    request.calculate(&mut zone)
}

impl CalculationRequest {
    pub(crate) fn calculate(
        &self,
        zone: &mut WindowsZone,
    ) -> Result<CalculatedSchedule, ScheduleError> {
        let request = self;
        let graph = graph_interval(&request.config, zone, request.at)?;
        let next_local = zone
            .local(request.at)?
            .date()
            .tomorrow()?
            .to_datetime(Time::midnight());
        let next_date = zone.boundary(next_local)?;
        let preview = (request.purpose == CalculationPurpose::Preview)
            .then(|| Playback::capture(&request.config, zone, request.at).map(Arc::new))
            .transpose()?;
        let mut views = vec![graph];
        let mut graph_end = graph.end;
        let through = preview
            .as_ref()
            .map_or(next_date, |playback| playback.interval().end.max(next_date));
        while graph_end <= through {
            let view = graph_interval(&request.config, zone, graph_end)?;
            if view.start != graph_end || view.end <= graph_end {
                return Err(ScheduleError::curve());
            }
            graph_end = view.end;
            views.push(view);
        }
        let gridlines = gridlines(
            zone,
            TimeInterval {
                start: graph.start,
                end: graph_end,
            },
        )?;
        let padding = SignedDuration::from_millis(request.config.input().ordinary_fade_ms);
        let end = graph_end.max(next_date).checked_add(padding)?;
        let valid = TimeInterval {
            start: graph.start.min(request.at),
            end,
        };
        let schedule = Schedule::calculate(&request.config, zone, valid.start, valid.end)?;
        let (today_wake, today_bedtime) = today_clocks(&request.config, zone, request.at)?;
        let transition_starts = schedule
            .curve
            .segments()
            .iter()
            .filter(|segment| segment.from != segment.to)
            .map(|segment| Ok((segment.start, zone.local(segment.start)?)))
            .collect::<Result<_, ScheduleError>>()?;
        let conflicts = schedule
            .conflicts
            .into_iter()
            .map(|interval| {
                Ok(SolarConflict {
                    interval,
                    local_wake: zone.local(interval.wake)?,
                    local_bedtime: zone.local(interval.bedtime)?,
                })
            })
            .collect::<Result<_, ScheduleError>>()?;
        let today = match solar_today(&request.config, zone, request.at) {
            Ok(selection) => Some(SolarToday {
                sunrise: selection
                    .sunrise
                    .map(|instant| zone.local(instant))
                    .transpose()?,
                sunset: selection
                    .sunset
                    .map(|instant| zone.local(instant))
                    .transpose()?,
            }),
            Err(_) => None,
        };
        let events = schedule
            .events
            .iter()
            .map(|event| {
                Ok(LocalEvent {
                    kind: event.kind,
                    instant: event.instant,
                    local: zone.local(event.instant)?,
                })
            })
            .collect::<Result<_, ScheduleError>>()?;
        Ok(CalculatedSchedule {
            curve: Arc::new(schedule.curve),
            events,
            transition_starts,
            today_wake,
            today_bedtime,
            gridlines,
            valid,
            graph,
            views,
            preview,
            next_date,
            solar_error: schedule.solar_error,
            conflicts,
            today,
        })
    }
}

fn today_clocks(
    config: &Config,
    zone: &mut WindowsZone,
    at: Timestamp,
) -> Result<(DateTime, DateTime), ScheduleError> {
    let today = zone.local(at)?.date();
    let anchors = crate::resolve_anchors(config, zone, today, today.tomorrow()?)?;
    let (wake, bedtime) = anchors
        .iter()
        .zip(anchors.iter().skip(1))
        .find(|(wake, bedtime)| {
            wake.kind == crate::AnchorKind::Wake
                && bedtime.kind == crate::AnchorKind::Bedtime
                && wake.nominal.date() == today
        })
        .ok_or_else(ScheduleError::curve)?;
    Ok((zone.local(wake.instant)?, zone.local(bedtime.instant)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn request(second: i64) -> CalculationRequest {
        CalculationRequest {
            config: Config::default(),
            at: Timestamp::from_second(second).expect("request instant"),
            purpose: CalculationPurpose::Schedule,
        }
    }

    #[test]
    fn queued_edits_replace_obsolete_work_and_results() {
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let worker = CalculationWorker::with_calculator(
            move |request| {
                if request.at.as_second() == 1 {
                    entered_tx.send(()).expect("entered calculation");
                    release_rx
                        .recv_timeout(Duration::from_secs(2))
                        .expect("released calculation");
                }
                Err(ScheduleError::curve())
            },
            move || done_tx.try_send(()).map_err(io::Error::other),
        )
        .expect("worker");
        let client = worker.client();
        assert_eq!(client.submit(request(1)).expect("first"), 1);
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("first running");
        assert_eq!(client.submit(request(2)).expect("second"), 2);
        assert_eq!(client.submit(request(3)).expect("third"), 3);
        release_tx.send(()).expect("release first");
        done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("latest published");
        let result = client
            .latest()
            .expect("result slot")
            .expect("completed result");
        assert_eq!(result.revision, 3);
        assert_eq!(result.request.at.as_second(), 3);
        assert!(done_rx.try_recv().is_err());
        worker.shutdown().expect("joined worker");
        assert_eq!(
            client
                .submit(request(4))
                .expect_err("closed admission")
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }

    #[test]
    fn graph_views_cover_the_interval_until_the_next_date_recalculation() {
        let mut zone = WindowsZone::current().expect("Windows zone");
        let at = zone
            .boundary("2026-02-01T00:00:00".parse().expect("local midnight"))
            .expect("midnight instant");
        let calculated = calculate(&CalculationRequest {
            config: Config::default(),
            at,
            purpose: CalculationPurpose::Schedule,
        })
        .expect("schedule");
        assert!(
            calculated.views.last().expect("final view").end >= calculated.next_date,
            "the graph must cross the 03:00 display boundary before the next midnight recalculation"
        );
        for view in &calculated.views {
            assert!(calculated.curve.clip(*view).is_ok());
        }
    }

    #[test]
    fn live_calculation_reads_clock_rules_without_accessing_night_light() {
        let request = CalculationRequest {
            config: Config::default(),
            at: "2026-02-01T12:00:00Z".parse().expect("instant"),
            purpose: CalculationPurpose::Preview,
        };
        let calculated = calculate(&request).expect("calculation");
        let preview = calculated.preview.expect("captured preview");
        assert_eq!(preview.duration(), Duration::from_secs(10));
        assert!(calculated.valid.start <= request.at);
        assert!(calculated.valid.end > preview.interval().end);
        assert!(calculated.next_date > request.at);
        assert!(calculated.curve.rounded(request.at).is_ok());
        assert!(calculated.curve.clip(calculated.graph).is_ok());
        assert!(calculated.views.last().expect("final view").end > preview.interval().end);
        for view in calculated.views {
            assert!(calculated.curve.clip(view).is_ok());
        }
    }

    #[test]
    fn failed_notification_closes_admission_and_is_reported_by_shutdown() {
        let worker = CalculationWorker::with_calculator(
            |_request| Err(ScheduleError::curve()),
            || Err(io::Error::other("notification failed")),
        )
        .expect("worker");
        let client = worker.client();
        client.submit(request(1)).expect("request");
        let guard = client.0.work.lock().expect("state lock");
        let (work, timeout) = client
            .0
            .ready
            .wait_timeout_while(guard, Duration::from_secs(2), |work| !work.stopping)
            .expect("termination signal");
        assert!(!timeout.timed_out());
        assert!(work.stopping);
        drop(work);
        assert_eq!(
            client
                .submit(request(2))
                .expect_err("closed admission")
                .kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(
            worker
                .shutdown()
                .expect_err("notification error")
                .to_string(),
            "notification failed"
        );
    }
}
