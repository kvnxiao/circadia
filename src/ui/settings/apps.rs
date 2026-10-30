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
use crate::l10n::bind_text;
use crate::platform::nls::RegionalFormat;
use crate::ui::file_info::AppFile;
use crate::ui::file_info::AppSelection;
use crate::ui::icon_cache::AppIcons;
use crate::ui::status;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::ButtonStyle;
use windows_reactor::CheckBox;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::ContentDialog;
use windows_reactor::ContentDialogButton;
use windows_reactor::ContentDialogExt;
use windows_reactor::ElementRef;
use windows_reactor::FocusEventInfo;
use windows_reactor::FontIcon;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::HyperlinkButton;
use windows_reactor::KeyedView;
use windows_reactor::Orientation;
use windows_reactor::ScrollViewer;
use windows_reactor::StackPanel;
use windows_reactor::TextBox;
use windows_reactor::ThemeBrush;
use windows_reactor::Thickness;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::keyed;

const SUN_GLYPH: &str = "\u{E706}";
const ADD_GLYPH: &str = "\u{E710}";
pub(super) const MINUTE: Duration = Duration::from_mins(1);

pub(super) fn view(settings: &Settings, context: &ViewContext<'_, Settings>) -> Vec<View> {
    let model = &settings.input.model;
    let (text, region) = (&settings.input.text, &settings.input.region);
    bind_text!(fl, text);
    let manual = manual_view(model, text, region);
    let mut children = vec![
        widgets::group(vec![
            widgets::card(
                Row::new(
                    "\u{E7F4}",
                    fl!("apps-explanation-title"),
                    fl!(
                        "apps-explanation-description",
                        kelvin = problems::kelvin(region, 6500)
                    ),
                )
                .view(settings.stacked()),
            ),
            widgets::card(
                Row::new(SUN_GLYPH, fl!("manual-title"), manual.description.clone())
                    .controls(vec![manual_button(manual, context)])
                    .view(settings.stacked()),
            ),
        ]),
        widgets::section(fl!("section-recent"), false),
        recent_card(settings, context),
        widgets::caption(fl!("recent-note"))
            .margin(Thickness::new(0.0, 8.0, 0.0, 0.0))
            .into(),
        list_header(settings, context),
    ];
    children.extend(settings.feedback(Field::Suppression));
    children.extend(suppression_cards(settings, context));
    children.push(widgets::section(fl!("section-never"), false));
    children.push(exclusions_card(settings, context));
    children.push(widgets::section(fl!("section-timing"), false));
    let delay = fl!("focus-delay-title");
    children.push(widgets::card(
        Row::new("\u{E916}", &delay, fl!("focus-delay-description"))
            .controls(settings.with_feedback(
                Field::FocusDelayMs,
                settings.number_controls(
                    NumberControls {
                        field: Field::FocusDelayMs,
                        number: automation_id::FOCUS_DELAY_NUMBER,
                        slider: None,
                        title: &delay,
                        unit: fl!("milliseconds-unit"),
                    },
                    context,
                ),
            ))
            .view(settings.stacked()),
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
    bind_text!(fl, text);
    let enabled = status::manual_available(model.management, model.scheduling, model.suppression);
    let description = if model.suppression.manual() {
        model.manual_end.map_or_else(
            || fl!("manual-active"),
            |minute| fl!("manual-until", time = problems::clock(region, minute)),
        )
    } else if model.accepted.is_some() && !model.management.is_on() {
        fl!("manual-unmanaged")
    } else {
        fl!(
            "manual-description",
            kelvin = problems::kelvin(region, 6500)
        )
    };
    let (command, label) = if model.suppression.manual() {
        (ManualCommand::Stop, fl!("manual-stop"))
    } else {
        (ManualCommand::Start, fl!("manual-title"))
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
    bind_text!(fl, text);
    let minutes = elapsed.as_secs() / MINUTE.as_secs();
    let whole = |value: u64| problems::whole(region, i64::try_from(value).unwrap_or(i64::MAX));
    match minutes {
        0 => fl!("recent-now"),
        1..60 => fl!("recent-minutes", minutes = whole(minutes)),
        60..1440 => fl!("recent-hours", hours = whole(minutes / 60)),
        _ => {
            let days = minutes / 1440;
            fl!("recent-days", count = days, days = whole(days))
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
    path: Option<&crate::ExecutablePath>,
    pixels: u16,
) -> AppIcon {
    if crate::same_executable(executable, running_executable) {
        return AppIcon::Circadia;
    }
    path.or_else(|| {
        icons
            .sources
            .get(&crate::ExecutableName::new(executable).ok()?)
    })
    .and_then(|path| icons.apps.get(path))
    .or(icons.generic.as_ref())
    .map_or(AppIcon::Placeholder, |images| {
        AppIcon::Image(Arc::clone(images.at(pixels)))
    })
}

fn icon(
    settings: &Settings,
    executable: &str,
    path: Option<&crate::ExecutablePath>,
    size: f64,
) -> View {
    icon_for(
        &settings.input.icons,
        &settings.input.model.running_executable,
        executable,
        path,
        icon_pixels(settings.dpi),
    )
    .view(size)
}

fn shown_apps(
    page: Page,
    dialog: Option<&AddDialog>,
    model: &Model,
) -> BTreeSet<crate::ExecutablePath> {
    if page != Page::Apps {
        return BTreeSet::new();
    }
    let config = model.config();
    let listed: Vec<crate::ExecutablePath> = [Field::Suppression, Field::Exclusions]
        .into_iter()
        .flat_map(|list| config.entries(list))
        .filter_map(|entry| crate::ui::icon_cache::source(entry, &model.sources))
        .collect();
    model
        .recent
        .iter()
        .filter_map(|recent| {
            recent
                .path
                .to_str()
                .and_then(|path| crate::ExecutablePath::new(path).ok())
        })
        .chain(listed)
        .chain(
            dialog
                .into_iter()
                .flat_map(|dialog| dialog.candidates(model))
                .filter_map(|file| file.identity()),
        )
        .filter(|path| !crate::same_executable(path.file_name(), &model.running_executable))
        .collect()
}

#[derive(Debug, Default)]
pub(super) struct IconRequests {
    view: Option<(Page, Option<Field>)>,
    requested: BTreeSet<crate::ExecutablePath>,
}

impl IconRequests {
    pub(super) fn due(
        &mut self,
        page: Page,
        dialog: Option<&AddDialog>,
        model: &Model,
    ) -> Vec<crate::ExecutablePath> {
        let view = (page, dialog.map(|dialog| dialog.list));
        if self.view != Some(view) {
            self.view = Some(view);
            self.requested.clear();
        }
        let due: Vec<crate::ExecutablePath> = shown_apps(page, dialog, model)
            .into_iter()
            .filter(|executable| !self.requested.contains(executable))
            .collect();
        self.requested.extend(due.iter().cloned());
        due
    }
}

pub(super) fn display_name(model: &Model, file: &AppFile) -> String {
    file.identity()
        .as_ref()
        .and_then(|path| model.names.get(path))
        .cloned()
        .unwrap_or_else(|| file.executable.clone())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RecentRow {
    pub(super) file: AppFile,
    pub(super) name: String,
    pub(super) detail: String,
    pub(super) listed: Option<(usize, AppEntry)>,
    pub(super) covering: usize,
    pub(super) excluded: bool,
}

pub(super) fn recent_rows(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    now: Duration,
) -> Vec<RecentRow> {
    bind_text!(fl, text);
    let config = model.config();
    model
        .recent
        .iter()
        .map(|recent| RecentRow {
            file: AppFile {
                executable: recent.executable.clone(),
                path: recent.path.clone(),
            },
            name: display_name(
                model,
                &AppFile {
                    executable: recent.executable.clone(),
                    path: recent.path.clone(),
                },
            ),
            detail: {
                let detail = fl!(
                    "recent-detail",
                    executable = recent.executable.as_str(),
                    since = since(text, region, recent.since_focus(now))
                );
                let same_name = model
                    .recent
                    .iter()
                    .filter(|other| crate::same_executable(&other.executable, &recent.executable))
                    .count()
                    > 1;
                recent
                    .path
                    .to_str()
                    .and_then(|path| crate::ExecutablePath::new(path).ok())
                    .filter(|_| same_name)
                    .map_or_else(
                        || detail.clone(),
                        |path| format!("{detail}\n{}", path.folder()),
                    )
            },
            listed: {
                let path = recent
                    .path
                    .to_str()
                    .and_then(|path| crate::ExecutablePath::new(path).ok());
                let entries: Vec<_> = config
                    .entries(Field::Suppression)
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| entry.covers(&recent.executable, path.as_ref()))
                    .collect();
                (entries.len() == 1)
                    .then(|| {
                        entries
                            .first()
                            .map(|(index, entry)| (*index, (*entry).clone()))
                    })
                    .flatten()
            },
            covering: config
                .classify_path(
                    &recent.executable,
                    recent
                        .path
                        .to_str()
                        .and_then(|path| crate::ExecutablePath::new(path).ok())
                        .as_ref(),
                    None,
                    &model.running_executable,
                )
                .covering,
            excluded: config
                .classify_path(
                    &recent.executable,
                    recent
                        .path
                        .to_str()
                        .and_then(|path| crate::ExecutablePath::new(path).ok())
                        .as_ref(),
                    None,
                    &model.running_executable,
                )
                .excluded,
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct EntryRow {
    pub(super) index: usize,
    pub(super) name: String,
    pub(super) executable: String,
    pub(super) path: Option<crate::ExecutablePath>,
    pub(super) expected: AppEntry,
    pub(super) id: String,
    pub(super) detail: String,
    pub(super) inactive: bool,
    pub(super) error: Option<String>,
}

pub(super) fn entry_rows(model: &Model, text: &Text, list: Field) -> Vec<EntryRow> {
    bind_text!(fl, text);
    let config = model.config();
    config
        .entries(list)
        .iter()
        .zip(config.entry_problems(list))
        .enumerate()
        .map(|(index, (entry, problem))| EntryRow {
            index,
            name: if entry.title.is_empty() {
                crate::ui::icon_cache::source(entry, &model.sources).map_or_else(
                    || entry.display_name().to_owned(),
                    |path| {
                        model
                            .names
                            .get(&path)
                            .cloned()
                            .unwrap_or_else(|| entry.executable.clone())
                    },
                )
            } else {
                entry.title.clone()
            },
            executable: entry.executable.clone(),
            path: crate::ui::icon_cache::source(entry, &model.sources),
            expected: entry.clone(),
            id: automation_id::rule(
                list,
                entry,
                config
                    .entries(list)
                    .iter()
                    .take(index)
                    .filter(|other| other.scope() == entry.scope())
                    .count(),
            ),
            detail: {
                let detail = if entry.title.is_empty() {
                    entry.executable.clone()
                } else {
                    fl!(
                        "entry-title-detail",
                        regex = if entry.regex { "yes" } else { "no" },
                        executable = entry.executable.as_str()
                    )
                };
                entry.path.as_ref().map_or_else(
                    || detail.clone(),
                    |path| {
                        let folder = crate::ExecutablePath::new(path)
                            .map_or_else(|_error| path.clone(), |path| path.folder().to_owned());
                        format!("{detail}\n{folder}")
                    },
                )
            },
            inactive: list == Field::Suppression
                && config.inactive(entry, &model.running_executable),
            error: problem.map(|problem| match problem {
                EntryProblem::Invalid => fl!("entry-invalid"),
                EntryProblem::Expression => fl!("entry-expression"),
                EntryProblem::Path => fl!("entry-path-invalid"),
                EntryProblem::PathFilename => fl!("entry-path-filename"),
                EntryProblem::Duplicate => {
                    fl!("entry-duplicate", executable = entry.executable.as_str())
                }
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
    bind_text!(fl, text);
    vec![
        FixedRow {
            executable: model.running_executable.clone(),
            description: fl!("fixed-self"),
        },
        FixedRow {
            executable: crate::config::SHARED_HOST.to_owned(),
            description: fl!("fixed-host"),
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AddDialog {
    pub(super) list: Field,
    chosen: BTreeSet<crate::ExecutablePath>,
    restricted: BTreeSet<crate::ExecutablePath>,
    browsed: Vec<AppFile>,
    problem: Option<(String, FieldErrorKind)>,
}

impl AddDialog {
    pub(super) fn new(list: Field) -> Self {
        Self {
            list,
            chosen: BTreeSet::new(),
            restricted: BTreeSet::new(),
            browsed: Vec::new(),
            problem: None,
        }
    }

    pub(super) fn choose(&mut self, path: &crate::ExecutablePath, checked: bool, model: &Model) {
        if checked && self.available(path, model) {
            self.chosen.insert(path.clone());
        } else {
            self.chosen.remove(path);
        }
    }

    pub(super) fn restrict(
        &mut self,
        path: &crate::ExecutablePath,
        restricted: bool,
        model: &Model,
    ) {
        if restricted {
            self.restricted.insert(path.clone());
        } else {
            self.restricted.remove(path);
        }
        if !self.available(path, model) {
            self.chosen.remove(path);
        }
    }

    fn selected(&self, file: AppFile) -> AppSelection {
        let restricted = file
            .identity()
            .is_some_and(|path| self.restricted.contains(&path));
        AppSelection { file, restricted }
    }

    fn available(&self, path: &crate::ExecutablePath, model: &Model) -> bool {
        let selection = self.selected(AppFile {
            executable: path.file_name().to_ascii_lowercase(),
            path: path.as_path().to_path_buf(),
        });
        let entry = selection.entry(selection.file.executable.clone());
        model
            .config()
            .add_entries(self.list, [entry.clone()], &model.running_executable)
            .is_ok()
            && !self
                .chosen
                .iter()
                .filter(|chosen| *chosen != path)
                .any(|chosen| {
                    let selected = self.selected(AppFile {
                        executable: chosen.file_name().to_ascii_lowercase(),
                        path: chosen.as_path().to_path_buf(),
                    });
                    selected.entry(selected.file.executable.clone()).scope() == entry.scope()
                })
    }

    pub(super) fn browsed(&mut self, model: &Model, file: AppFile) -> bool {
        let Some(path) = file.identity() else {
            return false;
        };
        if crate::is_fixed_exclusion(&file.executable, &model.running_executable) {
            self.problem = Some((file.executable, FieldErrorKind::FixedExclusion));
            return false;
        }
        let available = self.available(&path, model);
        self.problem =
            (!available).then(|| (file.executable.clone(), FieldErrorKind::AlreadyListed));
        if available {
            self.chosen.insert(path.clone());
        }
        if !self
            .browsed
            .iter()
            .any(|browsed| browsed.identity().as_ref() == Some(&path))
        {
            self.browsed.push(file);
        }
        true
    }

    pub(super) fn candidates(&self, model: &Model) -> Vec<AppFile> {
        let mut candidates: Vec<AppFile> = Vec::new();
        let recent = model.recent.iter().map(|recent| AppFile {
            executable: recent.executable.clone(),
            path: recent.path.clone(),
        });
        for file in recent.chain(self.browsed.iter().cloned()) {
            let Some(path) = file.identity() else {
                continue;
            };
            let file = self
                .browsed
                .iter()
                .find(|browsed| browsed.identity().as_ref() == Some(&path))
                .cloned()
                .unwrap_or(file);
            let shown = candidates
                .iter()
                .any(|candidate| candidate.identity().as_ref() == Some(&path));
            if !crate::is_fixed_exclusion(&file.executable, &model.running_executable) && !shown {
                candidates.push(file);
            }
        }
        candidates
    }

    pub(super) fn files(&self, model: &Model) -> Vec<AppSelection> {
        self.candidates(model)
            .into_iter()
            .filter(|file| {
                file.identity()
                    .is_some_and(|path| self.chosen.contains(&path) && self.available(&path, model))
            })
            .map(|file| self.selected(file))
            .collect()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum CandidateSection {
    Recent,
    Browsed,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DialogRow {
    pub(super) executable: String,
    pub(super) path: Option<crate::ExecutablePath>,
    pub(super) name: String,
    pub(super) checked: bool,
    pub(super) restricted: bool,
    pub(super) available: bool,
    pub(super) section: CandidateSection,
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
    bind_text!(fl, text);
    let count = dialog.files(model).len();
    let apps = problems::whole(region, i64::try_from(count).unwrap_or(i64::MAX));
    let exclusion = dialog.list == Field::Exclusions;
    DialogView {
        title: if exclusion {
            fl!("dialog-exclude-title")
        } else {
            fl!("dialog-add-title")
        },
        description: if exclusion {
            fl!("dialog-exclude-description")
        } else {
            fl!("dialog-add-description")
        },
        rows: dialog
            .candidates(model)
            .into_iter()
            .map(|file| DialogRow {
                path: file.identity(),
                name: display_name(model, &file),
                checked: file.identity().is_some_and(|path| {
                    dialog.chosen.contains(&path) && dialog.available(&path, model)
                }),
                restricted: file
                    .identity()
                    .is_some_and(|path| dialog.restricted.contains(&path)),
                available: file
                    .identity()
                    .is_some_and(|path| dialog.available(&path, model)),
                section: if dialog
                    .browsed
                    .iter()
                    .any(|browsed| browsed.identity() == file.identity())
                {
                    CandidateSection::Browsed
                } else {
                    CandidateSection::Recent
                },
                executable: file.executable,
            })
            .collect(),
        primary: if exclusion {
            fl!("dialog-exclude", count = count, apps = apps.as_str())
        } else {
            fl!("dialog-add", count = count, apps = apps.as_str())
        },
        primary_enabled: count > 0,
        message: dialog
            .problem
            .as_ref()
            .and_then(|(executable, kind)| match kind {
                FieldErrorKind::AlreadyListed => Some(fl!(
                    "dialog-already-listed",
                    executable = executable.as_str()
                )),
                FieldErrorKind::FixedExclusion => {
                    Some(fl!("dialog-fixed", executable = executable.as_str()))
                }
                _ => None,
            }),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TitleDialog {
    list: Field,
    index: usize,
    expected: AppEntry,
    restricted: bool,
    path: Option<String>,
    path_problem: Option<EntryProblem>,
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
            expected: entry.clone(),
            restricted: entry.path.is_some(),
            path: entry
                .path
                .clone()
                .or_else(|| {
                    crate::ui::icon_cache::source(entry, &model.sources)
                        .map(|path| path.as_str().to_owned())
                })
                .or_else(|| {
                    model
                        .recent
                        .iter()
                        .find(|recent| {
                            crate::same_executable(&recent.executable, &entry.executable)
                        })
                        .map(|recent| recent.path.to_string_lossy().into_owned())
                }),
            path_problem: None,
            text: entry.title.clone(),
            regex: entry.regex,
        })
    }

    pub(super) fn matches(&self, expected: &AppEntry) -> bool {
        self.expected.same_rule(expected)
    }

    pub(super) fn restrict(&mut self, restricted: bool) {
        self.restricted = restricted;
        self.path_problem = None;
    }

    pub(super) fn browsed(&mut self, file: &AppFile) -> bool {
        if !crate::same_executable(&self.expected.executable, &file.executable) {
            self.path_problem = Some(EntryProblem::PathFilename);
            return false;
        }
        let Some(path) = file.identity() else {
            self.path_problem = Some(EntryProblem::Path);
            return false;
        };
        self.path = Some(path.as_str().to_owned());
        self.path_problem = None;
        true
    }

    fn replacement(&self) -> AppEntry {
        self.expected
            .clone()
            .with_title(self.text.clone(), self.regex)
            .with_path(
                self.restricted
                    .then(|| self.path.clone().unwrap_or_default()),
            )
    }

    fn path_error(&self, model: &Model) -> Option<EntryProblem> {
        if self.restricted && self.path_problem.is_some() {
            return self.path_problem;
        }
        let replacement = self.replacement();
        if let Err(error) = replacement.restriction() {
            return Some(error);
        }
        model
            .config()
            .entries(self.list)
            .iter()
            .enumerate()
            .any(|(index, entry)| {
                index != self.index
                    && entry.scope().is_some()
                    && entry.scope() == replacement.scope()
            })
            .then_some(EntryProblem::Duplicate)
    }

    pub(super) fn set_text(&mut self, text: &str) {
        text.clone_into(&mut self.text);
    }

    pub(super) fn set_regex(&mut self, regex: bool) {
        self.regex = regex;
    }

    pub(super) fn use_title(&mut self, model: &Model) {
        if let Some(title) = last_seen(model, &self.replacement()) {
            self.text = if self.regex {
                regex::escape(title)
            } else {
                title.to_owned()
            };
        }
    }

    pub(super) fn command(&self, model: &Model) -> Option<Command> {
        (crate::valid_title(&self.text, self.regex)
            && self.path_error(model).is_none()
            && model
                .config()
                .entries(self.list)
                .get(self.index)
                .is_some_and(|entry| entry.same_rule(&self.expected)))
        .then(|| Command::Title {
            list: self.list,
            index: self.index,
            expected: self.expected.clone(),
            title: self.text.clone(),
            regex: self.regex,
            path: self.replacement().path,
        })
    }
}

fn last_seen<'a>(model: &'a Model, entry: &AppEntry) -> Option<&'a str> {
    let name = entry.file_name()?;
    let restriction = entry.restriction().ok()?;
    model
        .recent
        .iter()
        .find(|recent| {
            crate::same_executable(&recent.executable, &name)
                && restriction.as_ref().is_none_or(|path| {
                    recent
                        .path
                        .to_str()
                        .and_then(|path| crate::ExecutablePath::new(path).ok())
                        .as_ref()
                        == Some(path)
                })
        })
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
    pub(super) restricted: bool,
    pub(super) path: Option<String>,
    pub(super) path_error: Option<String>,
    pub(super) suggestion: Option<String>,
    pub(super) error: Option<String>,
    pub(super) save_enabled: bool,
}

pub(super) fn title_dialog_view(
    model: &Model,
    text: &Text,
    dialog: &TitleDialog,
) -> TitleDialogView {
    bind_text!(fl, text);
    let regex = if dialog.regex { "yes" } else { "no" };
    let valid = crate::valid_title(&dialog.text, dialog.regex);
    TitleDialogView {
        title: fl!("title-dialog-title"),
        description: fl!("title-dialog-description", regex = regex),
        label: fl!("title-dialog-field", regex = regex),
        text: dialog.text.clone(),
        regex: dialog.regex,
        restricted: dialog.restricted,
        path: dialog.path.clone(),
        path_error: dialog.path_error(model).map(|problem| match problem {
            EntryProblem::Duplicate => fl!("dialog-scope-duplicate"),
            EntryProblem::PathFilename => fl!("entry-path-filename"),
            _ => fl!("entry-path-invalid"),
        }),
        suggestion: last_seen(model, &dialog.replacement())
            .map(|title| fl!("title-dialog-last-seen", title = title)),
        error: (!valid).then(|| fl!("title-dialog-invalid")),
        save_enabled: dialog.command(model).is_some(),
    }
}

fn app_row(
    icon: View,
    name: &str,
    detail: Vec<View>,
    controls: Vec<View>,
    id: String,
    stacked: bool,
) -> Border {
    Border::new()
        .min_height(60.0)
        .padding(Thickness::new(16.0, 10.0, 16.0, 10.0))
        .automation_id(id)
        .content(
            Grid::new()
                .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                .rows([GridLength::Auto, GridLength::Auto])
                .row_spacing(if stacked { 12.0 } else { 0.0 })
                .column_spacing(16.0)
                .children((
                    icon,
                    StackPanel::new()
                        .grid_column(1)
                        .grid_column_span(if stacked { 2 } else { 1 })
                        .spacing(2.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(
                            std::iter::once(widgets::body(name).into())
                                .chain(detail)
                                .collect::<Vec<View>>(),
                        ),
                    widgets::action_group(controls)
                        .grid_column(if stacked { 1 } else { 2 })
                        .grid_row(i32::from(stacked))
                        .grid_column_span(if stacked { 2 } else { 1 }),
                )),
        )
}

fn title_button(
    row: &EntryRow,
    list: Field,
    id: &str,
    settings: &Settings,
    context: &ViewContext<'_, Settings>,
) -> View {
    bind_text!(fl, settings.input.text);
    widgets::icon_button("\u{E71C}", 16.0)
        .is_enabled(settings.input.model.editable())
        .automation_id(automation_id::title(id))
        .automation_name(fl!("entry-title-name", name = row.name.as_str()))
        .on_click(context.message(Message::OpenTitle(list, row.index, row.expected.clone())))
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
    bind_text!(fl, text);
    let editable = model.editable();
    let rows = recent_rows(model, text, region, crate::elapsed_since_boot());
    if rows.is_empty() {
        return widgets::card(
            Border::new()
                .padding(Thickness::new(16.0, 12.0, 16.0, 12.0))
                .content(widgets::caption(fl!("recent-empty"))),
        );
    }
    let views: Vec<KeyedView> = rows
        .into_iter()
        .enumerate()
        .map(|(position, row)| {
            let executable = row.file.identity().map_or_else(
                || row.file.path.to_string_lossy().into_owned(),
                |path| path.as_str().to_owned(),
            );
            let mut controls: Vec<View> = Vec::new();
            let mut detail = vec![widgets::caption(&row.detail).into()];
            if row.excluded {
                detail.push(widgets::caption(fl!("recent-excluded")).into());
            }
            if let Some((index, expected)) = &row.listed {
                controls.push(
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(4.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children((
                            FontIcon::new().glyph("\u{E73E}").font_size(12.0),
                            widgets::caption(fl!("recent-on-list")),
                        ))
                        .into(),
                );
                controls.push(
                    Button::new()
                        .is_enabled(editable)
                        .automation_id(automation_id::recent_remove(&executable))
                        .automation_name(fl!("recent-remove-name", name = row.name.as_str()))
                        .on_click(context.message(Message::Command(Command::Remove(
                            Field::Suppression,
                            *index,
                            expected.clone(),
                        ))))
                        .content(fl!("recent-remove"))
                        .into(),
                );
            } else if row.covering == 0 {
                controls.push(
                    Button::new()
                        .is_enabled(editable)
                        .automation_id(automation_id::recent_add(&executable))
                        .automation_name(fl!("recent-add-name", name = row.name.as_str()))
                        .on_click(context.message(Message::Command(Command::Add(
                            Field::Suppression,
                            vec![row.file.clone().into()],
                        ))))
                        .content(widgets::glyph_label(ADD_GLYPH, fl!("recent-add")))
                        .into(),
                );
            } else {
                detail.push(widgets::caption(fl!("recent-multiple-rules")).into());
            }
            let row_view = app_row(
                icon(
                    settings,
                    &row.file.executable,
                    row.file.identity().as_ref(),
                    32.0,
                ),
                &row.name,
                detail,
                controls,
                automation_id::recent(&executable),
                settings.stacked(),
            );
            keyed(executable, divided(row_view.into(), position > 0))
        })
        .collect();
    widgets::card(StackPanel::new().keyed_children(views))
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
    bind_text!(fl, text);
    let count = settings
        .input
        .model
        .config()
        .entries(Field::Suppression)
        .len();
    Grid::new()
        .columns([GridLength::STAR, GridLength::Auto])
        .rows([GridLength::Auto, GridLength::Auto])
        .row_spacing(if settings.stacked() { 8.0 } else { 0.0 })
        .margin(Thickness::new(0.0, 28.0, 0.0, 8.0))
        .children((
            widgets::strong(fl!("section-suppression", count = count))
                .vertical_alignment(VerticalAlignment::Center),
            Button::new()
                .grid_column(i32::from(!settings.stacked()))
                .grid_row(i32::from(settings.stacked()))
                .horizontal_alignment(HorizontalAlignment::Right)
                .style(ButtonStyle::Accent)
                .is_enabled(settings.input.model.editable())
                .automation_id(automation_id::ADD_APP)
                .automation_name(fl!("add-app"))
                .on_click(context.message(Message::OpenDialog(Field::Suppression)))
                .content(widgets::glyph_label(ADD_GLYPH, fl!("add-app"))),
        ))
        .into()
}

fn entry_card(row: &EntryRow, settings: &Settings, controls: Vec<View>, id: String) -> View {
    let text = &settings.input.text;
    bind_text!(fl, text);
    let mut detail: Vec<View> = vec![widgets::caption(&row.detail).into()];
    if row.inactive {
        detail.push(widgets::caption(fl!("entry-inactive")).into());
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
        .content(
            app_row(
                icon(settings, &row.executable, row.path.as_ref(), 32.0),
                &row.name,
                detail,
                controls,
                id,
                false,
            )
            .padding(Thickness::new(
                widgets::CONTENT_INSET,
                10.0,
                widgets::COMPACT_ACTION_INSET,
                10.0,
            )),
        )
        .into()
}

fn suppression_cards(settings: &Settings, context: &ViewContext<'_, Settings>) -> Vec<View> {
    let model = &settings.input.model;
    let text = &settings.input.text;
    bind_text!(fl, text);
    let rows = entry_rows(model, text, Field::Suppression);
    if rows.is_empty() {
        return vec![widgets::card(
            Row::new(
                APP_GLYPH,
                fl!("list-empty-title"),
                fl!("list-empty-description"),
            )
            .view(settings.stacked()),
        )];
    }
    let cards: Vec<KeyedView> = rows
        .iter()
        .map(|row| {
            let id = row.id.clone();
            let remove = remove_button(
                &fl!("entry-remove-name", name = row.name.as_str()),
                automation_id::remove(&id),
                model.editable(),
                context.message(Message::Command(Command::Remove(
                    Field::Suppression,
                    row.index,
                    row.expected.clone(),
                ))),
            );
            let title = title_button(row, Field::Suppression, &id, settings, context);
            keyed(
                id.clone(),
                entry_card(row, settings, vec![title, remove.into()], id),
            )
        })
        .collect();
    vec![StackPanel::new().spacing(4.0).keyed_children(cards).into()]
}

fn exclusion_row(
    row: &EntryRow,
    settings: &Settings,
    context: &ViewContext<'_, Settings>,
) -> KeyedView {
    let model = &settings.input.model;
    let text = &settings.input.text;
    bind_text!(fl, text);
    let id = row.id.clone();
    let mut detail: Vec<View> = vec![widgets::caption(&row.detail).into()];
    if let Some(error) = &row.error {
        detail.push(widgets::field_error(error, &automation_id::message(&id)));
    }
    let actions = vec![
        title_button(row, Field::Exclusions, &id, settings, context),
        remove_button(
            &fl!("entry-remove-name", name = row.name.as_str()),
            automation_id::remove(&id),
            model.editable(),
            context.message(Message::Command(Command::Remove(
                Field::Exclusions,
                row.index,
                row.expected.clone(),
            ))),
        )
        .into(),
    ];
    keyed(
        id.clone(),
        nested_app_row(
            icon(settings, &row.executable, row.path.as_ref(), 32.0),
            &row.name,
            StackPanel::new().spacing(2.0).children(detail).into(),
            actions,
            &id,
        ),
    )
}

fn exclusions_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let model = &settings.input.model;
    let text = &settings.input.text;
    bind_text!(fl, text);
    let rows = entry_rows(model, text, Field::Exclusions);
    let held = rows.iter().any(|row| row.error.is_some())
        || model.report.marks.contains_key(&Field::Exclusions);
    let expansion = Expansion::chosen(settings.expanded.exclusions).held_when(held);
    let expanded = expansion.expanded();
    let expand_name = match expansion {
        Expansion::Held => fl!("exclusions-held"),
        Expansion::Expanded => fl!("exclusions-collapse"),
        Expansion::Collapsed => fl!("exclusions-expand"),
    };
    let header = Row::new(
        "\u{E733}",
        fl!("exclusions-title"),
        fl!("exclusions-description"),
    )
    .controls(vec![
        widgets::action_group(vec![
            Button::new()
                .is_enabled(model.editable())
                .automation_id(automation_id::ADD_EXCLUSION)
                .on_click(context.message(Message::OpenDialog(Field::Exclusions)))
                .content(fl!("add-exclusion"))
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
    .trailing_icon()
    .view(settings.stacked());
    let mut children: Vec<View> = vec![header];
    children.extend(settings.feedback(Field::Exclusions));
    if expanded {
        children.extend(fixed_rows(model, text).into_iter().map(|row| {
            nested_app_row(
                icon(settings, &row.executable, None, 32.0),
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
                                .automation_name(fl!("fixed-locked")),
                        )
                        .into(),
                ],
                &automation_id::fixed_exclusion(&row.executable.to_ascii_lowercase()),
            )
        }));
        children.push(
            StackPanel::new()
                .keyed_children(rows.iter().map(|row| exclusion_row(row, settings, context)))
                .into(),
        );
        if rows.is_empty() {
            children.push(
                widgets::nested("exclusions-empty")
                    .content(widgets::caption(fl!("exclusions-empty")))
                    .into(),
            );
        }
    }
    widgets::card(StackPanel::new().children(children))
}

fn nested_app_row(icon: View, name: &str, detail: View, actions: Vec<View>, id: &str) -> View {
    widgets::nested(id)
        .padding(Thickness::new(
            52.0,
            10.0,
            widgets::COMPACT_ACTION_INSET,
            10.0,
        ))
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

#[derive(Clone, PartialEq)]
struct CandidateInput {
    position: usize,
    checkbox: CheckBox,
    restriction: CheckBox,
    description: View,
}

struct Candidate {
    input: CandidateInput,
    checkbox: ElementRef<CheckBox>,
    restriction: ElementRef<CheckBox>,
    focused: Option<bool>,
}

impl Component for Candidate {
    type Input = CandidateInput;
    type Message = Option<bool>;

    fn create(input: &Self::Input, _: &ComponentContext<Self>) -> Self {
        Self {
            input: input.clone(),
            checkbox: ElementRef::new(),
            restriction: ElementRef::new(),
            focused: None,
        }
    }

    fn input_changed(&mut self, input: &Self::Input, _: &ComponentContext<Self>) {
        let target = self.focused.map(|restriction| {
            if restriction {
                &self.restriction
            } else {
                &self.checkbox
            }
        });
        if input.position != self.input.position
            && target.is_some_and(|target| !target.request_focus())
        {
            tracing::debug!("candidate focus request rejected");
        }
        self.input = input.clone();
    }

    fn update(&mut self, focused: Option<bool>, _: &ComponentContext<Self>) {
        self.focused = focused;
    }

    fn view(&self, input: &Self::Input, context: &mut ViewContext<'_, Self>) -> View {
        StackPanel::new()
            .spacing(6.0)
            .margin(Thickness::new(0.0, 4.0, 0.0, 8.0))
            .children((
                Border::new()
                    .on_got_focus(context.callback(|_: FocusEventInfo| Some(false)))
                    .on_lost_focus(context.callback(|_: FocusEventInfo| None))
                    .content(input.checkbox.clone().element_ref(&self.checkbox)),
                input.description.clone(),
                Border::new()
                    .on_got_focus(context.callback(|_: FocusEventInfo| Some(true)))
                    .on_lost_focus(context.callback(|_: FocusEventInfo| None))
                    .content(input.restriction.clone().element_ref(&self.restriction)),
            ))
            .into()
    }
}

fn candidate_view(
    settings: &Settings,
    context: &ViewContext<'_, Settings>,
    position: usize,
    row: &DialogRow,
) -> KeyedView {
    let text = &settings.input.text;
    bind_text!(fl, text);
    let path = row.path.clone();
    let identity = path.as_ref().map_or("", crate::ExecutablePath::as_str);
    let restriction_path = path.clone();
    keyed(
        automation_id::identity(identity),
        View::component::<Candidate>(CandidateInput {
            position,
            restriction: CheckBox::new()
                .is_checked(row.restricted)
                .automation_id(automation_id::dialog_restriction(identity))
                .automation_name(fl!("installation-only"))
                .content(widgets::body(fl!("installation-only")))
                .on_is_checked_changed(context.callback(move |checked: Option<bool>| {
                    restriction_path.clone().map_or(Message::Tick, |path| {
                        Message::RestrictCandidate(path, checked == Some(true))
                    })
                })),
            description: StackPanel::new()
                .spacing(4.0)
                .children(
                    std::iter::once(widgets::caption(identity).into())
                        .chain(
                            (!row.available)
                                .then(|| widgets::caption(fl!("dialog-scope-duplicate")).into()),
                        )
                        .collect::<Vec<View>>(),
                )
                .into(),
            checkbox: CheckBox::new()
                .is_checked(row.checked)
                .is_enabled(row.available)
                .automation_id(automation_id::dialog_choice(identity))
                .automation_name(fl!(
                    "app-entry-name",
                    name = row.name.as_str(),
                    executable = row.executable.as_str()
                ))
                .on_is_checked_changed(context.callback(move |checked: Option<bool>| {
                    path.clone().map_or(Message::Tick, |path| {
                        Message::Choose(path, checked == Some(true))
                    })
                }))
                .content(
                    Grid::new()
                        .columns([GridLength::Auto, GridLength::STAR])
                        .column_spacing(14.0)
                        .children((
                            icon(settings, &row.executable, row.path.as_ref(), 28.0),
                            StackPanel::new().grid_column(1).children((
                                widgets::body(&row.name),
                                widgets::caption(&row.executable),
                            )),
                        )),
                ),
        }),
    )
}

fn add_dialog(settings: &Settings, context: &ViewContext<'_, Settings>) -> ContentDialog {
    let model = &settings.input.model;
    let (text, region) = (&settings.input.text, &settings.input.region);
    bind_text!(fl, text);
    let dialog = ContentDialog::new()
        .close_button_text(fl!("dialog-cancel"))
        .default_button(ContentDialogButton::Primary)
        .is_open(settings.dialog.is_some())
        .on_closed(context.callback(Message::DialogClosed));
    let Some(state) = &settings.dialog else {
        return dialog;
    };
    let view = dialog_view(model, text, region, state);
    let choice = |(position, row)| candidate_view(settings, context, position, &row);
    let (browsed, recent): (Vec<DialogRow>, Vec<DialogRow>) = view
        .rows
        .into_iter()
        .partition(|row| row.section == CandidateSection::Browsed);
    let rows: Vec<KeyedView> = if recent.is_empty() {
        vec![keyed("empty", widgets::caption(fl!("dialog-all-listed")))]
    } else {
        recent.into_iter().enumerate().map(choice).collect()
    };
    let mut browse: Vec<View> = vec![
        widgets::body(fl!("dialog-file-title")).into(),
        widgets::caption(fl!("dialog-file-description")).into(),
        Button::new()
            .margin(Thickness::new(0.0, 8.0, 0.0, 0.0))
            .automation_id(automation_id::DIALOG_BROWSE)
            .automation_name(fl!("dialog-browse"))
            .on_click(context.message(Message::Browse))
            .content(widgets::glyph_label("\u{E8B7}", fl!("dialog-browse")))
            .into(),
    ];
    browse.push(
        StackPanel::new()
            .keyed_children(browsed.into_iter().enumerate().map(choice))
            .into(),
    );
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
            ScrollViewer::new()
                .max_height(settings.dialog_height())
                .content(
                    StackPanel::new().spacing(16.0).max_width(472.0).children((
                        widgets::caption(view.description),
                        StackPanel::new().spacing(6.0).children((
                            widgets::strong(fl!("dialog-recent")),
                            StackPanel::new().spacing(2.0).keyed_children(rows),
                        )),
                        Border::new()
                            .height(1.0)
                            .background(ThemeBrush::CardStroke)
                            .horizontal_alignment(HorizontalAlignment::Stretch),
                        StackPanel::new().spacing(2.0).children(browse),
                    )),
                ),
        )
}
fn restriction_fields(
    text: &Text,
    view: &TitleDialogView,
    context: &ViewContext<'_, Settings>,
) -> Vec<View> {
    bind_text!(fl, text);
    let mut field: Vec<View> = vec![
        CheckBox::new()
            .is_checked(view.restricted)
            .automation_id(automation_id::RULE_RESTRICTION)
            .automation_name(fl!("installation-only"))
            .content(widgets::body(fl!("installation-only")))
            .on_is_checked_changed(
                context
                    .callback(|checked: Option<bool>| Message::RestrictRule(checked == Some(true))),
            )
            .into(),
    ];
    if view.restricted {
        field.push(
            widgets::caption(view.path.as_deref().unwrap_or(&fl!("installation-unknown")))
                .automation_id(automation_id::RULE_PATH)
                .into(),
        );
        field.push(
            Button::new()
                .horizontal_alignment(HorizontalAlignment::Right)
                .automation_id(automation_id::RULE_BROWSE)
                .content(fl!("installation-browse"))
                .on_click(context.message(Message::Browse))
                .into(),
        );
        if let Some(error) = &view.path_error {
            field.push(widgets::field_error(error, automation_id::RULE_PATH_ERROR));
        }
    } else if let Some(error) = &view.path_error {
        field.push(widgets::field_error(error, automation_id::RULE_PATH_ERROR));
    }
    field
}

fn title_dialog(settings: &Settings, context: &ViewContext<'_, Settings>) -> ContentDialog {
    let text = &settings.input.text;
    bind_text!(fl, text);
    let dialog = ContentDialog::new()
        .close_button_text(fl!("dialog-cancel"))
        .default_button(ContentDialogButton::Primary)
        .is_open(settings.title_dialog.is_some())
        .on_closed(context.callback(Message::TitleClosed));
    let Some(state) = &settings.title_dialog else {
        return dialog;
    };
    let view = title_dialog_view(&settings.input.model, text, state);
    let mut field = restriction_fields(text, &view, context);
    field.push(
        TextBox::new()
            .header(widgets::body(&view.label))
            .text(&view.text)
            .automation_id(automation_id::TITLE_DIALOG_TEXT)
            .automation_name(&view.label)
            .on_text_changed(
                context.callback(|text: std::rc::Rc<str>| Message::TitleText(text.to_string())),
            )
            .into(),
    );
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
            .automation_name(fl!("title-dialog-regex"))
            .on_is_checked_changed(
                context
                    .callback(|checked: Option<bool>| Message::TitleRegex(checked == Some(true))),
            )
            .content(fl!("title-dialog-regex"))
            .into(),
    );
    if let Some(suggestion) = &view.suggestion {
        field.push(
            StackPanel::new()
                .spacing(8.0)
                .children((
                    widgets::caption(suggestion)
                        .automation_id(automation_id::TITLE_DIALOG_LAST_SEEN),
                    HyperlinkButton::new()
                        .horizontal_alignment(HorizontalAlignment::Right)
                        .automation_id(automation_id::TITLE_DIALOG_USE)
                        .on_click(context.message(Message::UseTitle))
                        .content(fl!("title-dialog-use")),
                ))
                .into(),
        );
    }
    dialog
        .title(view.title)
        .primary_button_text(fl!("title-dialog-save"))
        .is_primary_button_enabled(view.save_enabled)
        .content(
            ScrollViewer::new()
                .max_height(settings.dialog_height())
                .content(StackPanel::new().spacing(16.0).max_width(472.0).children((
                    widgets::caption(view.description),
                    StackPanel::new().spacing(8.0).children(field),
                ))),
        )
}

#[cfg(test)]
mod tests;
