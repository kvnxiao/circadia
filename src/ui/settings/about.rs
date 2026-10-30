use super::Command;
use super::Message;
use super::Settings;
use super::general;
use super::widgets;
use crate::automation_id;
use crate::l10n::Text;
use crate::l10n::bind_text;
use crate::updates::Status;
use crate::updates::{self};
use windows_reactor::AutomationLiveSetting;
use windows_reactor::Button;
use windows_reactor::ButtonStyle;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::HyperlinkButton;
use windows_reactor::InfoBar;
use windows_reactor::InfoBarSeverity;
use windows_reactor::Orientation;
use windows_reactor::ProgressRing;
use windows_reactor::StackPanel;
use windows_reactor::Thickness;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;

pub(super) fn version_text(text: &Text) -> String {
    bind_text!(fl, text);
    fl!(
        "about-version",
        version = updates::version(),
        commit = env!("CIRCADIA_COMMIT"),
        architecture = match env!("CIRCADIA_ARCH") {
            "x86_64" => "x64",
            other => other,
        }
    )
}

fn identity(text: &Text) -> View {
    bind_text!(fl, text);
    Grid::new()
        .columns([GridLength::Auto, GridLength::STAR])
        .column_spacing(20.0)
        .margin(Thickness::new(0.0, 8.0, 0.0, 20.0))
        .children((
            widgets::AppIcon::Circadia.view(64.0),
            StackPanel::new()
                .grid_column(1)
                .vertical_alignment(VerticalAlignment::Center)
                .spacing(4.0)
                .children((
                    widgets::title(fl!("app-name")).font_size(24.0),
                    widgets::caption(version_text(text))
                        .automation_id(automation_id::ABOUT_VERSION),
                )),
        ))
        .into()
}

pub(super) fn view(settings: &Settings, context: &ViewContext<'_, Settings>) -> Vec<View> {
    let text = &settings.input.text;
    bind_text!(fl, text);
    let checking = settings.input.updates == Status::Checking;
    let mut result = vec![
        identity(text),
        Button::new()
            .style(ButtonStyle::Accent)
            .horizontal_alignment(HorizontalAlignment::Left)
            .automation_id(automation_id::CHECK_UPDATES)
            .is_enabled(!checking)
            .on_click(context.message(Message::Command(Command::CheckUpdates)))
            .content(widgets::body(fl!("updates-check")))
            .into(),
    ];
    result.push(banner(settings, context).unwrap_or_else(|| windows_reactor::Border::new().into()));
    let title = fl!("updates-automatic");
    result.extend([
        widgets::section(fl!("section-updates"), false),
        widgets::card(
            widgets::Row::new(
                "\u{E777}",
                title.clone(),
                fl!("updates-automatic-description"),
            )
            .controls(
                settings.with_feedback(
                    crate::Field::CheckUpdatesAutomatically,
                    general::switch(
                        settings,
                        crate::Field::CheckUpdatesAutomatically,
                        settings
                            .input
                            .model
                            .value(crate::Field::CheckUpdatesAutomatically)
                            == crate::FieldValue::Switch(true),
                        (automation_id::AUTOMATIC_UPDATES, &title),
                        context,
                    ),
                ),
            )
            .compact_actions()
            .view(settings.stacked()),
        ),
        widgets::section(fl!("section-feedback"), false),
        widgets::card(
            widgets::Row::new(
                "\u{EBE8}",
                fl!("about-report"),
                fl!("about-report-description"),
            )
            .controls(vec![
                HyperlinkButton::new()
                    .automation_id(automation_id::REPORT_BUG)
                    .automation_name(fl!("about-report"))
                    .on_click(context.message(Message::Command(Command::Report(format!(
                        "{}/issues/new?template=bug_report.md",
                        updates::REPOSITORY
                    )))))
                    .content(
                        windows_reactor::FontIcon::new()
                            .glyph("\u{E8A7}")
                            .font_size(14.0),
                    )
                    .into(),
            ])
            .compact_actions()
            .view(settings.stacked()),
        ),
        widgets::section(fl!("section-other-links"), false),
        links(settings, context),
    ]);
    result
}

fn links(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let text = &settings.input.text;
    bind_text!(fl, text);
    StackPanel::new()
        .orientation(if settings.stacked() {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        })
        .horizontal_alignment(HorizontalAlignment::Left)
        .spacing(8.0)
        .children(
            [
                (fl!("about-repository"), "", automation_id::ABOUT_REPOSITORY),
                (
                    fl!("about-contributing"),
                    "/blob/main/CONTRIBUTING.md",
                    automation_id::ABOUT_CONTRIBUTING,
                ),
                (
                    fl!("about-license"),
                    "/blob/main/LICENSE",
                    automation_id::ABOUT_LICENSE,
                ),
            ]
            .into_iter()
            .map(|(label, path, id)| {
                HyperlinkButton::new()
                    .automation_id(id)
                    .horizontal_alignment(HorizontalAlignment::Left)
                    .on_click(context.message(Message::Command(Command::Report(format!(
                        "{}{path}",
                        updates::REPOSITORY
                    )))))
                    .content(widgets::body(label))
                    .into()
            })
            .collect::<Vec<View>>(),
        )
        .into()
}

fn banner(settings: &Settings, context: &ViewContext<'_, Settings>) -> Option<View> {
    let text = &settings.input.text;
    bind_text!(fl, text);
    let (title, severity) = match &settings.input.updates {
        Status::Idle => return None,
        Status::Checking => (fl!("updates-checking"), InfoBarSeverity::Informational),
        Status::Current => (fl!("updates-current"), InfoBarSeverity::Success),
        Status::Available(_) => (fl!("updates-available"), InfoBarSeverity::Informational),
        Status::Unpublished => (fl!("updates-unpublished"), InfoBarSeverity::Informational),
        Status::Development => (fl!("updates-development"), InfoBarSeverity::Informational),
        Status::Failed => (fl!("updates-failed"), InfoBarSeverity::Error),
    };
    let mut children: Vec<View> = vec![
        InfoBar::new()
            .automation_id(automation_id::UPDATE_STATUS)
            .automation_live_setting(AutomationLiveSetting::Polite)
            .is_open(true)
            .is_closable(false)
            .severity(severity)
            .title(title)
            .into(),
    ];
    if settings.input.updates == Status::Checking {
        children.push(
            ProgressRing::new()
                .width(20.0)
                .height(20.0)
                .is_active(settings.visibility == super::Visibility::Shown)
                .into(),
        );
    }
    if let Status::Available(url) = &settings.input.updates {
        if env!("CIRCADIA_CHANNEL") == "preview" {
            children.push(widgets::caption(fl!("updates-preview-download")).into());
        }
        children.push(
            HyperlinkButton::new()
                .automation_id(automation_id::UPDATE_DOWNLOAD)
                .horizontal_alignment(HorizontalAlignment::Left)
                .on_click(context.message(Message::Command(Command::Report(url.clone()))))
                .content(widgets::body(fl!("updates-download")))
                .into(),
        );
    }
    Some(
        StackPanel::new()
            .spacing(8.0)
            .margin(Thickness::new(0.0, 12.0, 0.0, 0.0))
            .children(children)
            .into(),
    )
}
