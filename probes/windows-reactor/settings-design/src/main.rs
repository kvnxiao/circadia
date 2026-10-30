//! Render every settings page, dialog, and tray menu item with fixture data.

mod fixture;
mod graph;
mod icon;
mod kelvin;
mod l10n;
mod log;
mod motion;
mod palette;
mod phase;
mod progress;
mod region;
mod settings;
mod system;
mod time_picker;
mod widgets;

use anyhow::Context;
use anyhow::anyhow;
use anyhow::bail;
use fixture::Schedule;
use graph::Renderer;
use i18n_embed_fl::fl;
use l10n::LOADER;
use l10n::Language;
use phase::GlyphColor;
use region::Region;
use settings::Dialog;
use settings::LISTED;
use settings::Options;
use settings::Page;
use settings::RECENT;
use settings::Settings;
use settings::SettingsInput;
use settings::Status;
use std::path::PathBuf;
use std::rc::Rc;
use time_picker::Closing;
use time_picker::ListOrder;
use windows_reactor::App;
use windows_reactor::Application;
use windows_reactor::ApplicationContext;
use windows_reactor::ApplicationView;
use windows_reactor::ApplicationViewContext;
use windows_reactor::Key;
use windows_reactor::Menu;
use windows_reactor::MenuItem;
use windows_reactor::NotifyIcon;
use windows_reactor::WindowTheme;

struct AppInput {
    icon: PathBuf,
    open: bool,
    options: Rc<Options>,
    region: Rc<Region>,
}

#[derive(Clone)]
enum Message {
    Open,
    ToggleManual,
    Quit,
    Menu(Key),
}

struct Probe {
    manual: bool,
}

impl Probe {
    fn menu(&self) -> Vec<MenuItem> {
        let suppress = if self.manual {
            fl!(LOADER, "suppress-stop")
        } else {
            fl!(LOADER, "suppress-hour")
        };
        let mut items = vec![
            MenuItem::item("open", fl!(LOADER, "tray-open")),
            MenuItem::separator("open-separator"),
            MenuItem::item("manual", suppress),
            MenuItem::separator("manual-separator"),
            MenuItem::disabled("caption", fl!(LOADER, "tray-caption")),
        ];
        for (index, (app, listed)) in RECENT.iter().zip(LISTED).take(3).enumerate() {
            let key = format!("app-{index}");
            items.push(if app.excluded {
                MenuItem::disabled(
                    key,
                    fl!(LOADER, "tray-app-excluded", name = app.name, exe = app.exe),
                )
            } else if listed {
                MenuItem::item(
                    key,
                    fl!(LOADER, "tray-app-listed", name = app.name, exe = app.exe),
                )
            } else {
                MenuItem::item(key, fl!(LOADER, "tray-app", name = app.name, exe = app.exe))
            });
        }
        items.push(MenuItem::separator("quit-separator"));
        items.push(MenuItem::item("quit", fl!(LOADER, "quit")));
        items
    }
}

impl Application for Probe {
    type Input = AppInput;
    type Message = Message;

    fn create(input: &AppInput, context: &ApplicationContext<Self>) -> Self {
        if input.open {
            context.show_window("settings");
        }
        Self {
            manual: input.options.status == Status::Manual,
        }
    }

    fn update(&mut self, message: Message, context: &ApplicationContext<Self>) {
        match message {
            Message::Open => context.show_window("settings"),
            Message::ToggleManual => self.manual = !self.manual,
            Message::Quit => context.exit(),
            Message::Menu(key) => match key.as_str() {
                Some("open") => context.show_window("settings"),
                Some("manual") => self.manual = !self.manual,
                Some("quit") => context.exit(),
                Some(other) => log::write(&format!("tray menu item: {other}")),
                None => {}
            },
        }
    }

    fn view(&self, input: &AppInput, context: &ApplicationViewContext<Self>) -> ApplicationView {
        let schedule = Schedule::default();
        let kelvin = num_traits::ToPrimitive::to_u32(&schedule.kelvin_at(input.options.now))
            .and_then(|kelvin| input.region.whole(kelvin).ok())
            .unwrap_or_default();
        let status = if self.manual {
            fl!(
                LOADER,
                "status-suppressed",
                kelvin = input.region.whole(6500).unwrap_or_default()
            )
        } else {
            fl!(LOADER, "status-managing", kelvin = kelvin)
        };
        ApplicationView::new()
            .window::<Settings>(
                "settings",
                SettingsInput {
                    options: Rc::clone(&input.options),
                    region: Rc::clone(&input.region),
                    manual: self.manual,
                    toggle_manual: context.message(Message::ToggleManual),
                    quit: context.message(Message::Quit),
                },
            )
            .notify_icon(
                "tray",
                NotifyIcon::new(input.icon.clone())
                    .tooltip(fl!(LOADER, "tooltip", status = status))
                    .on_activate(context.callback(|_| Message::Open))
                    .menu(Menu::new(self.menu(), context.callback(Message::Menu))),
            )
    }
}

