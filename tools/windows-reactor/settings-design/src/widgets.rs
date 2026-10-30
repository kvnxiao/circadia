use crate::palette::Palette;
use windows_reactor::Border;
use windows_reactor::FontIcon;
use windows_reactor::FontWeight;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::NumberBox;
use windows_reactor::Orientation;
use windows_reactor::StackPanel;
use windows_reactor::TextBlock;
use windows_reactor::TextWrapping;
use windows_reactor::ThemeBrush;
use windows_reactor::Thickness;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;

pub(crate) fn caption(text: impl AsRef<str>) -> TextBlock {
    TextBlock::new()
        .text(text)
        .font_size(12.0)
        .line_height(16.0)
        .text_wrapping(TextWrapping::Wrap)
}

pub(crate) fn body(text: impl AsRef<str>) -> TextBlock {
    TextBlock::new()
        .text(text)
        .font_size(14.0)
        .line_height(20.0)
        .text_wrapping(TextWrapping::Wrap)
}

pub(crate) fn strong(text: impl AsRef<str>) -> TextBlock {
    body(text).font_weight(FontWeight::SEMI_BOLD)
}

pub(crate) fn title(text: impl AsRef<str>) -> TextBlock {
    TextBlock::new()
        .text(text)
        .font_size(28.0)
        .line_height(36.0)
        .font_weight(FontWeight::SEMI_BOLD)
        .text_wrapping(TextWrapping::Wrap)
}

pub(crate) fn secondary(block: TextBlock, palette: &Palette) -> TextBlock {
    block.foreground(palette.secondary)
}

pub(crate) fn glyph(code: &str, size: f64) -> FontIcon {
    FontIcon::new().glyph(code).font_size(size)
}

pub(crate) fn section(text: impl AsRef<str>, first: bool) -> View {
    strong(text)
        .margin(Thickness::new(
            0.0,
            if first { 0.0 } else { 28.0 },
            0.0,
            8.0,
        ))
        .into()
}

pub(crate) fn card(content: impl Into<View>) -> Border {
    Border::new()
        .background(ThemeBrush::CardBackground)
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(1.0)
        .corner_radius(4.0)
        .content(content)
}

pub(crate) fn group(cards: Vec<View>) -> View {
    StackPanel::new().spacing(4.0).children(cards).into()
}

pub(crate) struct Row {
    pub(crate) glyph: Option<View>,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) controls: Option<View>,
}

impl Row {
    pub(crate) fn new(glyph: &str, title: impl Into<String>) -> Self {
        Self {
            glyph: Some(self::glyph(glyph, 20.0).into()),
            title: title.into(),
            description: None,
            controls: None,
        }
    }

    pub(crate) fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub(crate) fn controls(mut self, controls: impl Into<View>) -> Self {
        self.controls = Some(controls.into());
        self
    }

    pub(crate) fn view(self, palette: &Palette) -> Border {
        let mut text = StackPanel::new().spacing(2.0).children((body(self.title),));
        if let Some(description) = self.description {
            text = StackPanel::new()
                .spacing(2.0)
                .children((text, secondary(caption(description), palette)));
        }
        let mut children: Vec<View> = Vec::new();
        if let Some(glyph) = self.glyph {
            children.push(
                Border::new()
                    .vertical_alignment(VerticalAlignment::Center)
                    .margin(Thickness::new(0.0, 0.0, 16.0, 0.0))
                    .content(glyph)
                    .into(),
            );
        }
        children.push(
            text.grid_column(1)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        );
        if let Some(controls) = self.controls {
            children.push(
                Border::new()
                    .grid_column(2)
                    .vertical_alignment(VerticalAlignment::Center)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .margin(Thickness::new(16.0, 0.0, 0.0, 0.0))
                    .content(controls)
                    .into(),
            );
        }
        Border::new()
            .min_height(68.0)
            .padding(Thickness::new(16.0, 12.0, 16.0, 12.0))
            .content(
                Grid::new()
                    .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                    .children(children),
            )
    }
}

pub(crate) fn nested(content: impl Into<View>, palette: &Palette) -> View {
    Border::new()
        .border_brush(palette.divider)
        .border_thickness(Thickness::new(0.0, 1.0, 0.0, 0.0))
        .min_height(56.0)
        .padding(Thickness::new(52.0, 10.0, 16.0, 10.0))
        .content(content)
        .into()
}

pub(crate) fn number(
    id: &str,
    name: &str,
    value: f64,
    range: (f64, f64),
    step: f64,
    unit: &str,
    palette: &Palette,
) -> StackPanel {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(8.0)
        .children((
            NumberBox::new()
                .width(96.0)
                .minimum(range.0)
                .maximum(range.1)
                .small_change(step)
                .value(Some(value))
                .automation_id(id)
                .automation_name(name),
            secondary(body(unit), palette).vertical_alignment(VerticalAlignment::Center),
        ))
}

pub(crate) fn pill(text: impl AsRef<str>, palette: &Palette) -> Border {
    Border::new()
        .height(22.0)
        .corner_radius(11.0)
        .padding(Thickness::new(8.0, 0.0, 8.0, 0.0))
        .background(palette.subtle_fill)
        .vertical_alignment(VerticalAlignment::Center)
        .content(caption(text).vertical_alignment(VerticalAlignment::Center))
}
