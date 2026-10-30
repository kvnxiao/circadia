use super::Command;
use super::Message;
use super::Model;
use super::NumberControls;
use super::Page;
use super::Settings;
use super::Visibility;
use super::problems;
use super::widgets;
use super::widgets::APP_GLYPH;
use super::widgets::AppIcon;
use super::widgets::Expansion;
use super::widgets::Row;
use crate::AppEntry;
use crate::EntryProblem;
use crate::Field;
use crate::FieldErrorKind;
use crate::ManualCommand;
use crate::automation_id;
use crate::l10n::Text;
use crate::l10n::text;
use crate::platform::nls::RegionalFormat;
use crate::ui::file_info::AppFile;
use crate::ui::icon_cache::AppIcons;
use crate::ui::status;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::ButtonStyle;
use windows_reactor::CheckBox;
use windows_reactor::ContentDialog;
use windows_reactor::ContentDialogButton;
use windows_reactor::ContentDialogExt;
use windows_reactor::FontIcon;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::HyperlinkButton;
use windows_reactor::Orientation;
use windows_reactor::StackPanel;
use windows_reactor::TextBox;
use windows_reactor::ThemeBrush;
use windows_reactor::Thickness;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;

const SUN_GLYPH: &str = "\u{E706}";
const ADD_GLYPH: &str = "\u{E710}";
pub(super) const MINUTE: Duration = Duration::from_mins(1);

