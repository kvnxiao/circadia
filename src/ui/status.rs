use crate::CalculatedSchedule;
use crate::CalculationPurpose;
use crate::CalculationResult;
use crate::ControllerSnapshot;
use crate::DiagnosticOperation;
use crate::EventKind;
use crate::LocalEvent;
use crate::ManagementState;
use crate::ManualCommand;
use crate::RecentApp;
use crate::SchedulingState;
use crate::Segment;
use crate::SuppressionState;
use crate::l10n::Text;
use crate::l10n::bind_text;
use crate::platform::nls::RegionalFormat;
use crate::ui::configuration::Configuration;
use crate::ui::configuration::Toggles;
use jiff::SignedDuration;
use jiff::Timestamp;
use jiff::civil::DateTime;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;
use windows_reactor::MenuItem;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent condition of the status table"
)]
pub(super) struct Stored {
    pub(super) error: bool,
    pub(super) paused: bool,
    pub(super) failed: bool,
    pub(super) restored: bool,
}

impl Stored {
    pub(super) fn new(configuration: &Configuration) -> Self {
        Self {
            error: configuration.accepted().is_none(),
            paused: configuration.paused(),
            failed: configuration.write_error().is_some(),
            restored: configuration.restore().is_some(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TimeOfDay {
    Morning,
    Daytime,
    Evening,
    Bedtime,
}

impl TimeOfDay {
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Morning => "morning",
            Self::Daytime => "daytime",
            Self::Evening => "evening",
            Self::Bedtime => "bedtime",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Picture {
    NotManaging,
    Phase(TimeOfDay),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Badge {
    Timer,
    AppWindow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PhaseName {
    NotManaging,
    Preview,
    Suppressed,
    Shifting(TimeOfDay),
    Current(TimeOfDay),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Phase {
    pub(super) name: PhaseName,
    pub(super) picture: Picture,
    pub(super) badge: Option<Badge>,
    pub(super) progress: Option<f64>,
}

impl Phase {
    pub(super) fn label(&self, text: &Text) -> String {
        bind_text!(fl, text);
        match self.name {
            PhaseName::NotManaging => fl!("phase-name", phase = "not-managing"),
            PhaseName::Preview => fl!("phase-name", phase = "preview"),
            PhaseName::Suppressed => fl!("phase-name", phase = "suppressed"),
            PhaseName::Shifting(to) => fl!("phase-shifting", phase = to.key()),
            PhaseName::Current(phase) => fl!("phase-name", phase = phase.key()),
        }
    }

    pub(super) fn art_name(&self, text: &Text) -> String {
        bind_text!(fl, text);
        match (self.picture, self.badge, self.progress) {
            (Picture::NotManaging, ..) => fl!("phase-art-not-managing"),
            (_, Some(Badge::Timer), _) => fl!("phase-art-manual"),
            (_, Some(Badge::AppWindow), _) => fl!("phase-art-automatic"),
            (Picture::Phase(phase), None, Some(progress)) => fl!(
                "phase-art-shifting",
                phase = phase.key(),
                percent = percent(progress)
            ),
            (Picture::Phase(phase), None, None) => fl!("phase-name", phase = phase.key()),
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "progress is clamped to 0 through 1 before scaling to a percentage"
)]
fn percent(progress: f64) -> u8 {
    (progress.clamp(0.0, 1.0) * 100.0).round() as u8
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Position {
    Steady(TimeOfDay),
    Shifting {
        event: EventKind,
        to: TimeOfDay,
        progress: f64,
        local: DateTime,
        kelvin: u16,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NextChange {
    pub(super) event: EventKind,
    pub(super) local: DateTime,
    pub(super) kelvin: u16,
    pub(super) from: Option<DateTime>,
    pub(super) warming: bool,
}

pub(super) fn next_change(schedule: &CalculatedSchedule, at: Timestamp) -> Option<NextChange> {
    let now = scheduled_kelvin(schedule, at)?;
    schedule
        .events
        .iter()
        .filter(|event| event.instant > at)
        .find_map(|event| {
            let kelvin = scheduled_kelvin(schedule, event.instant)?;
            let ramp = schedule.curve.segments().iter().find(|segment| {
                segment.start >= at && segment.start < event.instant && segment.from != segment.to
            });
            (ramp.is_some() || kelvin != now).then(|| NextChange {
                event: event.kind,
                local: event.local,
                kelvin,
                from: ramp
                    .and_then(|segment| schedule.transition_starts.get(&segment.start).copied()),
                warming: kelvin < now,
            })
        })
}

fn segment(schedule: &CalculatedSchedule, at: Timestamp) -> Option<&Segment> {
    schedule
        .curve
        .segments()
        .iter()
        .find(|segment| segment.start <= at && at < segment.end)
}

// The controller's curve lookups record activity measurements, so settings
// reads the segments directly.
pub(super) fn segment_at(schedule: &CalculatedSchedule, at: Timestamp) -> Option<&Segment> {
    schedule
        .curve
        .segments()
        .iter()
        .find(|segment| segment.start <= at && at <= segment.end)
}

pub(super) fn scheduled_kelvin(schedule: &CalculatedSchedule, at: Timestamp) -> Option<u16> {
    segment_at(schedule, at).and_then(|segment| segment.rounded(at).ok())
}

// A merged target keeps the earlier instant with the later event's
// temperature, so the event a segment reaches is the last one in that minute.
fn reached(events: &[LocalEvent], at: Timestamp) -> Option<usize> {
    let merge = at.checked_add(SignedDuration::from_secs(60)).ok()?;
    events
        .iter()
        .rposition(|event| at <= event.instant && event.instant < merge)
}

fn starts(events: &[LocalEvent], index: usize) -> Option<TimeOfDay> {
    let event = events.get(index)?;
    Some(match event.kind {
        EventKind::Wake => {
            let sunrise = events
                .iter()
                .skip(index + 1)
                .take_while(|next| next.kind != EventKind::Wake && next.kind != EventKind::Bedtime)
                .any(|next| next.kind == EventKind::Sunrise);
            if sunrise {
                TimeOfDay::Morning
            } else {
                TimeOfDay::Daytime
            }
        }
        EventKind::Sunrise => TimeOfDay::Daytime,
        EventKind::Sunset => TimeOfDay::Evening,
        EventKind::Bedtime => TimeOfDay::Bedtime,
    })
}

pub(super) fn position(schedule: &CalculatedSchedule, at: Timestamp) -> Option<Position> {
    let events = &schedule.events;
    if let Some(segment) = segment(schedule, at).filter(|segment| segment.from != segment.to) {
        let index = reached(events, segment.end)?;
        let event = events.get(index)?;
        let span = segment.end.duration_since(segment.start).as_secs_f64();
        return Some(Position::Shifting {
            event: event.kind,
            to: starts(events, index)?,
            progress: at.duration_since(segment.start).as_secs_f64() / span,
            local: event.local,
            kelvin: segment.to,
        });
    }
    let index = events.iter().rposition(|event| event.instant <= at)?;
    starts(events, index).map(Position::Steady)
}

pub(super) fn preview_deadline(snapshot: &ControllerSnapshot) -> Option<Instant> {
    snapshot
        .preview_deadline
        .filter(|_| snapshot.preview_active)
        .and_then(|deadline| snapshot.origin.checked_add(deadline))
}

pub(super) fn preview_active(snapshot: &ControllerSnapshot, now: Instant) -> bool {
    snapshot.preview_active && preview_deadline(snapshot).is_none_or(|deadline| now < deadline)
}

pub(super) fn preview_stopped(deadline: Option<Instant>, stopped: Option<Instant>) -> bool {
    stopped.is_some() && deadline == stopped
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Control<'a> {
    pub(super) management: ManagementState,
    pub(super) scheduling: SchedulingState,
    pub(super) suppression: SuppressionState,
    pub(super) preview_active: bool,
    pub(super) playback: Option<Timestamp>,
    pub(super) schedule: Option<&'a CalculatedSchedule>,
    pub(super) last_scheduled: Option<u16>,
    pub(super) output_failed: bool,
    pub(super) observation_failed: bool,
}

impl<'a> Control<'a> {
    pub(super) fn new(
        snapshot: &'a ControllerSnapshot,
        now: Instant,
        stopped: Option<Instant>,
    ) -> Self {
        let managing = snapshot.management.is_on();
        let preview_active =
            preview_active(snapshot, now) && !preview_stopped(preview_deadline(snapshot), stopped);
        Self {
            management: snapshot.management,
            scheduling: snapshot.scheduling,
            suppression: snapshot.suppression,
            preview_active,
            playback: snapshot
                .preview
                .as_ref()
                .filter(|_| preview_active)
                .and_then(|preview| {
                    let started = snapshot.origin.checked_add(preview.start)?;
                    preview
                        .playback
                        .at(now.saturating_duration_since(started))
                        .ok()
                }),
            schedule: snapshot
                .calculation
                .as_ref()
                .and_then(|calculation| calculation.result.as_ref().ok()),
            last_scheduled: snapshot
                .accepted
                .as_ref()
                .and_then(crate::ConfigLayer::last_scheduled),
            output_failed: managing
                && snapshot
                    .native_errors
                    .keys()
                    .any(|operation| *operation != DiagnosticOperation::Observation),
            observation_failed: snapshot
                .native_errors
                .contains_key(&DiagnosticOperation::Observation),
        }
    }

    pub(super) fn kelvin(&self, current: Timestamp) -> Option<u16> {
        if self.preview_active {
            return scheduled_kelvin(self.schedule?, self.playback?);
        }
        if self.suppression.active() {
            return Some(6500);
        }
        match self.scheduling {
            SchedulingState::Retained => self.last_scheduled,
            SchedulingState::Available | SchedulingState::SolarFallback => {
                scheduled_kelvin(self.schedule?, current)
            }
            SchedulingState::Waiting | SchedulingState::Evaluating | SchedulingState::Stopped => {
                None
            }
        }
    }

    pub(super) fn phase(&self, current: Timestamp) -> Phase {
        let suppressed = |badge| Phase {
            name: PhaseName::Suppressed,
            picture: Picture::Phase(TimeOfDay::Daytime),
            badge: Some(badge),
            progress: None,
        };
        if !self.management.is_on() {
            return Phase {
                name: PhaseName::NotManaging,
                picture: Picture::NotManaging,
                badge: None,
                progress: None,
            };
        }
        if !self.preview_active {
            if self.suppression.manual() {
                return suppressed(Badge::Timer);
            }
            if self.suppression.automatic {
                return suppressed(Badge::AppWindow);
            }
        }
        let at = self
            .playback
            .filter(|_| self.preview_active)
            .unwrap_or(current);
        let (picture, progress, name) =
            match self.schedule.and_then(|schedule| position(schedule, at)) {
                Some(Position::Shifting { to, progress, .. }) => {
                    (to, Some(progress), PhaseName::Shifting(to))
                }
                Some(Position::Steady(phase)) => (phase, None, PhaseName::Current(phase)),
                None => (
                    TimeOfDay::Daytime,
                    None,
                    PhaseName::Current(TimeOfDay::Daytime),
                ),
            };
        Phase {
            name: if self.preview_active {
                PhaseName::Preview
            } else {
                name
            },
            picture: Picture::Phase(picture),
            badge: None,
            progress,
        }
    }
}

// The calculation thread publishes each result before the controller, which can
// wait on a native call, applies it; the graph shows the newer result.
pub(super) fn with_latest(
    snapshot: Arc<ControllerSnapshot>,
    latest: Option<Arc<CalculationResult>>,
) -> Arc<ControllerSnapshot> {
    let current = snapshot
        .calculation
        .as_ref()
        .map(|calculation| calculation.revision);
    match latest {
        Some(latest)
            if !preview_active(&snapshot, Instant::now())
                && latest.request.purpose == CalculationPurpose::Schedule
                && current.is_none_or(|current| latest.revision > current) =>
        {
            Arc::new(ControllerSnapshot {
                calculation: Some(latest),
                ..(*snapshot).clone()
            })
        }
        _ => snapshot,
    }
}

pub(super) fn kelvin_text(region: &RegionalFormat, kelvin: u16) -> String {
    region
        .format_kelvin(kelvin)
        .unwrap_or_else(|_| format!("{kelvin} K"))
}

pub(super) fn status_line(
    text: &Text,
    region: &RegionalFormat,
    control: Option<&Control<'_>>,
    stored: Stored,
    current: Timestamp,
) -> String {
    bind_text!(fl, text);
    if stored.error {
        return fl!("status-configuration-error");
    }
    let Some(control) = control else {
        return fl!("status-starting");
    };
    if !control.management.is_on() {
        return fl!("status-not-managing");
    }
    if control.scheduling == SchedulingState::Stopped {
        return fl!("status-control-unavailable");
    }
    if control.output_failed {
        return if control.suppression.active() {
            fl!("status-output-unavailable-suppressed")
        } else {
            fl!("status-output-unavailable")
        };
    }
    if control.scheduling == SchedulingState::Retained {
        return control.last_scheduled.map_or_else(
            || fl!("status-control-unavailable"),
            |value| {
                fl!(
                    "status-schedule-retained",
                    kelvin = kelvin_text(region, value)
                )
            },
        );
    }
    if stored.paused {
        return fl!("status-saving-paused");
    }
    if stored.failed {
        return fl!("status-write-failed");
    }
    if stored.restored {
        return fl!("status-backup-restored");
    }
    let running = matches!(
        control.scheduling,
        SchedulingState::Available | SchedulingState::SolarFallback
    );
    if running && control.observation_failed {
        return fl!("status-observation-failed");
    }
    let Some(value) = control.kelvin(current).filter(|_| running) else {
        return fl!("status-calculating");
    };
    let state = if control.preview_active {
        "preview"
    } else if control.suppression.active() {
        "suppressed"
    } else {
        "schedule"
    };
    let solar = if control.scheduling == SchedulingState::SolarFallback {
        "fallback"
    } else {
        "available"
    };
    fl!(
        "status-managing",
        state = state,
        solar = solar,
        kelvin = kelvin_text(region, value)
    )
}

pub(super) fn tooltip(text: &Text, status: &str) -> String {
    bind_text!(fl, text);
    fl!("tooltip", status = status)
}

pub(super) fn announcement(
    text: &Text,
    control: &Control<'_>,
    stored: Stored,
    current: Timestamp,
) -> String {
    bind_text!(fl, text);
    if stored.error {
        return fl!("announcement", status = fl!("status-configuration-error"));
    }
    let mode = match control.management {
        ManagementState::Off => "off",
        ManagementState::TurningOn => "starting",
        ManagementState::On => "on",
        ManagementState::TurningOff => "stopping",
        ManagementState::Exiting => "exiting",
    };
    let mut parts = vec![fl!("announcement-management", mode = mode)];
    if control.management.is_on() {
        let mut phase = control.phase(current);
        if control.preview_active {
            phase.name = match (phase.picture, phase.progress) {
                (Picture::Phase(picture), Some(_)) => PhaseName::Shifting(picture),
                (Picture::Phase(picture), None) => PhaseName::Current(picture),
                (Picture::NotManaging, _) => PhaseName::NotManaging,
            };
        }
        let phase_text = if control.preview_active {
            if control.playback.is_none() {
                fl!("preview-starting")
            } else {
                fl!("announcement-preview", phase = phase.label(text))
            }
        } else if phase.badge.is_some() {
            fl!("announcement-phase", phase = phase.art_name(text))
        } else if control.schedule.is_some() {
            fl!("announcement-phase", phase = phase.label(text))
        } else {
            String::new()
        };
        if !phase_text.is_empty() {
            parts.push(phase_text);
        }
        parts.push(match control.scheduling {
            SchedulingState::Waiting | SchedulingState::Evaluating => {
                fl!("status-calculating")
            }
            SchedulingState::Available => fl!("announcement-schedule-available"),
            SchedulingState::SolarFallback => fl!("announcement-solar-fallback"),
            SchedulingState::Retained => fl!("announcement-schedule-retained"),
            SchedulingState::Stopped => fl!("status-control-unavailable"),
        });
        if control.output_failed {
            parts.push(fl!("status-output-unavailable"));
        }
        if control.observation_failed {
            parts.push(fl!("status-observation-failed"));
        }
    }
    if stored.paused {
        parts.push(fl!("status-saving-paused"));
    }
    if stored.failed {
        parts.push(fl!("status-write-failed"));
    }
    if stored.restored {
        parts.push(fl!("status-backup-restored"));
    }
    fl!("announcement", status = parts.join(" "))
}

pub(super) fn manual_label(text: &Text, manual: ManualCommand) -> String {
    bind_text!(fl, text);
    match manual {
        ManualCommand::Start => fl!("manual-title"),
        ManualCommand::Stop => fl!("manual-stop"),
    }
}

fn item(key: impl Into<windows_reactor::Key>, label: String, enabled: bool) -> MenuItem {
    if enabled {
        MenuItem::item(key, label)
    } else {
        MenuItem::disabled(key, label)
    }
}

// Normalized executable file names cannot contain ':', so the `add:` and
// `remove:` keys stay distinct from the fixed keys.
pub(super) fn menu_items(
    text: &Text,
    snapshot: Option<&ControllerSnapshot>,
    manual: ManualCommand,
    toggles: Toggles,
    names: &BTreeMap<crate::ExecutablePath, String>,
) -> Vec<MenuItem> {
    bind_text!(fl, text);
    let manual_key = match manual {
        ManualCommand::Start => "manual-start",
        ManualCommand::Stop => "manual-stop",
    };
    let manual_enabled = snapshot.is_some_and(|snapshot| {
        manual_available(
            snapshot.management,
            snapshot.scheduling,
            snapshot.suppression,
        )
    });
    let reason = match toggles {
        Toggles::Available => "none",
        Toggles::ConfigurationError => "error",
        Toggles::Paused => "paused",
    };
    let mut items = vec![
        MenuItem::item("settings", fl!("tray-settings")),
        MenuItem::separator("separator-manual"),
        item(manual_key, manual_label(text, manual), manual_enabled),
        MenuItem::separator("separator-recent"),
        MenuItem::disabled("caption", fl!("tray-caption", reason = reason)),
    ];
    let history = snapshot.map_or(&[][..], |snapshot| snapshot.recent.as_slice());
    items.extend(history.iter().take(3).map(|recent| {
        let executable = recent.executable.as_str();
        let name = if recent.condition.is_empty() {
            recent
                .path
                .to_str()
                .and_then(|path| crate::ExecutablePath::new(path).ok())
                .as_ref()
                .and_then(|path| names.get(path))
                .map_or(executable, String::as_str)
        } else {
            recent.condition.as_str()
        };
        let action = if recent.included { "remove" } else { "add" };
        let path = recent.path.to_string_lossy();
        let regex = snapshot
            .and_then(|snapshot| snapshot.accepted.as_ref())
            .into_iter()
            .flat_map(|layer| layer.config().entries(crate::Field::Suppression))
            .find(|entry| {
                entry.path.is_none() && crate::same_executable(&entry.executable, executable)
            })
            .is_some_and(|entry| entry.regex);
        let key = format!("{action}:{}:{path}", u8::from(regex));
        let folder = history
            .iter()
            .filter(|other| crate::same_executable(&other.executable, executable))
            .nth(1)
            .and_then(|_| crate::ExecutablePath::new(&path).ok())
            .map(|path| path.folder().to_owned());
        let label = recent_label(text, recent, name, folder.as_deref());
        if recent.excluded {
            MenuItem::disabled(key, label)
        } else {
            item(
                key,
                label,
                toggles == Toggles::Available && (!recent.included || recent.removable),
            )
        }
    }));
    items.push(MenuItem::separator("separator-quit"));
    items.push(MenuItem::item("quit", fl!("tray-quit")));
    items
}

fn recent_label(text: &Text, recent: &RecentApp, name: &str, folder: Option<&str>) -> String {
    bind_text!(fl, text);
    let format = |[name, executable, directory]: [&str; 3]| {
        let listed = if recent.included { "yes" } else { "no" };
        let app = if recent.excluded {
            fl!(
                "tray-app-excluded",
                listed = listed,
                name = name,
                executable = executable
            )
        } else {
            fl!(
                "tray-app",
                listed = listed,
                name = name,
                executable = executable
            )
        };
        if folder.is_some() {
            fl!("tray-installation", app = app.as_str(), folder = directory)
        } else {
            app
        }
    };
    let mut budget = 96_usize.saturating_sub(format([""; 3]).chars().count());
    let mut fields = [name, recent.executable.as_str(), folder.unwrap_or_default()]
        .map(|value| (value, value.chars().take(budget + 1).count(), 0));
    let count = fields.len();
    let mut ordered = fields.each_mut();
    ordered.sort_by_key(|(_, length, _)| *length);
    for (position, (_, length, limit)) in ordered.into_iter().enumerate() {
        *limit = (*length).min(budget / (count - position));
        budget -= *limit;
    }
    let bounded = fields.map(|(value, _, limit)| middle_ellipsis(value, limit));
    format(bounded.each_ref().map(String::as_str))
}

fn middle_ellipsis(value: &str, limit: usize) -> String {
    if value.chars().take(limit + 1).count() <= limit {
        return value.to_owned();
    }
    if limit == 0 {
        return String::new();
    }
    let prefix = (limit - 1).div_ceil(2);
    let suffix = (limit - 1) / 2;
    value
        .chars()
        .take(prefix)
        .chain(std::iter::once('…'))
        .chain(
            value
                .chars()
                .rev()
                .take(suffix)
                .collect::<Vec<_>>()
                .into_iter()
                .rev(),
        )
        .collect()
}

pub(super) fn manual_available(
    management: ManagementState,
    scheduling: SchedulingState,
    suppression: SuppressionState,
) -> bool {
    suppression.manual()
        || (management.is_on()
            && matches!(
                scheduling,
                SchedulingState::Available
                    | SchedulingState::SolarFallback
                    | SchedulingState::Retained
            ))
}

#[cfg(test)]
mod tests;
