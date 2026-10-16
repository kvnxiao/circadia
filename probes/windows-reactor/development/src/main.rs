//! Exercise application state and independent tray and settings lifetimes.

use windows_reactor::App;
use windows_reactor::Application;
use windows_reactor::ApplicationContext;
use windows_reactor::ApplicationView;
use windows_reactor::ApplicationViewContext;
use windows_reactor::Button;
use windows_reactor::Callback;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::Key;
use windows_reactor::Menu;
use windows_reactor::MenuItem;
use windows_reactor::NotifyIcon;
use windows_reactor::StackPanel;
use windows_reactor::TextBlock;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::WindowBackdrop;
use windows_reactor::WindowTheme;
use windows_reactor::WindowVisuals;

struct AppInput {
    icon_path: std::path::PathBuf,
    open_on_start: bool,
}

#[derive(Clone)]
enum Message {
    Open,
    Increment,
    Exit,
    Menu(Key),
}

struct TrayProbe {
    count: u32,
}

impl Application for TrayProbe {
    type Input = AppInput;
    type Message = Message;

    fn create(input: &AppInput, context: &ApplicationContext<Self>) -> Self {
        if input.open_on_start {
            context.show_window("settings");
        }
        Self { count: 0 }
    }

    fn update(&mut self, message: Message, context: &ApplicationContext<Self>) {
        match message {
            Message::Open => context.show_window("settings"),
            Message::Increment => self.count = self.count.saturating_add(1),
            Message::Exit => context.exit(),
            Message::Menu(key) => match key.as_str() {
                Some("open") => context.show_window("settings"),
                Some("increment") => self.count = self.count.saturating_add(1),
                Some("exit") => context.exit(),
                _ => {}
            },
        }
    }

    fn view(&self, input: &AppInput, context: &ApplicationViewContext<Self>) -> ApplicationView {
        ApplicationView::new()
            .window::<Settings>(
                "settings",
                Input {
                    count: self.count,
                    increment: context.message(Message::Increment),
                    exit: context.message(Message::Exit),
                },
            )
            .notify_icon(
                "tray",
                NotifyIcon::new(input.icon_path.clone())
                    .tooltip(format!("Circadia lifecycle probe: {}", self.count))
                    .on_activate(context.callback(|_| Message::Open))
                    .menu(Menu::new(
                        [
                            MenuItem::item("open", "Open settings probe"),
                            MenuItem::item("increment", "Increment application counter"),
                            MenuItem::item("exit", "Exit probe"),
                        ],
                        context.callback(Message::Menu),
                    )),
            )
    }
}

#[derive(Clone, PartialEq)]
struct Input {
    count: u32,
    increment: Callback<()>,
    exit: Callback<()>,
}

enum SettingsMessage {
    InspectHandle,
    HandleResult(bool),
}

struct Settings {
    handle_status: String,
}

impl Component for Settings {
    type Input = Input;
    type Message = SettingsMessage;

    fn create(_: &Input, _: &ComponentContext<Self>) -> Self {
        Self {
            handle_status: "Window handle has not been checked.".into(),
        }
    }

    fn update(&mut self, message: SettingsMessage, context: &ComponentContext<Self>) {
        match message {
            SettingsMessage::InspectHandle => {
                if !context
                    .run_window(|window| SettingsMessage::HandleResult(!window.as_raw().is_null()))
                {
                    self.handle_status = "Window handle request was rejected.".into();
                }
            }
            SettingsMessage::HandleResult(valid) => {
                self.handle_status = format!("Native window handle available: {valid}");
            }
        }
    }

    fn view(&self, input: &Input, context: &mut ViewContext<'_, Self>) -> View {
        context.window_visuals(
            WindowVisuals::new()
                .theme(WindowTheme::System)
                .backdrop(WindowBackdrop::Mica),
        );
        context.window_frame(
            "Circadia development lifecycle probe",
            StackPanel::new().spacing(12.0).children((
                "Close this window and reopen it from the blue square tray icon.",
                TextBlock::new()
                    .text(format!("Application counter: {}", input.count))
                    .automation_id("counter"),
                Button::new()
                    .automation_id("increment")
                    .on_click(input.increment.clone())
                    .content("Increment"),
                Button::new()
                    .automation_id("inspect-handle")
                    .on_click(context.callback(|()| SettingsMessage::InspectHandle))
                    .content("Check native window handle"),
                TextBlock::new()
                    .text(self.handle_status.clone())
                    .automation_id("handle-status"),
                Button::new()
                    .automation_id("exit")
                    .on_click(input.exit.clone())
                    .content("Exit probe"),
            )),
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let open = std::env::args()
        .skip(1)
        .any(|argument| argument == "--open");
    App::run_application::<TrayProbe>(AppInput {
        icon_path: std::env::current_exe()?.with_file_name("probe.ico"),
        open_on_start: open,
    })?;
    Ok(())
}
