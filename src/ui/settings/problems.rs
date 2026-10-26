use super::Model;
use super::Page;
use crate::DiagnosticFailure;
use crate::DiagnosticOperation;
use crate::Field;
use crate::FieldErrorKind;
use crate::FieldValue;
use crate::Mark;
use crate::Source;
use crate::Unusable;
use crate::l10n::Text;
use crate::l10n::text;
use crate::platform::nls::RegionalFormat;
use crate::ui::configuration::FileProblem;
use std::collections::BTreeSet;
use windows_reactor::InfoBarSeverity;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui) enum Action {
    Reset,
    OpenFolder,
    Retry,
    RetryNative,
    GoTo(Page),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Banner {
    pub(super) id: &'static str,
    pub(super) severity: InfoBarSeverity,
    pub(super) title: String,
    pub(super) message: String,
    pub(super) actions: Vec<(Action, String)>,
    pub(super) closable: bool,
    pub(super) enabled: bool,
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
        | Field::Sunrise
        | Field::Sunset
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
    let in_use = model.config().value(field);
    if let Some(kind) = model.errors.get(&field) {
        return Some(Feedback::Error(error_message(text, region, field, kind)));
    }
    if let Some(mark) = model.report.marks.get(&field) {
        return Some(Feedback::Error(mark_message(
            text, region, field, mark, &in_use,
        )));
    }
    model.report.missing.get(&field).map(|source| {
        Feedback::Note(format!(
            "{} {}",
            text!(text, "missing-note"),
            using(text, region, field, *source, &in_use)
        ))
    })
}

pub(super) fn banners(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    page: Page,
) -> Vec<Banner> {
    let mut banners = Vec::new();
    if let Some(error) = model.error {
        banners.push(Banner {
            id: "banner-configuration-error",
            severity: InfoBarSeverity::Error,
            title: text!(text, "banner-error-title"),
            message: text!(
                text,
                "banner-error-message",
                file = problem(text, error.file),
                backup = problem(text, error.backup)
            ),
            actions: vec![
                (Action::Reset, text!(text, "action-reset")),
                (Action::OpenFolder, text!(text, "action-open-folder")),
            ],
            closable: false,
            enabled: true,
        });
    }
    let mut native = native(model, text);
    if model.management.is_on() && output_failed(model) {
        banners.extend(native.take());
    }
    if model.paused {
        banners.push(Banner {
            id: "banner-paused",
            severity: InfoBarSeverity::Error,
            title: text!(text, "banner-paused-title"),
            message: text!(text, "banner-paused-message"),
            actions: Vec::new(),
            closable: false,
            enabled: true,
        });
    }
    if let Some(failure) = model.write_error {
        banners.push(Banner {
            id: "banner-write-failed",
            severity: InfoBarSeverity::Error,
            title: text!(text, "banner-write-title"),
            message: with_code(text, text!(text, "banner-write-message"), failure),
            actions: vec![(Action::Retry, text!(text, "action-retry"))],
            closable: false,
            enabled: true,
        });
    }
    if let Some(problem_kind) = model.restore {
        banners.push(Banner {
            id: "banner-restored",
            severity: InfoBarSeverity::Warning,
            title: text!(text, "banner-restored-title"),
            message: text!(
                text,
                "banner-restored-message",
                problem = problem(text, problem_kind)
            ),
            actions: Vec::new(),
            closable: true,
            enabled: true,
        });
    }
    banners.extend(native);
    if !model.report.unknown.is_empty() {
        banners.push(Banner {
            id: "banner-unknown-keys",
            severity: InfoBarSeverity::Warning,
            title: text!(text, "banner-unknown-title"),
            message: text!(
                text,
                "banner-unknown-message",
                keys = model.report.unknown.join(", ")
            ),
            actions: Vec::new(),
            closable: false,
            enabled: true,
        });
    }
    if let Some(failure) = model.backup_error {
        banners.push(Banner {
            id: "banner-backup-failed",
            severity: InfoBarSeverity::Error,
            title: text!(text, "banner-backup-title"),
            message: with_code(text, text!(text, "banner-backup-message"), failure),
            actions: vec![(Action::Retry, text!(text, "action-retry"))],
            closable: false,
            enabled: true,
        });
    }
    banners.extend(other_pages(model, text, region, page));
    banners
}

fn output_failed(model: &Model) -> bool {
    model
        .native_errors
        .iter()
        .any(|(operation, _)| *operation != DiagnosticOperation::Observation)
}

fn native(model: &Model, text: &Text) -> Option<Banner> {
    if model.native_errors.is_empty() {
        return None;
    }
    let managing = model.management.is_on();
    let output = output_failed(model);
    let title = match (managing, output) {
        (false, _) => text!(text, "banner-native-off-title"),
        (true, true) => text!(text, "banner-native-output-title"),
        (true, false) => text!(text, "banner-native-observation-title"),
    };
    let mut lines: Vec<String> = model
        .native_errors
        .iter()
        .map(|(operation, failure)| {
            with_code(
                text,
                text!(
                    text,
                    "native-failure",
                    operation = text!(text, "native-operation", operation = operation.to_string()),
                    category = text!(
                        text,
                        "native-category",
                        category = failure.category.to_string()
                    )
                ),
                *failure,
            )
        })
        .collect();
    if !managing {
        lines.push(text!(text, "banner-native-retry-unavailable"));
    }
    Some(Banner {
        id: "banner-native",
        severity: InfoBarSeverity::Error,
        title,
        message: lines.join("\n"),
        actions: vec![(Action::RetryNative, text!(text, "action-retry"))],
        closable: false,
        enabled: managing && !model.retrying,
    })
}

fn other_pages(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    current: Page,
) -> Option<Banner> {
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
    lines.push(text!(
        text,
        "banner-invalid-note",
        saving = if model.paused { "paused" } else { "automatic" }
    ));
    let pages: BTreeSet<Page> = problems.iter().map(|(field, _)| page(*field)).collect();
    Some(Banner {
        id: "banner-invalid-fields",
        severity: InfoBarSeverity::Error,
        title: text!(text, "banner-invalid-title", count = problems.len()),
        message: lines.join("\n"),
        actions: pages
            .into_iter()
            .map(|page| {
                (
                    Action::GoTo(page),
                    text!(text, "action-go-to", page = page.label(text)),
                )
            })
            .collect(),
        closable: false,
        enabled: true,
    })
}

fn problem(text: &Text, problem: FileProblem) -> String {
    match problem {
        FileProblem::Missing => text!(text, "problem-missing"),
        FileProblem::Unreadable { code: Some(code) } => {
            text!(text, "problem-unreadable-code", code = hex(code))
        }
        FileProblem::Unreadable { code: None } => text!(text, "problem-unreadable"),
        FileProblem::Unusable(Unusable::Syntax { line: Some(line) }) => {
            text!(text, "problem-syntax-line", line = line)
        }
        FileProblem::Unusable(Unusable::Syntax { line: None }) => text!(text, "problem-syntax"),
        FileProblem::Unusable(Unusable::Version) => text!(text, "problem-version"),
    }
}

pub(super) fn with_code(text: &Text, message: String, failure: DiagnosticFailure) -> String {
    match failure.code {
        Some(code) => format!("{message} {}", text!(text, "error-code", code = hex(code))),
        None => message,
    }
}

fn hex(code: i32) -> String {
    format!("0x{code:08X}")
}

fn name(text: &Text, field: Field) -> String {
    text!(text, "field-name", field = field.key())
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
    match (field, value) {
        (_, FieldValue::Clock(minute)) => clock(region, *minute),
        (
            Field::DaytimeKelvin | Field::MiddleKelvin | Field::SleepKelvin,
            FieldValue::Number(n),
        ) => kelvin(region, *n),
        (_, FieldValue::Number(n)) => {
            text!(
                text,
                "value-number",
                value = whole(region, *n),
                unit = field.key()
            )
        }
        (_, FieldValue::Switch(true)) => text!(text, "value-on"),
        (_, FieldValue::Switch(false)) => text!(text, "value-off"),
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
        (_, FieldValue::Coordinates(None)) => text!(text, "value-none"),
        (_, FieldValue::Apps(entries)) => text!(text, "value-apps", count = entries.len()),
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
    let in_use = value(text, region, field, in_use);
    match source {
        Source::Backup => text!(text, "using-backup", value = in_use),
        Source::Default => text!(text, "using-default", value = in_use),
    }
}

fn error_message(
    text: &Text,
    region: &RegionalFormat,
    field: Field,
    kind: &FieldErrorKind,
) -> String {
    let field_name = name(text, field);
    match kind {
        FieldErrorKind::SameClock { other } => text!(
            text,
            "error-same-clock",
            field = field_name,
            other = name(text, *other)
        ),
        FieldErrorKind::LowerThan {
            kelvin: value,
            other,
            other_kelvin,
        } => text!(
            text,
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
        } => text!(
            text,
            "error-higher",
            field = field_name,
            value = kelvin(region, *value),
            other = name(text, *other),
            other_value = kelvin(region, *other_kelvin)
        ),
        FieldErrorKind::OutOfRange { minimum, maximum } => text!(
            text,
            "error-range",
            field = field_name,
            minimum = value(text, region, field, &FieldValue::Number(*minimum)),
            maximum = value(text, region, field, &FieldValue::Number(*maximum))
        ),
        FieldErrorKind::CoordinatesRequired => {
            text!(text, "error-coordinates-required", field = field_name)
        }
        FieldErrorKind::WrongType
        | FieldErrorKind::Clock
        | FieldErrorKind::Coordinates
        | FieldErrorKind::Executable
        | FieldErrorKind::AlreadyListed
        | FieldErrorKind::FixedExclusion
        | FieldErrorKind::Language
        | FieldErrorKind::Inconsistent => text!(text, "error-invalid", field = field_name),
    }
}

fn mark_message(
    text: &Text,
    region: &RegionalFormat,
    field: Field,
    mark: &Mark,
    in_use: &FieldValue,
) -> String {
    let field_name = name(text, field);
    let problem = match &mark.kind {
        FieldErrorKind::SameClock { other } => text!(
            text,
            "mark-same-clock",
            field = field_name,
            file = mark.text.as_str(),
            other = name(text, *other)
        ),
        FieldErrorKind::LowerThan {
            kelvin: value,
            other,
            ..
        } => text!(
            text,
            "mark-lower",
            field = field_name,
            file = kelvin(region, *value),
            other = name(text, *other)
        ),
        FieldErrorKind::HigherThan {
            kelvin: value,
            other,
            ..
        } => text!(
            text,
            "mark-higher",
            field = field_name,
            file = kelvin(region, *value),
            other = name(text, *other)
        ),
        FieldErrorKind::CoordinatesRequired => {
            text!(text, "mark-coordinates-required", field = field_name)
        }
        FieldErrorKind::Inconsistent => text!(
            text,
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
        | FieldErrorKind::Language => text!(
            text,
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
