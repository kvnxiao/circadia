use super::APP_ICON;
use super::Command;
use super::Folder;
use super::Message;
use super::Model;
use super::Settings;
use super::problems;
use super::problems::FeatureMessage;
use super::problems::Severity;
use super::schedule;
use super::widgets;
use super::widgets::Row;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::DiagnosticLog;
use crate::DiagnosticOperation;
use crate::Field;
use crate::FieldValue;
use crate::automation_id;
use crate::l10n::Text;
use crate::l10n::WINDOWS;
use crate::l10n::text;
use crate::platform::startup::Effective;
use crate::platform::startup::Registration;
#[cfg(test)]
use std::cell::RefCell;
use std::io;
#[cfg(test)]
use std::rc::Rc;
use windows_reactor::Button;
use windows_reactor::ComboBox;
use windows_reactor::EncodedImage;
use windows_reactor::FontIcon;
use windows_reactor::Image;
use windows_reactor::StackPanel;
use windows_reactor::ThemeBrush;
use windows_reactor::ToggleSwitch;
use windows_reactor::View;
use windows_reactor::ViewContext;

pub(in crate::ui) trait Registrar {
    fn read(&self) -> io::Result<Effective>;
    fn set(&self, enabled: bool) -> io::Result<()>;
}

impl Registrar for Registration {
    fn read(&self) -> io::Result<Effective> {
        Registration::read(self)
    }

    fn set(&self, enabled: bool) -> io::Result<()> {
        Registration::set(self, enabled)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::ui) struct SignInStatus {
    effective: Option<Result<Effective, DiagnosticFailure>>,
    failure: Option<(bool, DiagnosticFailure)>,
}

#[derive(Debug, Default)]
pub(in crate::ui) struct SignIn {
    status: SignInStatus,
    log: DiagnosticLog,
}

impl SignIn {
    pub(in crate::ui) fn status(&self) -> SignInStatus {
        self.status
    }

    pub(in crate::ui) fn read(&mut self, registration: &dyn Registrar) {
        let effective = registration.read().map_err(|error| diagnostic(&error));
        match effective {
            Ok(_) => self.log.recovered(DiagnosticOperation::StartupStatusRead),
            Err(failure) => self
                .log
                .failure(DiagnosticOperation::StartupStatusRead, failure),
        }
        self.status.effective = Some(effective);
    }

