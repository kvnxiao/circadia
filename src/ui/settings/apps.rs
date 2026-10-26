use super::Command;
use super::Message;
use super::Model;
use super::Settings;
use super::problems;
use super::widgets;
use super::widgets::Row;
use crate::ManualCommand;
use crate::l10n::Text;
use crate::l10n::text;
use crate::platform::nls::RegionalFormat;
use crate::ui::status;
use windows_reactor::Button;
use windows_reactor::FontIcon;
use windows_reactor::Orientation;
use windows_reactor::StackPanel;
use windows_reactor::View;
use windows_reactor::ViewContext;

const SUN_GLYPH: &str = "\u{E706}";

pub(super) fn view(settings: &Settings, context: &ViewContext<'_, Settings>) -> Vec<View> {
    let text = &settings.input.text;
    let manual = manual_view(&settings.input.model, text, &settings.input.region);
    vec![widgets::group(vec![widgets::card(
        Row::new(
            SUN_GLYPH,
            text!(text, "manual-title"),
            manual.description.clone(),
        )
        .controls(vec![manual_button(manual, context)])
        .view(),
    )])]
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
        .automation_id("manual-suppression")
        .automation_name(&manual.label)
        .on_click(context.message(Message::Command(Command::Manual(manual.command))))
        .content(
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(8.0)
                .children((
                    FontIcon::new().glyph(SUN_GLYPH).font_size(16.0),
                    widgets::body(manual.label),
                )),
        )
        .into()
}

#[cfg(test)]
mod tests;
