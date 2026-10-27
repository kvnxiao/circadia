use std::sync::Arc;
use windows_reactor::AutomationLiveSetting;
use windows_reactor::Border;
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
use windows_reactor::VerticalAlignment;
use windows_reactor::View;

pub(super) const APP_GLYPH: &str = "\u{ECAA}";

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

pub(super) fn pill(text: impl AsRef<str>) -> View {
    Border::new()
        .height(22.0)
        .corner_radius(11.0)
        .padding(Thickness::new(8.0, 0.0, 8.0, 0.0))
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(1.0)
        .vertical_alignment(VerticalAlignment::Center)
        .content(caption(text).vertical_alignment(VerticalAlignment::Center))
        .into()
}

pub(super) fn nested_row(glyph: FontIcon, text: Vec<View>, action: Option<View>, id: &str) -> View {
    Border::new()
        .min_height(56.0)
        .padding(Thickness::new(52.0, 10.0, 16.0, 10.0))
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(Thickness::new(0.0, 1.0, 0.0, 0.0))
        .automation_id(id)
        .automation_live_setting(AutomationLiveSetting::Assertive)
        .content(
            Grid::new()
                .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                .column_spacing(12.0)
                .children((
                    glyph
                        .font_size(16.0)
                        .vertical_alignment(VerticalAlignment::Top)
                        .margin(Thickness::new(0.0, 2.0, 0.0, 0.0)),
                    StackPanel::new()
                        .grid_column(1)
                        .spacing(2.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(text),
                    StackPanel::new()
                        .grid_column(2)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(action.into_iter().collect::<Vec<_>>()),
                )),
        )
        .into()
}

pub(super) fn group(cards: Vec<View>) -> View {
    StackPanel::new().spacing(4.0).children(cards).into()
}

pub(super) struct Row {
    glyph: &'static str,
    icon: Option<View>,
    title: String,
    description: String,
    detail: Option<View>,
    controls: Vec<View>,
}

impl Row {
    pub(super) fn new(
        glyph: &'static str,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            glyph,
            icon: None,
            title: title.into(),
            description: description.into(),
            detail: None,
            controls: Vec::new(),
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

    pub(super) fn icon(mut self, icon: View) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(super) fn view(self) -> View {
        let top = self.controls.len() > 1;
        let alignment = if top {
            VerticalAlignment::Top
        } else {
            VerticalAlignment::Center
        };
        let offset = if top { 6.0 } else { 0.0 };
        Border::new()
            .min_height(68.0)
            .padding(Thickness::new(16.0, 12.0, 16.0, 12.0))
            .content(
                Grid::new()
                    .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                    .children((
                        Border::new()
                            .vertical_alignment(alignment)
                            .margin(Thickness::new(0.0, offset, 16.0, 0.0))
                            .content(self.icon.unwrap_or_else(|| {
                                FontIcon::new().glyph(self.glyph).font_size(20.0).into()
                            })),
                        StackPanel::new()
                            .grid_column(1)
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
                            .grid_column(2)
                            .vertical_alignment(alignment)
                            .horizontal_alignment(HorizontalAlignment::Right)
                            .margin(Thickness::new(16.0, 0.0, 0.0, 0.0))
                            .children(self.controls),
                    )),
            )
            .into()
    }
}
