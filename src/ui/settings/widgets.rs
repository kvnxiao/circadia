use super::problems::FeatureMessage;
use super::problems::Severity;
use crate::automation_id;
use crate::l10n::Text;
use crate::l10n::bind_text;
use std::sync::Arc;
use windows_reactor::AutomationLiveSetting;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::ButtonStyle;
use windows_reactor::Callback;
use windows_reactor::EncodedImage;
use windows_reactor::FontIcon;
use windows_reactor::FontWeight;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::Image;
use windows_reactor::Orientation;
use windows_reactor::StackPanel;
use windows_reactor::Stretch;
use windows_reactor::TextBlock;
use windows_reactor::TextWrapping;
use windows_reactor::ThemeBrush;
use windows_reactor::Thickness;
use windows_reactor::ToggleSwitch;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;

pub(super) const APP_GLYPH: &str = "\u{ECAA}";
pub(super) const ICON_ACTION_SIZE: f64 = 40.0;
pub(super) const CONTENT_INSET: f64 = 16.0;
pub(super) const COMPACT_ACTION_INSET: f64 = CONTENT_INSET - (ICON_ACTION_SIZE - 16.0) / 2.0;

pub(super) fn toggle(text: &Text, on: bool, control: ToggleSwitch) -> StackPanel {
    bind_text!(fl, text);
    let label = fl!("toggle-state", on = if on { "yes" } else { "no" });
    action_group(vec![
        body(label)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
        control
            // Empty native switch content still reserves a 12 epx trailing column.
            .min_width(0.0)
            .on_content("")
            .off_content("")
            .is_on(on)
            .into(),
    ])
}

pub(super) fn glyph_label(glyph: &'static str, label: impl AsRef<str>) -> StackPanel {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(8.0)
        .children((FontIcon::new().glyph(glyph).font_size(16.0), body(label)))
}

pub(super) fn action_group(actions: Vec<View>) -> StackPanel {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .horizontal_alignment(HorizontalAlignment::Right)
        .vertical_alignment(VerticalAlignment::Center)
        .spacing(12.0)
        .children(actions)
}