    pub(in crate::ui) fn register(&mut self, registration: &dyn Registrar, enabled: bool) {
        match registration.set(enabled) {
            Ok(()) => {
                self.status.failure = None;
                self.log.recovered(DiagnosticOperation::StartupRegistration);
            }
            Err(error) => {
                let failure = diagnostic(&error);
                self.status.failure = Some((enabled, failure));
                self.log
                    .failure(DiagnosticOperation::StartupRegistration, failure);
            }
        }
        self.read(registration);
    }
}

fn diagnostic(error: &io::Error) -> DiagnosticFailure {
    DiagnosticFailure {
        category: match error.kind() {
            io::ErrorKind::PermissionDenied => DiagnosticCategory::AccessDenied,
            io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => {
                DiagnosticCategory::InvalidData
            }
            _ => DiagnosticCategory::Windows,
        },
        code: error.raw_os_error(),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct SignInView {
    pub(super) on: bool,
    pub(super) message: Option<FeatureMessage>,
}

pub(super) fn sign_in_view(
    preference: Option<bool>,
    status: SignInStatus,
    text: &Text,
) -> SignInView {
    let on = match (status.effective, status.failure) {
        (Some(Ok(effective)), _) => effective == Effective::Enabled,
        (_, Some((requested, _))) => !requested,
        _ => preference.unwrap_or_default(),
    };
    let warning = |text: String| {
        Some(FeatureMessage {
            severity: Severity::Warning,
            text,
            retry: None,
        })
    };
    let message = match (status.failure, status.effective) {
        (Some((_, failure)), _) => Some(FeatureMessage {
            severity: Severity::Error,
            text: problems::with_code(
                text,
                text!(
                    text,
                    "sign-in-failed",
                    state = if on { "on" } else { "off" }
                ),
                failure,
            ),
            retry: Some(text!(text, "action-retry")),
        }),
        (None, Some(Err(failure))) => Some(FeatureMessage {
            severity: Severity::Error,
            text: problems::with_code(text, text!(text, "sign-in-read-failed"), failure),
            retry: None,
        }),
        (None, Some(Ok(Effective::DisabledByWindows))) if preference == Some(true) => {
            warning(text!(text, "sign-in-disabled-by-windows"))
        }
        (None, Some(Ok(Effective::NotRegistered))) if preference == Some(true) => {
            warning(text!(text, "sign-in-not-registered"))
        }
        (None, Some(Ok(Effective::DifferentCommand))) if preference == Some(true) => {
            warning(text!(text, "sign-in-other-location"))
        }
        (None, Some(Ok(Effective::Enabled))) if preference == Some(false) => {
            warning(text!(text, "sign-in-still-registered"))
        }
        (None, _) => None,
    };
    SignInView { on, message }
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub(in crate::ui) struct FakeRegistration(pub(in crate::ui) Rc<RefCell<FakeState>>);

#[cfg(test)]
#[derive(Debug)]
pub(in crate::ui) struct FakeState {
    pub(in crate::ui) effective: Effective,
    pub(in crate::ui) failure: Option<i32>,
    pub(in crate::ui) read_failure: Option<i32>,
    pub(in crate::ui) sets: Vec<bool>,
}

#[cfg(test)]
impl Default for FakeState {
    fn default() -> Self {
        Self {
            effective: Effective::NotRegistered,
            failure: None,
            read_failure: None,
            sets: Vec::new(),
        }
    }
}

#[cfg(test)]
impl Registrar for FakeRegistration {
    fn read(&self) -> io::Result<Effective> {
        let state = self.0.borrow();
        match state.read_failure {
            Some(code) => Err(io::Error::from_raw_os_error(code)),
            None => Ok(state.effective),
        }
    }

    fn set(&self, enabled: bool) -> io::Result<()> {
        let mut state = self.0.borrow_mut();
        state.sets.push(enabled);
        if let Some(code) = state.failure {
            return Err(io::Error::from_raw_os_error(code));
        }
        state.effective = match (enabled, state.effective) {
            (true, Effective::DisabledByWindows) => Effective::DisabledByWindows,
            (true, _) => Effective::Enabled,
            (false, _) => Effective::NotRegistered,
        };
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RestartPrompt {
    pub(super) title: String,
    pub(super) description: String,
    pub(super) action: String,
}

fn language_values() -> Vec<String> {
    std::iter::once(WINDOWS.to_owned())
        .chain(crate::l10n::shipped().iter().map(ToString::to_string))
        .collect()
}

pub(super) fn language_label(text: &Text, language: &str) -> String {
    if language == WINDOWS {
        text!(text, "language-windows")
    } else {
        crate::l10n::language_name(language)
    }
}

pub(super) fn languages(text: &Text) -> Vec<(String, String)> {
    language_values()
        .into_iter()
        .map(|language| {
            let label = language_label(text, &language);
            (language, label)
        })
        .collect()
}

pub(super) fn language_edits(index: Option<usize>) -> Option<Vec<(Field, FieldValue)>> {
    let language = language_values().into_iter().nth(index?)?;
    Some(vec![(
        Field::InterfaceLanguage,
        FieldValue::Language(language),
    )])
}

pub(super) fn restart_prompt(model: &Model, text: &Text) -> Option<RestartPrompt> {
    (model.config().input().interface_language != model.launch_language).then(|| RestartPrompt {
        title: text!(text, "restart-title"),
        description: text!(text, "restart-description"),
        action: text!(text, "action-quit"),
    })
}

pub(super) fn view(settings: &Settings, context: &ViewContext<'_, Settings>) -> Vec<View> {
    let text = &settings.input.text;
    let minimized = text!(text, "start-minimized-title");
    vec![
        widgets::section(text!(text, "section-language"), true),
        language_card(settings, context),
        widgets::section(text!(text, "section-startup"), false),
        widgets::group(vec![
            sign_in_card(settings, context),
            widgets::card(
                Row::new(
                    "\u{E73F}",
                    minimized.clone(),
                    text!(text, "start-minimized-description"),
                )
                .controls(settings.with_feedback(
                    Field::StartMinimized,
                    switch(
                        settings,
                        Field::StartMinimized,
                        settings.input.model.value(Field::StartMinimized)
                            == FieldValue::Switch(true),
                        (automation_id::START_MINIMIZED, &minimized),
                        context,
                    ),
                ))
                .view(settings.stacked()),
            ),
        ]),
        widgets::section(text!(text, "section-night-light"), false),
        widgets::card(
            Row::new(
                "\u{EA18}",
                text!(text, "night-light-title"),
                text!(text, "night-light-description"),
            )
            .controls(vec![widgets::pill(management_label(
                &settings.input.model,
                text,
            ))])
            .view(settings.stacked()),
        ),
        widgets::section(text!(text, "section-troubleshooting"), false),
        widgets::group(vec![
            folder_card(
                settings,
                "\u{E8B7}",
                text!(text, "settings-file-title"),
                text!(text, "settings-file-description"),
                Folder::Data,
                automation_id::OPEN_SETTINGS_FOLDER,
                context,
            ),
            folder_card(
                settings,
                "\u{E8A5}",
                text!(text, "logs-title"),
                text!(text, "logs-description"),
                Folder::Logs,
                automation_id::OPEN_LOGS_FOLDER,
                context,
            ),
        ]),
        widgets::section(text!(text, "section-about"), false),
        about_card(settings, context),
    ]
}

pub(super) fn management_label(model: &Model, text: &Text) -> String {
    schedule::change_line(model, text).unwrap_or_else(|| {
        if model.managing() {
            text!(text, "night-light-managing")
        } else {
            text!(text, "night-light-not-managing")
        }
    })
}

pub(super) fn version_text(text: &Text) -> String {
    text!(text, "about-version", version = env!("CARGO_PKG_VERSION"))
}

fn folder_card(
    settings: &Settings,
    glyph: &'static str,
    title: String,
    description: String,
    folder: Folder,
    id: &str,
    context: &ViewContext<'_, Settings>,
) -> View {
    let text = &settings.input.text;
    let name = text!(text, "open-folder-name", name = title.as_str());
    widgets::card(
        Row::new(glyph, title, description)
            .controls(vec![
                Button::new()
                    .automation_id(id)
                    .automation_name(name)
                    .on_click(context.message(Message::Command(Command::OpenFolder(folder))))
                    .content(text!(text, "action-open-folder"))
                    .into(),
            ])
            .view(settings.stacked()),
    )
}

fn about_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let text = &settings.input.text;
    widgets::card(
        Row::new("", text!(text, "app-name"), version_text(text))
            .icon(
                Image::new()
                    .source_svg(EncodedImage::from_static(APP_ICON))
                    .width(32.0)
                    .height(32.0)
                    .into(),
            )
            .controls(vec![
                Button::new()
                    .automation_id(automation_id::ABOUT_QUIT)
                    .on_click(context.message(Message::Command(Command::Quit)))
                    .content(text!(text, "action-quit"))
                    .into(),
            ])
            .view(settings.stacked()),
    )
}

fn switch(
    settings: &Settings,
    field: Field,
    on: bool,
    (id, name): (&str, &str),
    context: &ViewContext<'_, Settings>,
) -> View {
    ToggleSwitch::new()
        .is_on(on)
        .is_enabled(settings.input.model.editable())
        .automation_id(id)
        .automation_name(name)
        .on_toggled(
            context.callback(move |on| Message::Edit(vec![(field, FieldValue::Switch(on))])),
        )
        .into()
}

fn sign_in_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let text = &settings.input.text;
    let title = text!(text, "sign-in-title");
    let model = &settings.input.model;
    let view = sign_in_view(
        model
            .accepted
            .as_ref()
            .map(|config| config.input().start_at_sign_in),
        settings.input.sign_in,
        text,
    );
    let mut rows = vec![
        Row::new(
            "\u{E7E8}",
            title.clone(),
            text!(text, "sign-in-description"),
        )
        .controls(settings.with_feedback(
            Field::StartAtSignIn,
            switch(
                settings,
                Field::StartAtSignIn,
                view.on,
                (automation_id::SIGN_IN, &title),
                context,
            ),
        ))
        .view(settings.stacked()),
    ];
    if let Some(message) = view.message {
        rows.push(widgets::feature_row(
            message,
            automation_id::SIGN_IN,
            settings
                .input
                .model
                .editable()
                .then(|| context.message(Message::Command(Command::RetrySignIn))),
            settings.stacked(),
        ));
    }
    widgets::card(StackPanel::new().children(rows))
}

fn language_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let (text, model) = (&settings.input.text, &settings.input.model);
    let title = text!(text, "language-title");
    let options = languages(text);
    let selected = match model.value(Field::InterfaceLanguage) {
        FieldValue::Language(language) => options.iter().position(|(value, _)| *value == language),
        _ => None,
    };
    let combo = ComboBox::new()
        .items_source(options.iter().map(|(_, label)| label.as_str()))
        .selected_index(selected)
        .is_enabled(model.editable())
        .automation_id(automation_id::LANGUAGE)
        .automation_name(&title)
        .on_selection_changed(context.callback(Message::Language))
        .into();
    let mut rows = vec![
        Row::new("\u{F2B7}", title, text!(text, "language-description"))
            .controls(settings.with_feedback(Field::InterfaceLanguage, combo))
            .view(settings.stacked()),
    ];
    if let Some(prompt) = restart_prompt(model, text) {
        rows.push(widgets::nested_row(
            FontIcon::new()
                .glyph("\u{E946}")
                .foreground(ThemeBrush::AccentText),
            vec![
                widgets::body(prompt.title).into(),
                widgets::caption(prompt.description).into(),
            ],
            Some(
                Button::new()
                    .automation_id(automation_id::RESTART_QUIT)
                    .on_click(context.message(Message::Command(Command::Quit)))
                    .content(prompt.action)
                    .into(),
            ),
            automation_id::RESTART_PROMPT,
            settings.stacked(),
        ));
    }
    widgets::card(StackPanel::new().children(rows))
}

#[cfg(test)]
mod tests;