pub(super) fn view(settings: &Settings, context: &ViewContext<'_, Settings>) -> Vec<View> {
    let model = &settings.input.model;
    let (text, region) = (&settings.input.text, &settings.input.region);
    let manual = manual_view(model, text, region);
    let mut children = vec![
        widgets::group(vec![
            widgets::card(
                Row::new(
                    "\u{E7F4}",
                    text!(text, "apps-explanation-title"),
                    text!(
                        text,
                        "apps-explanation-description",
                        kelvin = problems::kelvin(region, 6500)
                    ),
                )
                .view(),
            ),
            widgets::card(
                Row::new(
                    SUN_GLYPH,
                    text!(text, "manual-title"),
                    manual.description.clone(),
                )
                .controls(vec![manual_button(manual, context)])
                .view(),
            ),
        ]),
        widgets::section(text!(text, "section-recent"), false),
        recent_card(settings, context),
        widgets::caption(text!(text, "recent-note"))
            .margin(Thickness::new(0.0, 8.0, 0.0, 0.0))
            .into(),
        list_header(settings, context),
    ];
    children.extend(settings.feedback(Field::Suppression));
    children.extend(suppression_cards(settings, context));
    children.push(widgets::section(text!(text, "section-never"), false));
    children.push(exclusions_card(settings, context));
    children.push(widgets::section(text!(text, "section-timing"), false));
    let delay = text!(text, "focus-delay-title");
    children.push(widgets::card(
        Row::new("\u{E916}", &delay, text!(text, "focus-delay-description"))
            .controls(settings.with_feedback(
                Field::FocusDelayMs,
                settings.number_controls(
                    NumberControls {
                        field: Field::FocusDelayMs,
                        number: automation_id::FOCUS_DELAY_NUMBER,
                        slider: None,
                        title: &delay,
                        unit: text!(text, "milliseconds-unit"),
                    },
                    context,
                ),
            ))
            .view(),
    ));
    vec![
        Border::new()
            .content(
                StackPanel::new()
                    .children(children)
                    .content_dialog(add_dialog(settings, context)),
            )
            .content_dialog(title_dialog(settings, context)),
    ]
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ManualView {
    pub(super) command: ManualCommand,
    pub(super) label: String,
    pub(super) enabled: bool,
    pub(super) description: String,
}

pub(super) fn manual_view(model: &Model, text: &Text, region: &RegionalFormat) -> ManualView {
    let enabled = status::manual_available(model.management, model.scheduling, model.suppression);
    let description = if model.suppression.manual() {
        model.manual_end.map_or_else(
            || text!(text, "manual-active"),
            |minute| text!(text, "manual-until", time = problems::clock(region, minute)),
        )
    } else if model.accepted.is_some() && !model.management.is_on() {
        text!(text, "manual-unmanaged")
    } else {
        text!(
            text,
            "manual-description",
            kelvin = problems::kelvin(region, 6500)
        )
    };
    let (command, label) = if model.suppression.manual() {
        (ManualCommand::Stop, text!(text, "manual-stop"))
    } else {
        (ManualCommand::Start, text!(text, "manual-title"))
    };
    ManualView {
        command,
        label,
        enabled,
        description,
    }
}

pub(super) fn manual_button(manual: ManualView, context: &ViewContext<'_, Settings>) -> View {
    Button::new()
        .is_enabled(manual.enabled)
        .automation_id(automation_id::MANUAL_SUPPRESSION)
        .automation_name(&manual.label)
        .on_click(context.message(Message::Command(Command::Manual(manual.command))))
        .content(widgets::glyph_label(SUN_GLYPH, manual.label))
        .into()
}

pub(super) fn relative_times_shown(page: Page, model: &Model, visibility: Visibility) -> bool {
    visibility == Visibility::Shown && page == Page::Apps && !model.recent.is_empty()
}

pub(super) fn since(text: &Text, region: &RegionalFormat, elapsed: Duration) -> String {
    let minutes = elapsed.as_secs() / MINUTE.as_secs();
    let whole = |value: u64| problems::whole(region, i64::try_from(value).unwrap_or(i64::MAX));
    match minutes {
        0 => text!(text, "recent-now"),
        1..60 => text!(text, "recent-minutes", minutes = whole(minutes)),
        60..1440 => text!(text, "recent-hours", hours = whole(minutes / 60)),
        _ => {
            let days = minutes / 1440;
            text!(text, "recent-days", count = days, days = whole(days))
        }
    }
}

pub(super) fn icon_pixels(dpi: u32) -> u16 {
    let dpi = if dpi == 0 { 96 } else { dpi };
    u16::try_from((32 * dpi).div_ceil(96)).unwrap_or(u16::MAX)
}

pub(super) fn icon_for(
    icons: &AppIcons,
    running_executable: &str,
    executable: &str,
    pixels: u16,
) -> AppIcon {
    if executable.eq_ignore_ascii_case(running_executable) {
        return AppIcon::Circadia;
    }
    icons
        .apps
        .get(&executable.to_lowercase())
        .or(icons.generic.as_ref())
        .map_or(AppIcon::Placeholder, |images| {
            AppIcon::Image(Arc::clone(images.at(pixels)))
        })
}

fn icon(settings: &Settings, executable: &str, size: f64) -> View {
    icon_for(
        &settings.input.icons,
        &settings.input.model.running_executable,
        executable,
        icon_pixels(settings.dpi),
    )
    .view(size)
}

fn shown_apps(page: Page, dialog: Option<&AddDialog>, model: &Model) -> BTreeSet<String> {
    if page != Page::Apps {
        return BTreeSet::new();
    }
    let config = model.config();
    let listed: Vec<String> = [Field::Suppression, Field::Exclusions]
        .into_iter()
        .flat_map(|list| config.entries(list))
        .filter_map(AppEntry::file_name)
        .collect();
    let running = model.running_executable.to_lowercase();
    model
        .recent
        .iter()
        .map(|recent| recent.executable.clone())
        .chain(listed)
        .chain(
            dialog
                .into_iter()
                .flat_map(|dialog| dialog.candidates(model))
                .map(|file| file.executable),
        )
        .filter(|executable| *executable != running)
        .collect()
}

#[derive(Debug, Default)]
pub(super) struct IconRequests {
    view: Option<(Page, Option<Field>)>,
    requested: BTreeSet<String>,
}

impl IconRequests {
    pub(super) fn due(
        &mut self,
        page: Page,
        dialog: Option<&AddDialog>,
        model: &Model,
    ) -> Vec<String> {
        let view = (page, dialog.map(|dialog| dialog.list));
        if self.view != Some(view) {
            self.view = Some(view);
            self.requested.clear();
        }
        let due: Vec<String> = shown_apps(page, dialog, model)
            .into_iter()
            .filter(|executable| !self.requested.contains(executable))
            .collect();
        self.requested.extend(due.iter().cloned());
        due
    }
}

pub(super) fn display_name(model: &Model, executable: &str) -> String {
    model
        .names
        .get(executable)
        .cloned()
        .unwrap_or_else(|| executable.to_owned())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RecentRow {
    pub(super) file: AppFile,
    pub(super) name: String,
    pub(super) detail: String,
    pub(super) listed: Option<usize>,
    pub(super) excluded: bool,
}

pub(super) fn recent_rows(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    now: Duration,
) -> Vec<RecentRow> {
    let config = model.config();
    model
        .recent
        .iter()
        .map(|recent| RecentRow {
            file: AppFile {
                executable: recent.executable.clone(),
                path: recent.path.clone(),
            },
            name: display_name(model, &recent.executable),
            detail: text!(
                text,
                "recent-detail",
                executable = recent.executable.as_str(),
                since = since(text, region, recent.since_focus(now))
            ),
            listed: config
                .entries(Field::Suppression)
                .iter()
                .position(|entry| entry.file_name().as_deref() == Some(recent.executable.as_str())),
            excluded: config
                .classify(&recent.executable, None, &model.running_executable)
                .excluded,
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct EntryRow {
    pub(super) index: usize,
    pub(super) name: String,
    pub(super) executable: String,
    pub(super) detail: String,
    pub(super) inactive: bool,
    pub(super) error: Option<String>,
}

pub(super) fn entry_rows(model: &Model, text: &Text, list: Field) -> Vec<EntryRow> {
    let config = model.config();
    config
        .entries(list)
        .iter()
        .zip(config.entry_problems(list))
        .enumerate()
        .map(|(index, (entry, problem))| EntryRow {
            index,
            name: if entry.title.is_empty() {
                entry.display_name().to_owned()
            } else {
                entry.title.clone()
            },
            executable: entry.executable.clone(),
            detail: if entry.title.is_empty() {
                entry.executable.clone()
            } else {
                text!(
                    text,
                    "entry-title-detail",
                    regex = if entry.regex { "yes" } else { "no" },
                    executable = entry.executable.as_str()
                )
            },
            inactive: list == Field::Suppression
                && entry.file_name().is_some_and(|name| {
                    config
                        .classify(&name, None, &model.running_executable)
                        .excluded
                }),
            error: problem.map(|problem| match problem {
                EntryProblem::Invalid => text!(text, "entry-invalid"),
                EntryProblem::Expression => text!(text, "entry-expression"),
                EntryProblem::Duplicate => text!(
                    text,
                    "entry-duplicate",
                    executable = entry.executable.as_str()
                ),
            }),
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FixedRow {
    pub(super) executable: String,
    pub(super) description: String,
}

pub(super) fn fixed_rows(model: &Model, text: &Text) -> Vec<FixedRow> {
    vec![
        FixedRow {
            executable: model.running_executable.clone(),
            description: text!(text, "fixed-self"),
        },
        FixedRow {
            executable: crate::config::SHARED_HOST.to_owned(),
            description: text!(text, "fixed-host"),
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AddDialog {
    pub(super) list: Field,
    chosen: BTreeSet<String>,
    browsed: Vec<AppFile>,
    problem: Option<(String, FieldErrorKind)>,
}

impl AddDialog {
    pub(super) fn new(list: Field) -> Self {
        Self {
            list,
            chosen: BTreeSet::new(),
            browsed: Vec::new(),
            problem: None,
        }
    }

    pub(super) fn choose(&mut self, executable: &str, checked: bool) {
        if checked {
            self.chosen.insert(executable.to_owned());
        } else {
            self.chosen.remove(executable);
        }
    }

    pub(super) fn browsed(&mut self, model: &Model, file: AppFile) -> bool {
        let entry = AppEntry::new(file.executable.clone(), file.executable.clone());
        let added = model
            .config()
            .add_entries(self.list, [entry], &model.running_executable);
        if let Err(kind) = added {
            self.problem = Some((file.executable, kind));
            return false;
        }
        self.problem = None;
        self.chosen.insert(file.executable.clone());
        if !self
            .browsed
            .iter()
            .any(|browsed| browsed.executable == file.executable)
        {
            self.browsed.push(file);
        }
        true
    }

    pub(super) fn candidates(&self, model: &Model) -> Vec<AppFile> {
        let config = model.config();
        let entries = config.entries(self.list);
        let mut candidates: Vec<AppFile> = Vec::new();
        let recent = model.recent.iter().map(|recent| AppFile {
            executable: recent.executable.clone(),
            path: recent.path.clone(),
        });
        for file in recent.chain(self.browsed.iter().cloned()) {
            let listed = entries
                .iter()
                .any(|entry| entry.executable.to_lowercase() == file.executable);
            let shown = candidates
                .iter()
                .any(|candidate| candidate.executable == file.executable);
            if !listed && !shown {
                candidates.push(file);
            }
        }
        candidates
    }

    pub(super) fn files(&self, model: &Model) -> Vec<AppFile> {
        self.candidates(model)
            .into_iter()
            .filter(|file| self.chosen.contains(&file.executable))
            .collect()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DialogRow {
    pub(super) executable: String,
    pub(super) name: String,
    pub(super) checked: bool,
    pub(super) browsed: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DialogView {
    pub(super) title: String,
    pub(super) description: String,
    pub(super) rows: Vec<DialogRow>,
    pub(super) primary: String,
    pub(super) primary_enabled: bool,
    pub(super) message: Option<String>,
}

pub(super) fn dialog_view(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    dialog: &AddDialog,
) -> DialogView {
    let count = dialog.files(model).len();
    let apps = problems::whole(region, i64::try_from(count).unwrap_or(i64::MAX));
    let exclusion = dialog.list == Field::Exclusions;
    DialogView {
        title: if exclusion {
            text!(text, "dialog-exclude-title")
        } else {
            text!(text, "dialog-add-title")
        },
        description: if exclusion {
            text!(text, "dialog-exclude-description")
        } else {
            text!(text, "dialog-add-description")
        },
        rows: dialog
            .candidates(model)
            .into_iter()
            .map(|file| DialogRow {
                name: display_name(model, &file.executable),
                checked: dialog.chosen.contains(&file.executable),
                browsed: !model
                    .recent
                    .iter()
                    .any(|recent| recent.executable == file.executable),
                executable: file.executable,
            })
            .collect(),
        primary: if exclusion {
            text!(text, "dialog-exclude", count = count, apps = apps.as_str())
        } else {
            text!(text, "dialog-add", count = count, apps = apps.as_str())
        },
        primary_enabled: count > 0,
        message: dialog
            .problem
            .as_ref()
            .and_then(|(executable, kind)| match kind {
                FieldErrorKind::AlreadyListed => Some(text!(
                    text,
                    "dialog-already-listed",
                    executable = executable.as_str()
                )),
                FieldErrorKind::FixedExclusion => Some(text!(
                    text,
                    "dialog-fixed",
                    executable = executable.as_str()
                )),
                _ => None,
            }),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TitleDialog {
    list: Field,
    index: usize,
    executable: String,
    text: String,
    regex: bool,
}

impl TitleDialog {
    pub(super) fn open(model: &Model, list: Field, index: usize) -> Option<Self> {
        let config = model.config();
        let entry = config.entries(list).get(index)?;
        Some(Self {
            list,
            index,
            executable: entry.executable.clone(),
            text: entry.title.clone(),
            regex: entry.regex,
        })
    }

    pub(super) fn set_text(&mut self, text: &str) {
        text.clone_into(&mut self.text);
    }

    pub(super) fn set_regex(&mut self, regex: bool) {
        self.regex = regex;
    }

    pub(super) fn use_title(&mut self, model: &Model) {
        if let Some(title) = last_seen(model, &self.executable) {
            self.text = if self.regex {
                regex::escape(title)
            } else {
                title.to_owned()
            };
        }
    }

    pub(super) fn command(&self) -> Option<Command> {
        crate::valid_title(&self.text, self.regex).then(|| Command::Title {
            list: self.list,
            index: self.index,
            executable: self.executable.clone(),
            title: self.text.clone(),
            regex: self.regex,
        })
    }
}

fn last_seen<'a>(model: &'a Model, executable: &str) -> Option<&'a str> {
    model
        .recent
        .iter()
        .find(|recent| recent.executable == executable)
        .and_then(|recent| recent.title.as_deref())
        .filter(|title| !title.is_empty())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct TitleDialogView {
    pub(super) title: String,
    pub(super) description: String,
    pub(super) label: String,
    pub(super) text: String,
    pub(super) regex: bool,
    pub(super) suggestion: Option<String>,
    pub(super) error: Option<String>,
    pub(super) save_enabled: bool,
}

pub(super) fn title_dialog_view(
    model: &Model,
    text: &Text,
    dialog: &TitleDialog,
) -> TitleDialogView {
    let regex = if dialog.regex { "yes" } else { "no" };
    let valid = crate::valid_title(&dialog.text, dialog.regex);
    TitleDialogView {
        title: text!(text, "title-dialog-title"),
        description: text!(
            text,
            "title-dialog-description",
            regex = regex,
            executable = dialog.executable.as_str()
        ),
        label: text!(text, "title-dialog-field", regex = regex),
        text: dialog.text.clone(),
        regex: dialog.regex,
        suggestion: last_seen(model, &dialog.executable)
            .map(|title| text!(text, "title-dialog-last-seen", title = title)),
        error: (!valid).then(|| text!(text, "title-dialog-invalid")),
        save_enabled: valid,
    }
}

fn app_row(icon: View, name: &str, detail: Vec<View>, controls: Vec<View>, id: String) -> View {
    Border::new()
        .min_height(60.0)
        .padding(Thickness::new(16.0, 10.0, 16.0, 10.0))
        .automation_id(id)
        .content(
            Grid::new()
                .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                .column_spacing(16.0)
                .children((
                    icon,
                    StackPanel::new()
                        .grid_column(1)
                        .spacing(2.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(
                            std::iter::once(widgets::body(name).into())
                                .chain(detail)
                                .collect::<Vec<View>>(),
                        ),
                    widgets::action_group(controls).grid_column(2),
                )),
        )
        .into()
}

fn title_button(
    row: &EntryRow,
    list: Field,
    id: &str,
    settings: &Settings,
    context: &ViewContext<'_, Settings>,
) -> View {
    widgets::icon_button("\u{E71C}", 16.0)
        .is_enabled(settings.input.model.editable())
        .automation_id(automation_id::title(id))
        .automation_name(text!(
            settings.input.text,
            "entry-title-name",
            name = row.name.as_str()
        ))
        .on_click(context.message(Message::OpenTitle(list, row.index)))
        .into()
}

fn remove_button(
    name: &str,
    id: String,
    enabled: bool,
    remove: windows_reactor::Callback<()>,
) -> Button {
    widgets::icon_button("\u{E74D}", 16.0)
        .is_enabled(enabled)
        .automation_id(id)
        .automation_name(name)
        .on_click(remove)
}

fn recent_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let model = &settings.input.model;
    let (text, region) = (&settings.input.text, &settings.input.region);
    let editable = model.editable();
    let rows = recent_rows(model, text, region, crate::elapsed_since_boot());
    if rows.is_empty() {
        return widgets::card(
            Border::new()
                .padding(Thickness::new(16.0, 12.0, 16.0, 12.0))
                .content(widgets::caption(text!(text, "recent-empty"))),
        );
    }
    let views: Vec<View> = rows
        .into_iter()
        .enumerate()
        .map(|(position, row)| {
            let executable = row.file.executable.clone();
            let mut controls: Vec<View> = Vec::new();
            if row.excluded {
                controls.push(
                    widgets::caption(text!(text, "recent-excluded"))
                        .vertical_alignment(VerticalAlignment::Center)
                        .into(),
                );
            }
            if let Some(index) = row.listed {
                controls.push(
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(4.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children((
                            FontIcon::new().glyph("\u{E73E}").font_size(12.0),
                            widgets::caption(text!(text, "recent-on-list")),
                        ))
                        .into(),
                );
                controls.push(
                    Button::new()
                        .is_enabled(editable)
                        .automation_id(automation_id::recent_remove(&executable))
                        .automation_name(text!(
                            text,
                            "recent-remove-name",
                            name = row.name.as_str()
                        ))
                        .on_click(context.message(Message::Command(Command::Remove(
                            Field::Suppression,
                            index,
                            row.file.executable.clone(),
                        ))))
                        .content(text!(text, "recent-remove"))
                        .into(),
                );
            } else {
                controls.push(
                    Button::new()
                        .is_enabled(editable)
                        .automation_id(automation_id::recent_add(&executable))
                        .automation_name(text!(text, "recent-add-name", name = row.name.as_str()))
                        .on_click(context.message(Message::Command(Command::Add(
                            Field::Suppression,
                            vec![row.file.clone()],
                        ))))
                        .content(widgets::glyph_label(ADD_GLYPH, text!(text, "recent-add")))
                        .into(),
                );
            }
            let row_view = app_row(
                icon(settings, &row.file.executable, 32.0),
                &row.name,
                vec![widgets::caption(&row.detail).into()],
                controls,
                automation_id::recent(&executable),
            );
            divided(row_view, position > 0)
        })
        .collect();
    widgets::card(StackPanel::new().children(views))
}

fn divided(view: View, divider: bool) -> View {
    Border::new()
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(Thickness::new(
            0.0,
            if divider { 1.0 } else { 0.0 },
            0.0,
            0.0,
        ))
        .content(view)
        .into()
}

fn list_header(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let text = &settings.input.text;
    let count = settings
        .input
        .model
        .config()
        .entries(Field::Suppression)
        .len();
    Grid::new()
        .columns([GridLength::STAR, GridLength::Auto])
        .margin(Thickness::new(0.0, 28.0, 0.0, 8.0))
        .children((
            widgets::strong(text!(text, "section-suppression", count = count))
                .vertical_alignment(VerticalAlignment::Center),
            Button::new()
                .grid_column(1)
                .style(ButtonStyle::Accent)
                .is_enabled(settings.input.model.editable())
                .automation_id(automation_id::ADD_APP)
                .automation_name(text!(text, "add-app"))
                .on_click(context.message(Message::OpenDialog(Field::Suppression)))
                .content(widgets::glyph_label(ADD_GLYPH, text!(text, "add-app"))),
        ))
        .into()
}

fn entry_card(row: &EntryRow, settings: &Settings, controls: Vec<View>, id: String) -> View {
    let text = &settings.input.text;
    let mut detail: Vec<View> = vec![widgets::caption(&row.detail).into()];
    if row.inactive {
        detail.push(widgets::caption(text!(text, "entry-inactive")).into());
    }
    if let Some(error) = &row.error {
        detail.push(widgets::field_error(error, &automation_id::message(&id)));
    }
    let border = if row.error.is_some() {
        ThemeBrush::SystemCritical
    } else {
        ThemeBrush::CardStroke
    };
    Border::new()
        .background(ThemeBrush::CardBackground)
        .border_brush(border)
        .border_thickness(1.0)
        .corner_radius(4.0)
        .content(app_row(
            icon(settings, &row.executable, 32.0),
            &row.name,
            detail,
            controls,
            id,
        ))
        .into()
}

fn suppression_cards(settings: &Settings, context: &ViewContext<'_, Settings>) -> Vec<View> {
    let model = &settings.input.model;
    let text = &settings.input.text;
    let rows = entry_rows(model, text, Field::Suppression);
    if rows.is_empty() {
        return vec![widgets::card(
            Row::new(
                APP_GLYPH,
                text!(text, "list-empty-title"),
                text!(text, "list-empty-description"),
            )
            .view(),
        )];
    }
    let cards = rows
        .iter()
        .map(|row| {
            let id = automation_id::suppression_entry(row.index);
            let remove = remove_button(
                &text!(text, "entry-remove-name", name = row.name.as_str()),
                automation_id::remove(&id),
                model.editable(),
                context.message(Message::Command(Command::Remove(
                    Field::Suppression,
                    row.index,
                    row.executable.clone(),
                ))),
            );
            let title = title_button(row, Field::Suppression, &id, settings, context);
            entry_card(row, settings, vec![title, remove.into()], id)
        })
        .collect();
    vec![widgets::group(cards)]
}

fn exclusions_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let model = &settings.input.model;
    let text = &settings.input.text;
    let rows = entry_rows(model, text, Field::Exclusions);
    let held = rows.iter().any(|row| row.error.is_some())
        || model.report.marks.contains_key(&Field::Exclusions);
    let expansion = Expansion::chosen(settings.expanded.exclusions).held_when(held);
    let expanded = expansion.expanded();
    let expand_name = match expansion {
        Expansion::Held => text!(text, "exclusions-held"),
        Expansion::Expanded => text!(text, "exclusions-collapse"),
        Expansion::Collapsed => text!(text, "exclusions-expand"),
    };
    let header = Row::new(
        "\u{E733}",
        text!(text, "exclusions-title"),
        text!(text, "exclusions-description"),
    )
    .controls(vec![
        widgets::action_group(vec![
            Button::new()
                .is_enabled(model.editable())
                .automation_id(automation_id::ADD_EXCLUSION)
                .on_click(context.message(Message::OpenDialog(Field::Exclusions)))
                .content(text!(text, "add-exclusion"))
                .into(),
            widgets::expand_button(
                expansion,
                expand_name,
                automation_id::EXCLUSIONS_EXPAND,
                context.message(Message::Expand(!expanded)),
            ),
        ])
        .into(),
    ])
    .view();
    let mut children: Vec<View> = vec![header];
    children.extend(settings.feedback(Field::Exclusions));
    if expanded {
        children.extend(fixed_rows(model, text).into_iter().map(|row| {
            nested_app_row(
                icon(settings, &row.executable, 32.0),
                &row.executable,
                widgets::caption(&row.description).into(),
                vec![
                    Border::new()
                        .width(widgets::ICON_ACTION_SIZE)
                        .height(widgets::ICON_ACTION_SIZE)
                        .content(
                            FontIcon::new()
                                .glyph("\u{E72E}")
                                .font_size(16.0)
                                .horizontal_alignment(HorizontalAlignment::Center)
                                .vertical_alignment(VerticalAlignment::Center)
                                .automation_name(text!(text, "fixed-locked")),
                        )
                        .into(),
                ],
                &automation_id::fixed_exclusion(&row.executable.to_lowercase()),
            )
        }));
        children.extend(rows.iter().map(|row| {
            let id = automation_id::exclusion_entry(row.index);
            let mut detail: Vec<View> = vec![widgets::caption(&row.detail).into()];
            if let Some(error) = &row.error {
                detail.push(widgets::field_error(error, &automation_id::message(&id)));
            }
            let actions = vec![
                title_button(row, Field::Exclusions, &id, settings, context),
                remove_button(
                    &text!(text, "entry-remove-name", name = row.name.as_str()),
                    automation_id::remove(&id),
                    model.editable(),
                    context.message(Message::Command(Command::Remove(
                        Field::Exclusions,
                        row.index,
                        row.executable.clone(),
                    ))),
                )
                .into(),
            ];
            nested_app_row(
                icon(settings, &row.executable, 32.0),
                &row.name,
                StackPanel::new().spacing(2.0).children(detail).into(),
                actions,
                &id,
            )
        }));
        if rows.is_empty() {
            children.push(
                Border::new()
                    .padding(Thickness::new(52.0, 10.0, 16.0, 10.0))
                    .border_brush(ThemeBrush::CardStroke)
                    .border_thickness(Thickness::new(0.0, 1.0, 0.0, 0.0))
                    .content(widgets::caption(text!(text, "exclusions-empty")))
                    .into(),
            );
        }
    }
    widgets::card(StackPanel::new().children(children))
}

fn nested_app_row(icon: View, name: &str, detail: View, actions: Vec<View>, id: &str) -> View {
    widgets::nested(id)
        .content(
            Grid::new()
                .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                .column_spacing(12.0)
                .children((
                    icon,
                    StackPanel::new()
                        .grid_column(1)
                        .spacing(2.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children((widgets::body(name), detail)),
                    widgets::action_group(actions).grid_column(2),
                )),
        )
        .into()
}

fn add_dialog(settings: &Settings, context: &ViewContext<'_, Settings>) -> ContentDialog {
    let model = &settings.input.model;
    let (text, region) = (&settings.input.text, &settings.input.region);
    let dialog = ContentDialog::new()
        .close_button_text(text!(text, "dialog-cancel"))
        .default_button(ContentDialogButton::Primary)
        .is_open(settings.dialog.is_some())
        .on_closed(context.callback(Message::DialogClosed));
    let Some(state) = &settings.dialog else {
        return dialog;
    };
    let view = dialog_view(model, text, region, state);
    let choice = |row: DialogRow| -> View {
        let executable = row.executable.clone();
        CheckBox::new()
            .is_checked(row.checked)
            .automation_id(automation_id::dialog_choice(&row.executable))
            .automation_name(text!(
                text,
                "app-entry-name",
                name = row.name.as_str(),
                executable = row.executable.as_str()
            ))
            .on_is_checked_changed(context.callback(move |checked: Option<bool>| {
                Message::Choose(executable.clone(), checked == Some(true))
            }))
            .content(
                Grid::new()
                    .columns([GridLength::Auto, GridLength::STAR])
                    .column_spacing(14.0)
                    .children((
                        icon(settings, &row.executable, 28.0),
                        StackPanel::new().grid_column(1).children((
                            widgets::body(&row.name),
                            widgets::caption(&row.executable),
                        )),
                    )),
            )
            .into()
    };
    let (browsed, recent): (Vec<DialogRow>, Vec<DialogRow>) =
        view.rows.into_iter().partition(|row| row.browsed);
    let rows: Vec<View> = if recent.is_empty() {
        vec![widgets::caption(text!(text, "dialog-all-listed")).into()]
    } else {
        recent.into_iter().map(choice).collect()
    };
    let mut browse: Vec<View> = vec![
        widgets::body(text!(text, "dialog-file-title")).into(),
        widgets::caption(text!(text, "dialog-file-description")).into(),
        Button::new()
            .margin(Thickness::new(0.0, 8.0, 0.0, 0.0))
            .automation_id(automation_id::DIALOG_BROWSE)
            .automation_name(text!(text, "dialog-browse"))
            .on_click(context.message(Message::Browse))
            .content(widgets::glyph_label(
                "\u{E8B7}",
                text!(text, "dialog-browse"),
            ))
            .into(),
    ];
    browse.extend(browsed.into_iter().map(choice));
    if let Some(message) = view.message {
        browse.push(
            widgets::caption(message)
                .margin(Thickness::new(0.0, 4.0, 0.0, 0.0))
                .automation_id(automation_id::DIALOG_MESSAGE)
                .automation_live_setting(windows_reactor::AutomationLiveSetting::Polite)
                .into(),
        );
    }
    dialog
        .title(view.title)
        .primary_button_text(view.primary)
        .is_primary_button_enabled(view.primary_enabled)
        .content(
            StackPanel::new().spacing(16.0).max_width(472.0).children((
                widgets::caption(view.description),
                StackPanel::new().spacing(6.0).children((
                    widgets::strong(text!(text, "dialog-recent")),
                    StackPanel::new().spacing(2.0).children(rows),
                )),
                Border::new()
                    .height(1.0)
                    .background(ThemeBrush::CardStroke)
                    .horizontal_alignment(HorizontalAlignment::Stretch),
                StackPanel::new().spacing(2.0).children(browse),
            )),
        )
}
fn title_dialog(settings: &Settings, context: &ViewContext<'_, Settings>) -> ContentDialog {
    let text = &settings.input.text;
    let dialog = ContentDialog::new()
        .close_button_text(text!(text, "dialog-cancel"))
        .default_button(ContentDialogButton::Primary)
        .is_open(settings.title_dialog.is_some())
        .on_closed(context.callback(Message::TitleClosed));
    let Some(state) = &settings.title_dialog else {
        return dialog;
    };
    let view = title_dialog_view(&settings.input.model, text, state);
    let mut field: Vec<View> = vec![
        TextBox::new()
            .header(widgets::body(&view.label))
            .text(&view.text)
            .automation_id(automation_id::TITLE_DIALOG_TEXT)
            .automation_name(&view.label)
            .on_text_changed(
                context.callback(|text: std::rc::Rc<str>| Message::TitleText(text.to_string())),
            )
            .into(),
    ];
    if let Some(error) = &view.error {
        field.push(widgets::field_error(
            error,
            automation_id::TITLE_DIALOG_ERROR,
        ));
    }
    field.push(
        CheckBox::new()
            .is_checked(Some(view.regex))
            .automation_id(automation_id::TITLE_DIALOG_REGEX)
            .automation_name(text!(text, "title-dialog-regex"))
            .on_is_checked_changed(
                context
                    .callback(|checked: Option<bool>| Message::TitleRegex(checked == Some(true))),
            )
            .content(text!(text, "title-dialog-regex"))
            .into(),
    );
    if let Some(suggestion) = &view.suggestion {
        field.push(
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(8.0)
                .children((
                    widgets::caption(suggestion)
                        .automation_id(automation_id::TITLE_DIALOG_LAST_SEEN)
                        .vertical_alignment(VerticalAlignment::Center),
                    HyperlinkButton::new()
                        .automation_id(automation_id::TITLE_DIALOG_USE)
                        .on_click(context.message(Message::UseTitle))
                        .content(text!(text, "title-dialog-use")),
                ))
                .into(),
        );
    }
    dialog
        .title(view.title)
        .primary_button_text(text!(text, "title-dialog-save"))
        .is_primary_button_enabled(view.save_enabled)
        .content(StackPanel::new().spacing(16.0).max_width(472.0).children((
            widgets::caption(view.description),
            StackPanel::new().spacing(8.0).children(field),
        )))
}

#[cfg(test)]
mod tests;