pub(super) fn icon_button(glyph: &'static str, size: f64) -> Button {
    Button::new()
        .style(ButtonStyle::Subtle)
        .min_width(ICON_ACTION_SIZE)
        .min_height(ICON_ACTION_SIZE)
        .horizontal_content_alignment(HorizontalAlignment::Center)
        .vertical_content_alignment(VerticalAlignment::Center)
        .content(FontIcon::new().glyph(glyph).font_size(size))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AppIcon {
    Circadia,
    Image(Arc<[u8]>),
    Placeholder,
}

impl AppIcon {
    pub(super) fn view(&self, size: f64) -> View {
        let image = match self {
            Self::Circadia => Image::new().source_svg(EncodedImage::from_static(super::APP_ICON)),
            Self::Image(png) => Image::new().source_data(EncodedImage::new(Arc::clone(png))),
            Self::Placeholder => {
                return FontIcon::new()
                    .glyph(APP_GLYPH)
                    .font_size(size)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into();
            }
        };
        image
            .width(size)
            .height(size)
            .stretch(Stretch::Uniform)
            .vertical_alignment(VerticalAlignment::Center)
            .into()
    }
}

pub(super) fn caption(text: impl AsRef<str>) -> TextBlock {
    TextBlock::new()
        .text(text)
        .font_size(12.0)
        .line_height(16.0)
        .text_wrapping(TextWrapping::Wrap)
}

pub(super) fn body(text: impl AsRef<str>) -> TextBlock {
    TextBlock::new()
        .text(text)
        .font_size(14.0)
        .line_height(20.0)
        .text_wrapping(TextWrapping::Wrap)
}

pub(super) fn strong(text: impl AsRef<str>) -> TextBlock {
    body(text).font_weight(FontWeight::SEMI_BOLD)
}

pub(super) fn title(text: impl AsRef<str>) -> TextBlock {
    TextBlock::new()
        .text(text)
        .font_size(28.0)
        .line_height(36.0)
        .font_weight(FontWeight::SEMI_BOLD)
        .text_wrapping(TextWrapping::Wrap)
}

pub(super) fn section(text: impl AsRef<str>, first: bool) -> View {
    strong(text)
        .margin(Thickness::new(
            0.0,
            if first { 0.0 } else { 28.0 },
            0.0,
            8.0,
        ))
        .into()
}

pub(super) fn card(content: impl Into<View>) -> View {
    Border::new()
        .background(ThemeBrush::CardBackground)
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(1.0)
        .corner_radius(4.0)
        .content(content)
        .into()
}

pub(super) fn field_error(text: impl AsRef<str>, id: &str) -> View {
    caption(text)
        .foreground(ThemeBrush::SystemCritical)
        .max_width(360.0)
        .horizontal_alignment(HorizontalAlignment::Right)
        .margin(Thickness::new(0.0, 4.0, 0.0, 0.0))
        .automation_id(id)
        .automation_live_setting(AutomationLiveSetting::Assertive)
        .into()
}

pub(super) fn field_note(text: impl AsRef<str>, id: &str) -> View {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(4.0)
        .horizontal_alignment(HorizontalAlignment::Right)
        .margin(Thickness::new(0.0, 4.0, 0.0, 0.0))
        .children((
            FontIcon::new().glyph("\u{E946}").font_size(12.0),
            caption(text)
                .max_width(340.0)
                .automation_id(id)
                .automation_live_setting(AutomationLiveSetting::Polite),
        ))
        .into()
}

pub(super) fn pill(text: impl AsRef<str>, on: bool, palette: &super::palette::Palette) -> View {
    let (background, foreground) = if on {
        (palette.accent(), palette.on_accent.into())
    } else {
        (palette.status_fill.into(), palette.primary())
    };
    Border::new()
        .min_height(22.0)
        .corner_radius(11.0)
        .padding(Thickness::new(8.0, 2.0, 8.0, 2.0))
        .background(background)
        .border_brush(palette.primary())
        .border_thickness(if palette.contrast().is_some() {
            1.0
        } else {
            0.0
        })
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Center)
        .content(
            caption(text)
                .foreground(foreground)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .into()
}

pub(super) fn nested(id: &str) -> Border {
    Border::new()
        .min_height(56.0)
        .padding(Thickness::new(52.0, 10.0, 16.0, 10.0))
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(Thickness::new(0.0, 1.0, 0.0, 0.0))
        .automation_id(id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Expansion {
    Collapsed,
    Expanded,
    Held,
}

impl Expansion {
    pub(super) fn chosen(expanded: bool) -> Self {
        if expanded {
            Self::Expanded
        } else {
            Self::Collapsed
        }
    }

    pub(super) fn held_when(self, held: bool) -> Self {
        if held { Self::Held } else { self }
    }

    pub(super) fn expanded(self) -> bool {
        self != Self::Collapsed
    }
}

pub(super) fn expand_button(
    expansion: Expansion,
    name: String,
    id: &str,
    on_click: Callback<()>,
) -> View {
    icon_button(
        if expansion.expanded() {
            "\u{E70E}"
        } else {
            "\u{E70D}"
        },
        12.0,
    )
    .is_enabled(expansion != Expansion::Held)
    .automation_id(id)
    .automation_name(name)
    .on_click(on_click)
    .into()
}

pub(super) fn nested_row(
    glyph: FontIcon,
    text: Vec<View>,
    action: Option<View>,
    id: &str,
    stacked: bool,
) -> View {
    nested(id)
        .automation_live_setting(AutomationLiveSetting::Assertive)
        .content(
            Grid::new()
                .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                .rows([GridLength::Auto, GridLength::Auto])
                .column_spacing(12.0)
                .row_spacing(if stacked { 12.0 } else { 0.0 })
                .children((
                    glyph
                        .font_size(16.0)
                        .vertical_alignment(VerticalAlignment::Top)
                        .margin(Thickness::new(0.0, 2.0, 0.0, 0.0)),
                    StackPanel::new()
                        .grid_column(1)
                        .grid_column_span(if stacked { 2 } else { 1 })
                        .spacing(2.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(text),
                    StackPanel::new()
                        .grid_column(if stacked { 1 } else { 2 })
                        .grid_row(i32::from(stacked))
                        .grid_column_span(if stacked { 2 } else { 1 })
                        .horizontal_alignment(HorizontalAlignment::Right)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(action.into_iter().collect::<Vec<_>>()),
                )),
        )
        .into()
}

pub(super) fn feature_row(
    message: FeatureMessage,
    name: &str,
    retry: Option<Callback<()>>,
    stacked: bool,
) -> View {
    let glyph = match message.severity {
        Severity::Error => FontIcon::new()
            .glyph("\u{EB90}")
            .foreground(ThemeBrush::SystemCritical),
        Severity::Warning => FontIcon::new().glyph("\u{E7BA}"),
    };
    let action = message.retry.map(|label| {
        let button = Button::new()
            .automation_id(automation_id::retry(name))
            .is_enabled(retry.is_some())
            .content(label);
        match retry {
            Some(retry) => button.on_click(retry).into(),
            None => button.into(),
        }
    });
    nested_row(
        glyph,
        vec![
            body(message.text)
                .automation_id(automation_id::message(name))
                .into(),
        ],
        action,
        &automation_id::row(name),
        stacked,
    )
}

pub(super) fn group(cards: Vec<View>) -> View {
    StackPanel::new().spacing(4.0).children(cards).into()
}

pub(super) struct Row {
    glyph: &'static str,
    title: String,
    description: String,
    detail: Option<View>,
    controls: Vec<View>,
    compact: bool,
    action_inset: f64,
}

impl Row {
    pub(super) fn new(
        glyph: &'static str,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            glyph,
            title: title.into(),
            description: description.into(),
            detail: None,
            controls: Vec::new(),
            compact: false,
            action_inset: CONTENT_INSET,
        }
    }

    pub(super) fn detail(mut self, detail: Option<View>) -> Self {
        self.detail = detail;
        self
    }

    pub(super) fn controls(mut self, controls: Vec<View>) -> Self {
        self.controls = controls;
        self
    }

    pub(super) fn compact_actions(mut self) -> Self {
        self.compact = true;
        self.action_inset = COMPACT_ACTION_INSET;
        self
    }

    pub(super) fn trailing_icon(mut self) -> Self {
        self.action_inset = COMPACT_ACTION_INSET;
        self
    }

    pub(super) fn view(self, stacked: bool) -> View {
        let top = self.controls.len() > 1;
        let stacked = stacked && (!self.compact || top);
        let alignment = if top {
            VerticalAlignment::Top
        } else {
            VerticalAlignment::Center
        };
        let offset = if top { 6.0 } else { 0.0 };
        Border::new()
            .min_height(68.0)
            .padding(Thickness::new(CONTENT_INSET, 12.0, self.action_inset, 12.0))
            .content(
                Grid::new()
                    .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                    .rows([GridLength::Auto, GridLength::Auto])
                    .children((
                        Border::new()
                            .vertical_alignment(alignment)
                            .margin(Thickness::new(0.0, offset, 16.0, 0.0))
                            .content(FontIcon::new().glyph(self.glyph).font_size(20.0)),
                        StackPanel::new()
                            .grid_column(1)
                            .grid_column_span(if stacked { 2 } else { 1 })
                            .spacing(2.0)
                            .vertical_alignment(alignment)
                            .margin(Thickness::new(0.0, offset, 0.0, 0.0))
                            .children(
                                [body(self.title).into(), caption(self.description).into()]
                                    .into_iter()
                                    .chain(self.detail)
                                    .collect::<Vec<View>>(),
                            ),
                        StackPanel::new()
                            .grid_column(if stacked { 1 } else { 2 })
                            .grid_row(i32::from(stacked))
                            .grid_column_span(if stacked { 2 } else { 1 })
                            .vertical_alignment(alignment)
                            .horizontal_alignment(HorizontalAlignment::Right)
                            .margin(Thickness::new(
                                if stacked { 0.0 } else { 16.0 },
                                if stacked && !self.controls.is_empty() {
                                    12.0
                                } else {
                                    0.0
                                },
                                0.0,
                                0.0,
                            ))
                            .children(self.controls),
                    )),
            )
            .into()
    }
}
