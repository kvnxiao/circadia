use super::Model;
use super::Page;
use crate::DiagnosticFailure;
use crate::DiagnosticOperation;
use crate::Field;
use crate::FieldErrorKind;
use crate::FieldValue;
use crate::Mark;
use crate::SchedulingState;
use crate::Source;
use crate::Unusable;
use crate::automation_id;
use crate::l10n::Text;
use crate::l10n::bind_text;
use crate::platform::nls::RegionalFormat;
use crate::ui::configuration::FileProblem;
use std::collections::BTreeSet;
use windows_reactor::InfoBarSeverity;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui) enum Action {
    Reset,
    OpenFolder,
    OpenLogs,
    Report,
    Retry,
    RetryNative,
    GoTo(Page),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Banner {
    pub(super) id: &'static str,
    pub(super) severity: InfoBarSeverity,
    pub(super) title: String,
    pub(super) message: String,
    pub(super) actions: Vec<(Action, String)>,
    pub(super) dismiss: Option<super::Command>,
    pub(super) enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Severity {
    Error,
    Warning,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FeatureMessage {
    pub(super) severity: Severity,
    pub(super) text: String,
    pub(super) retry: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Feedback {
    Error(String),
    Note(String),
}

pub(super) fn page(field: Field) -> Page {
    match field {
        Field::Wake
        | Field::Bedtime
        | Field::DaytimeKelvin
        | Field::MiddleKelvin
        | Field::SleepKelvin
        | Field::SolarTiming
        | Field::Sunrise
        | Field::Sunset
        | Field::UseWindowsLocation
        | Field::Coordinates
        | Field::ManageNightLight => Page::Schedule,
        Field::Suppression | Field::Exclusions | Field::FocusDelayMs => Page::Apps,
        Field::ScheduledTransitionMinutes
        | Field::OrdinaryFadeMs
        | Field::OutputRateHz
        | Field::PreviewSeconds => Page::Transitions,
        Field::StartAtSignIn | Field::StartMinimized | Field::InterfaceLanguage => Page::General,
    }
}

pub(super) fn marked_pages(model: &Model) -> BTreeSet<Page> {
    let entries = model
        .accepted
        .as_ref()
        .is_some_and(|config| config.has_entry_problems());
    model
        .errors
        .keys()
        .chain(model.report.marks.keys())
        .map(|field| page(*field))
        .chain(entries.then_some(Page::Apps))
        .collect()
}

pub(super) fn feedback(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    field: Field,
) -> Option<Feedback> {
    bind_text!(fl, text);
    let in_use = model.config().value(field);
    if let Some(kind) = model.errors.get(&field) {
        return Some(Feedback::Error(error_message(text, region, field, kind)));
    }
    if let Some(mark) = model.report.marks.get(&field) {
        return Some(Feedback::Error(mark_message(
            text, region, field, mark, &in_use,
        )));
    }
    if let Some(source) = model.report.missing.get(&field) {
        return Some(Feedback::Note(format!(
            "{} {}",
            fl!("missing-note"),
            using(text, region, field, *source, &in_use)
        )));
    }
    let prefix = format!("{}.", field.key());
    let missing = |key: &str| {
        model
            .report
            .missing_entry_keys
            .iter()
            .any(|missing| missing.strip_prefix(&prefix) == Some(key))
    };
    let notes: Vec<String> = [
        missing("title").then(|| fl!("missing-entry-title-note")),
        missing("regex").then(|| fl!("missing-entry-regex-note")),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!notes.is_empty()).then(|| Feedback::Note(notes.join(" ")))
}

pub(super) fn banners(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    page: Page,
) -> Vec<Banner> {
    bind_text!(fl, text);
    let mut banners = Vec::new();
    if let Some(error) = model.error {
        banners.push(Banner {
            id: automation_id::BANNER_CONFIGURATION_ERROR,
            severity: InfoBarSeverity::Error,
            title: fl!("banner-error-title"),
            message: fl!(
                "banner-error-message",
                file = problem(text, error.file),
                backup = problem(text, error.backup)
            ),
            actions: vec![
                (Action::Reset, fl!("action-reset")),
                (Action::OpenFolder, fl!("action-open-folder")),
            ],
            dismiss: None,
            enabled: true,
        });
    }
    banners.extend(control_unavailable(model, text));
    let mut native = native(model, text);
    if model.management.is_on() && output_failed(model) {
        banners.extend(native.take());
    }
    if model.paused {
        banners.push(Banner {
            id: automation_id::BANNER_PAUSED,
            severity: InfoBarSeverity::Error,
            title: fl!("banner-paused-title"),
            message: fl!("banner-paused-message"),
            actions: Vec::new(),
            dismiss: None,
            enabled: true,
        });
    }
    if let Some(failure) = model.write_error {
        banners.push(Banner {
            id: automation_id::BANNER_WRITE_FAILED,
            severity: InfoBarSeverity::Error,
            title: fl!("banner-write-title"),
            message: with_code(text, fl!("banner-write-message"), failure),
            actions: vec![(Action::Retry, fl!("action-retry"))],
            dismiss: None,
            enabled: true,
        });
    }
    if let Some(problem_kind) = model.restore {
        banners.push(Banner {
            id: automation_id::BANNER_RESTORED,
            severity: InfoBarSeverity::Warning,
            title: fl!("banner-restored-title"),
            message: fl!(
                "banner-restored-message",
                problem = problem(text, problem_kind)
            ),
            actions: Vec::new(),
            dismiss: Some(super::Command::DismissRestore),
            enabled: true,
        });
    }
    banners.extend(native);
    if !model.report.unknown.is_empty() {
        banners.push(Banner {
            id: automation_id::BANNER_UNKNOWN_KEYS,
            severity: InfoBarSeverity::Warning,
            title: fl!("banner-unknown-title"),
            message: fl!(
                "banner-unknown-message",
                keys = model.report.unknown.join(", ")
            ),
            actions: Vec::new(),
            dismiss: None,
            enabled: true,
        });
    }
    if let Some(failure) = model.backup_error {
        banners.push(Banner {
            id: automation_id::BANNER_BACKUP_FAILED,
            severity: InfoBarSeverity::Error,
            title: fl!("banner-backup-title"),
            message: with_code(text, fl!("banner-backup-message"), failure),
            actions: vec![(Action::Retry, fl!("action-retry"))],
            dismiss: None,
            enabled: true,
        });
    }
    banners.extend(crash(model, text));
    banners.extend(other_pages(model, text, region, page));
    banners
}

fn crash(model: &Model, text: &Text) -> Option<Banner> {
    bind_text!(fl, text);
    model.crash.as_ref().map(|_| Banner {
        id: automation_id::BANNER_CRASH,
        severity: InfoBarSeverity::Informational,
        title: fl!("banner-crash-title"),
        message: fl!("banner-crash-message"),
        actions: vec![
            (Action::OpenLogs, fl!("action-open-folder")),
            (Action::Report, fl!("action-report")),
        ],
        dismiss: Some(super::Command::DismissCrash),
        enabled: true,
    })
}

fn control_unavailable(model: &Model, text: &Text) -> Option<Banner> {
    bind_text!(fl, text);
    if !model.management.is_on() {
        return None;
    }
    let (severity, title, message) = match model.scheduling {
        SchedulingState::Waiting
        | SchedulingState::Evaluating
        | SchedulingState::Available
        | SchedulingState::SolarFallback => return None,
        SchedulingState::Stopped => (
            InfoBarSeverity::Error,
            fl!("banner-schedule-title"),
            fl!("banner-schedule-message"),
        ),
        SchedulingState::Retained => (
            InfoBarSeverity::Warning,
            fl!("banner-schedule-retained-title"),
            fl!("banner-schedule-retained-message"),
        ),
    };
    Some(Banner {
        id: automation_id::BANNER_SCHEDULE,
        severity,
        title,
        message,
        actions: vec![(Action::RetryNative, fl!("action-retry"))],
        dismiss: None,
        enabled: !model.retrying,
    })
}

pub(super) fn output_failed(model: &Model) -> bool {
    model
        .native_errors
        .iter()
        .any(|(operation, _)| *operation != DiagnosticOperation::Observation)
}

fn native(model: &Model, text: &Text) -> Option<Banner> {
    bind_text!(fl, text);
    if model.native_errors.is_empty() {
        return None;
    }
    let managing = model.management.is_on();
    let output = output_failed(model);
    let title = match (managing, output) {
        (false, _) => fl!("banner-native-off-title"),
        (true, true) => fl!("banner-native-output-title"),
        (true, false) => fl!("banner-native-observation-title"),
    };
    let mut lines: Vec<String> = model
        .native_errors
        .iter()
        .map(|(operation, failure)| {
            with_code(
                text,
                fl!(
                    "native-failure",
                    operation = fl!("native-operation", operation = operation.to_string()),
                    category = fl!("native-category", category = failure.category.to_string())
                ),
                *failure,
            )
        })
        .collect();
    if !managing {
        lines.push(fl!("banner-native-retry-unavailable"));
    }
    Some(Banner {
        id: automation_id::BANNER_NATIVE,
        severity: InfoBarSeverity::Error,
        title,
        message: lines.join("\n"),
        actions: vec![(Action::RetryNative, fl!("action-retry"))],
        dismiss: None,
        enabled: managing && !model.retrying,
    })
}

fn other_pages(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    current: Page,
) -> Option<Banner> {
    bind_text!(fl, text);
    let problems: Vec<(Field, &FieldErrorKind)> = model
        .errors
        .iter()
        .filter(|(field, _)| page(**field) != current)
        .map(|(field, kind)| (*field, kind))
        .collect();
    if problems.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = problems
        .iter()
        .map(|(field, kind)| error_message(text, region, *field, kind))
        .collect();
    lines.push(fl!(
        "banner-invalid-note",
        saving = if model.paused { "paused" } else { "automatic" }
    ));
    let pages: BTreeSet<Page> = problems.iter().map(|(field, _)| page(*field)).collect();
    Some(Banner {
        id: automation_id::BANNER_INVALID_FIELDS,
        severity: InfoBarSeverity::Error,
        title: fl!("banner-invalid-title", count = problems.len()),
        message: lines.join("\n"),
        actions: pages
            .into_iter()
            .map(|page| {
                (
                    Action::GoTo(page),
                    fl!("action-go-to", page = page.label(text)),
                )
            })
            .collect(),
        dismiss: None,
        enabled: true,
    })
}

fn problem(text: &Text, problem: FileProblem) -> String {
    bind_text!(fl, text);
    match problem {
        FileProblem::Missing => fl!("problem-missing"),
        FileProblem::Unreadable { code: Some(code) } => {
            fl!("problem-unreadable-code", code = hex(code))
        }
        FileProblem::Unreadable { code: None } => fl!("problem-unreadable"),
        FileProblem::Unusable(Unusable::Syntax { line: Some(line) }) => {
            fl!("problem-syntax-line", line = line)
        }
        FileProblem::Unusable(Unusable::Syntax { line: None }) => fl!("problem-syntax"),
        FileProblem::Unusable(Unusable::Version) => fl!("problem-version"),
    }
}

pub(super) fn with_code(text: &Text, message: String, failure: DiagnosticFailure) -> String {
    bind_text!(fl, text);
    match failure.code {
        Some(code) => format!("{message} {}", fl!("error-code", code = hex(code))),
        None => message,
    }
}

fn hex(code: i32) -> String {
    format!("0x{code:08X}")
}

fn name(text: &Text, field: Field) -> String {
    bind_text!(fl, text);
    fl!("field-name", field = field.key())
}

pub(super) fn kelvin(region: &RegionalFormat, kelvin: i64) -> String {
    u16::try_from(kelvin)
        .ok()
        .and_then(|kelvin| region.format_kelvin(kelvin).ok())
        .unwrap_or_else(|| format!("{kelvin} K"))
}

pub(super) fn clock(region: &RegionalFormat, minute: u16) -> String {
    region
        .format_clock(minute)
        .unwrap_or_else(|_| format!("{:02}:{:02}", minute / 60, minute % 60))
}

#[expect(
    clippy::cast_precision_loss,
    reason = "configuration numbers are at most 10000"
)]
pub(super) fn whole(region: &RegionalFormat, number: i64) -> String {
    region
        .format_decimal(number as f64, 0)
        .unwrap_or_else(|_| number.to_string())
}

fn value(text: &Text, region: &RegionalFormat, field: Field, value: &FieldValue) -> String {
    bind_text!(fl, text);
    match (field, value) {
        (_, FieldValue::Clock(minute)) => clock(region, *minute),
        (
            Field::DaytimeKelvin | Field::MiddleKelvin | Field::SleepKelvin,
            FieldValue::Number(n),
        ) => kelvin(region, *n),
        (_, FieldValue::Number(n)) => {
            fl!(
                "value-number",
                value = whole(region, *n),
                unit = field.key()
            )
        }
        (_, FieldValue::Switch(true)) => fl!("value-on"),
        (_, FieldValue::Switch(false)) => fl!("value-off"),
        (_, FieldValue::Coordinates(Some(coordinates))) => {
            let degrees = |degrees: f64| {
                region
                    .format_decimal(degrees, 4)
                    .unwrap_or_else(|_| degrees.to_string())
            };
            format!(
                "{}, {}",
                degrees(coordinates.latitude),
                degrees(coordinates.longitude)
            )
        }
        (_, FieldValue::Coordinates(None)) => fl!("value-none"),
        (_, FieldValue::Apps(entries)) => fl!("value-apps", count = entries.len()),
        (_, FieldValue::Language(language)) => super::general::language_label(text, language),
    }
}

fn using(
    text: &Text,
    region: &RegionalFormat,
    field: Field,
    source: Source,
    in_use: &FieldValue,
) -> String {
    bind_text!(fl, text);
    let in_use = value(text, region, field, in_use);
    match source {
        Source::Backup => fl!("using-backup", value = in_use),
        Source::Default => fl!("using-default", value = in_use),
    }
}

fn error_message(
    text: &Text,
    region: &RegionalFormat,
    field: Field,
    kind: &FieldErrorKind,
) -> String {
    bind_text!(fl, text);
    let field_name = name(text, field);
    match kind {
        FieldErrorKind::SameClock { other } => fl!(
            "error-same-clock",
            field = field_name,
            other = name(text, *other)
        ),
        FieldErrorKind::LowerThan {
            kelvin: value,
            other,
            other_kelvin,
        } => fl!(
            "error-lower",
            field = field_name,
            value = kelvin(region, *value),
            other = name(text, *other),
            other_value = kelvin(region, *other_kelvin)
        ),
        FieldErrorKind::HigherThan {
            kelvin: value,
            other,
            other_kelvin,
        } => fl!(
            "error-higher",
            field = field_name,
            value = kelvin(region, *value),
            other = name(text, *other),
            other_value = kelvin(region, *other_kelvin)
        ),
        FieldErrorKind::OutOfRange { minimum, maximum } => fl!(
            "error-range",
            field = field_name,
            minimum = value(text, region, field, &FieldValue::Number(*minimum)),
            maximum = value(text, region, field, &FieldValue::Number(*maximum))
        ),
        FieldErrorKind::CoordinatesRequired => {
            fl!("error-coordinates-required", field = field_name)
        }
        FieldErrorKind::Coordinates => fl!("error-coordinates"),
        FieldErrorKind::WrongType
        | FieldErrorKind::Clock
        | FieldErrorKind::Executable
        | FieldErrorKind::AlreadyListed
        | FieldErrorKind::FixedExclusion
        | FieldErrorKind::Language
        | FieldErrorKind::Inconsistent => fl!("error-invalid", field = field_name),
    }
}

fn mark_message(
    text: &Text,
    region: &RegionalFormat,
    field: Field,
    mark: &Mark,
    in_use: &FieldValue,
) -> String {
    bind_text!(fl, text);
    let field_name = name(text, field);
    let problem = match &mark.kind {
        FieldErrorKind::SameClock { other } => fl!(
            "mark-same-clock",
            field = field_name,
            file = mark.text.as_str(),
            other = name(text, *other)
        ),
        FieldErrorKind::LowerThan {
            kelvin: value,
            other,
            ..
        } => fl!(
            "mark-lower",
            field = field_name,
            file = kelvin(region, *value),
            other = name(text, *other)
        ),
        FieldErrorKind::HigherThan {
            kelvin: value,
            other,
            ..
        } => fl!(
            "mark-higher",
            field = field_name,
            file = kelvin(region, *value),
            other = name(text, *other)
        ),
        FieldErrorKind::CoordinatesRequired => {
            fl!("mark-coordinates-required", field = field_name)
        }
        FieldErrorKind::Inconsistent => fl!(
            "mark-inconsistent",
            field = field_name,
            file = mark.text.as_str()
        ),
        FieldErrorKind::WrongType
        | FieldErrorKind::Clock
        | FieldErrorKind::OutOfRange { .. }
        | FieldErrorKind::Coordinates
        | FieldErrorKind::Executable
        | FieldErrorKind::AlreadyListed
        | FieldErrorKind::FixedExclusion
        | FieldErrorKind::Language => fl!(
            "mark-invalid",
            field = field_name,
            file = mark.text.as_str()
        ),
    };
    format!(
        "{problem} {}",
        using(text, region, field, mark.source, in_use)
    )
}

#[cfg(test)]
mod tests;