struct Arguments {
    options: Options,
    language: Language,
    region: Option<String>,
    ui_language: Option<String>,
    open: bool,
}

fn parse_clock(text: &str) -> Option<i32> {
    let (hour, minute) = text.split_once(':')?;
    let hour: i32 = hour.parse().ok()?;
    let minute: i32 = minute.parse().ok()?;
    ((0..24).contains(&hour) && (0..60).contains(&minute)).then_some(hour * 60 + minute)
}

fn parse_arguments(arguments: impl IntoIterator<Item = String>) -> anyhow::Result<Arguments> {
    let mut parsed = Arguments {
        options: Options {
            theme: WindowTheme::System,
            renderer: Renderer::Composition,
            closing: Closing::Detach,
            order: ListOrder::Centered,
            page: Page::Schedule,
            dialog: None,
            status: Status::Managing,
            now: 22 * 60 + 15,
            banners: false,
            invalid: false,
            glyph: GlyphColor::Substituted,
        },
        language: Language::English,
        region: None,
        ui_language: None,
        open: false,
    };
    let mut arguments = arguments.into_iter();
    while let Some(flag) = arguments.next() {
        match flag.as_str() {
            "--open" => parsed.open = true,
            "--banners" => parsed.options.banners = true,
            "--invalid" => parsed.options.invalid = true,
            _ => {
                let value = arguments
                    .next()
                    .with_context(|| format!("{flag} needs a value"))?;
                let invalid = || anyhow!("{flag} does not accept {value}");
                let options = &mut parsed.options;
                match flag.as_str() {
                    "--theme" => {
                        options.theme = match value.as_str() {
                            "dark" => WindowTheme::Dark,
                            "light" => WindowTheme::Light,
                            "system" => WindowTheme::System,
                            _ => return Err(invalid()),
                        };
                    }
                    "--renderer" => {
                        options.renderer = Renderer::parse(&value).ok_or_else(invalid)?;
                    }
                    "--close" => options.closing = Closing::parse(&value).ok_or_else(invalid)?,
                    "--order" => {
                        options.order = match value.as_str() {
                            "natural" => ListOrder::Natural,
                            "centered" => ListOrder::Centered,
                            _ => return Err(invalid()),
                        };
                    }
                    "--page" => options.page = Page::parse(&value).ok_or_else(invalid)?,
                    "--dialog" => {
                        options.dialog = Some(match value.as_str() {
                            "consent" => Dialog::Consent,
                            "add" => Dialog::AddApp,
                            _ => return Err(invalid()),
                        });
                    }
                    "--status" => options.status = Status::parse(&value).ok_or_else(invalid)?,
                    "--now" => options.now = parse_clock(&value).ok_or_else(invalid)?,
                    "--glyph" => {
                        options.glyph = match value.as_str() {
                            "raw" => GlyphColor::Raw,
                            "substituted" => GlyphColor::Substituted,
                            _ => return Err(invalid()),
                        };
                    }
                    "--language" => {
                        parsed.language = Language::parse(&value).ok_or_else(invalid)?;
                    }
                    "--region" => parsed.region = Some(value.clone()),
                    "--ui-language" => parsed.ui_language = Some(value.clone()),
                    _ => bail!("unknown argument {flag}"),
                }
            }
        }
    }
    Ok(parsed)
}

fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--motion") {
        return motion::run();
    }
    if std::env::args().nth(1).as_deref() == Some("--progress") {
        return progress::run();
    }
    let arguments = parse_arguments(std::env::args().skip(1))?;
    if let Some(tag) = &arguments.ui_language {
        system::prefer_ui_language(tag).with_context(|| format!("preferring UI language {tag}"))?;
    }
    l10n::load(arguments.language)
        .with_context(|| format!("loading {} catalogs", arguments.language.tag()))?;
    let region = Region::load(arguments.region.as_deref()).context("reading regional formats")?;
    log::write(&format!(
        "start: language {}, region {}, renderer {}, closing {}",
        arguments.language.tag(),
        region.label(),
        arguments.options.renderer.name(),
        arguments.options.closing.name()
    ));
    App::run_application::<Probe>(AppInput {
        icon: std::env::current_exe()?.with_file_name("circadia.ico"),
        open: arguments.open,
        options: Rc::new(arguments.options),
        region: Rc::new(region),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("00:00", Some(0))]
    #[test_case("22:15", Some(1335))]
    #[test_case("23:59", Some(1439))]
    #[test_case("24:00", None)]
    #[test_case("7", None)]
    #[test_case("07:60", None)]
    fn clock_arguments_parse_to_minutes(text: &str, expected: Option<i32>) {
        assert_eq!(parse_clock(text), expected);
    }

    #[test]
    fn unknown_values_are_rejected() {
        let error = parse_arguments(["--renderer".to_owned(), "gdi".to_owned()])
            .err()
            .map(|error| error.to_string());
        assert_eq!(error.as_deref(), Some("--renderer does not accept gdi"));
    }
}
