use std::io;
use std::time::Instant;
use uiautomation::UIAutomation;
use uiautomation::UIElement;
use uiautomation::patterns::UIInvokePattern;
use uiautomation::patterns::UIValuePattern;
use uiautomation::patterns::UIWindowPattern;
use uiautomation::types::ControlType;
use uiautomation::types::TreeScope;
use uiautomation::types::WindowVisualState;
use winsafe::co;
use winsafe::guard::CoUninitializeGuard;

pub(super) struct Automation {
    client: UIAutomation,
    _apartment: CoUninitializeGuard,
}

impl Automation {
    pub(super) fn scrolls(&self, window: &winsafe::HWND, process: u32) -> io::Result<Vec<Scroll>> {
        let mut scrolls = Vec::new();
        for element in self.elements(window, process)? {
            if element.is_offscreen().map_err(io::Error::other)? {
                continue;
            }
            let Some(pattern) = optional_scroll(element.get_pattern())? else {
                continue;
            };
            let scroll = Scroll {
                id: element_id(&element)?,
                pattern,
            };
            if scroll.state()?.is_some() {
                scrolls.push(scroll);
            }
        }
        Ok(scrolls)
    }

    pub(super) fn expand(&self, window: &winsafe::HWND, process: u32, id: &str) -> io::Result<()> {
        use uiautomation::patterns::UIExpandCollapsePattern;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other("combo box is absent"))?;
        let pattern: UIExpandCollapsePattern = element.get_pattern().map_err(io::Error::other)?;
        pattern.expand().map_err(io::Error::other)
    }

    pub(super) fn select_language(
        &self,
        window: &winsafe::HWND,
        process: u32,
        label: &str,
    ) -> io::Result<bool> {
        use uiautomation::patterns::UISelectionItemPattern;
        for element in self.elements(window, process)? {
            let control = Control::read(&element)?;
            if control.name != label || control.kind != ControlType::ListItem {
                continue;
            }
            let pattern: UISelectionItemPattern =
                element.get_pattern().map_err(io::Error::other)?;
            pattern.select().map_err(io::Error::other)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn bounds(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<winsafe::RECT> {
        let graph = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        let bounds = graph.get_bounding_rectangle().map_err(io::Error::other)?;
        Ok(winsafe::RECT {
            left: bounds.get_left(),
            top: bounds.get_top(),
            right: bounds.get_right(),
            bottom: bounds.get_bottom(),
        })
    }

    pub(super) fn visible(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<bool> {
        let Some(element) = self.find(window, process, id)? else {
            return Ok(false);
        };
        Ok(!element.is_offscreen().map_err(io::Error::other)?)
    }

    pub(super) fn navigation_button(
        &self,
        window: &winsafe::HWND,
        process: u32,
    ) -> io::Result<String> {
        let controls = self.controls_within(window, process, circadia::automation_id::TITLE_BAR)?;
        let buttons: Vec<_> = controls
            .iter()
            .filter(|control| {
                control.kind == ControlType::Button
                    && control.bounds.2 > control.bounds.0
                    && control.bounds.3 > control.bounds.1
            })
            .collect();
        let [button] = buttons.as_slice() else {
            return Err(io::Error::other(format!(
                "expected one native titlebar menu button, found {buttons:?}"
            )));
        };
        if button.id.is_empty() {
            return Err(io::Error::other(
                "native titlebar menu button lacks an automation identifier",
            ));
        }
        Ok(button.id.clone())
    }

    #[cfg(feature = "graph-measurement")]
    pub(super) fn show_graph(&self, window: &winsafe::HWND, process: u32) -> io::Result<bool> {
        self.show_control(window, process, circadia::automation_id::GRAPH)
    }

    #[cfg(feature = "graph-measurement")]
    pub(super) fn live_setting(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<i32>> {
        self.find(window, process, id)?
            .map(|element| {
                element
                    .get_property_value(uiautomation::types::UIProperty::LiveSetting)
                    .and_then(TryInto::try_into)
                    .map_err(io::Error::other)
            })
            .transpose()
    }

    pub(super) fn number(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<f64>> {
        use uiautomation::patterns::UIRangeValuePattern;
        self.find(window, process, id)?
            .map(|element| {
                let pattern: UIRangeValuePattern =
                    element.get_pattern().map_err(io::Error::other)?;
                pattern.get_value().map_err(io::Error::other)
            })
            .transpose()
    }

    #[cfg(feature = "graph-measurement")]
    pub(super) fn range(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<(f64, f64, f64)> {
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        let pattern: uiautomation::patterns::UIRangeValuePattern =
            element.get_pattern().map_err(io::Error::other)?;
        let value = pattern.get_value().map_err(io::Error::other)?;
        let minimum = pattern.get_minimum().map_err(io::Error::other)?;
        let maximum = pattern.get_maximum().map_err(io::Error::other)?;
        Ok((value, minimum, maximum))
    }

    pub(super) fn select(&self, window: &winsafe::HWND, process: u32, id: &str) -> io::Result<()> {
        use uiautomation::patterns::UISelectionItemPattern;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        let pattern: UISelectionItemPattern = element.get_pattern().map_err(io::Error::other)?;
        pattern.select().map_err(io::Error::other)
    }

    pub(super) fn set_number(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
        value: f64,
    ) -> io::Result<()> {
        use uiautomation::patterns::UIRangeValuePattern;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        let pattern: UIRangeValuePattern = element.get_pattern().map_err(io::Error::other)?;
        pattern.set_value(value).map_err(io::Error::other)
    }

    pub(super) fn toggle(&self, window: &winsafe::HWND, process: u32, id: &str) -> io::Result<()> {
        use uiautomation::patterns::UITogglePattern;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other("toggle control not found"))?;
        let pattern: UITogglePattern = element.get_pattern().map_err(io::Error::other)?;
        pattern.toggle().map_err(io::Error::other)
    }

    pub(super) fn show_control(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<bool> {
        use circadia::automation_id;
        use uiautomation::patterns::UIScrollPattern;
        let Some(graph) = self.find(window, process, id)? else {
            return Err(io::Error::other(
                "generated graph has no UI Automation element",
            ));
        };
        let Some(scroll) = self.find(window, process, automation_id::SCROLL)? else {
            return Err(io::Error::other(
                "settings viewport has no UI Automation element",
            ));
        };
        let bounds = graph.get_bounding_rectangle().map_err(io::Error::other)?;
        let mut viewport = scroll.get_bounding_rectangle().map_err(io::Error::other)?;
        let outer = window.GetWindowRect().map_err(io::Error::other)?;
        viewport.set_top(viewport.get_top().max(outer.top));
        viewport.set_bottom(viewport.get_bottom().min(outer.bottom));
        if bounds.get_top() >= viewport.get_top()
            && bounds.get_bottom() <= viewport.get_bottom()
            && bounds.get_bottom() > bounds.get_top()
            && bounds.get_right() > bounds.get_left()
        {
            return Ok(true);
        }
        let pattern: UIScrollPattern = scroll.get_pattern().map_err(io::Error::other)?;
        let visible = pattern.get_vertical_view_size().map_err(io::Error::other)?;
        let current = pattern
            .get_vertical_scroll_percent()
            .map_err(io::Error::other)?;
        let extent =
            f64::from(viewport.get_bottom() - viewport.get_top()) * (100.0 / visible - 1.0);
        if !extent.is_finite() || extent <= 0.0 {
            return Err(io::Error::other(format!(
                "invalid graph scroll extent: {extent}; view size {visible}; graph {bounds:?}; viewport {viewport:?}"
            )));
        }
        let next = if bounds.get_bottom() <= bounds.get_top() {
            if current >= 100.0 {
                return Err(io::Error::other(
                    "graph did not become visible after scrolling",
                ));
            }
            current + 5.0
        } else {
            let delta = f64::from(
                bounds.get_top() + bounds.get_bottom() - viewport.get_top() - viewport.get_bottom(),
            ) / 2.0;
            current + delta / extent * 100.0
        };
        pattern
            .set_scroll_percent(-1.0, next.clamp(0.0, 100.0))
            .map_err(io::Error::other)?;
        Ok(false)
    }

    pub(super) fn enabled(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<bool>> {
        self.find(window, process, id)?
            .map(|element| element.is_enabled().map_err(io::Error::other))
            .transpose()
    }

    pub(super) fn value(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<String>> {
        self.find(window, process, id)?
            .map(|element| {
                let pattern: UIValuePattern = element.get_pattern().map_err(io::Error::other)?;
                pattern.get_value().map_err(io::Error::other)
            })
            .transpose()
    }

    pub(super) fn set_value(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
        value: &str,
    ) -> io::Result<()> {
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        let pattern: UIValuePattern = element.get_pattern().map_err(io::Error::other)?;
        pattern.set_value(value).map_err(io::Error::other)
    }

    pub(super) fn dialog_button(
        &self,
        window: &winsafe::HWND,
        process: u32,
        button: DialogButton,
    ) -> io::Result<bool> {
        let id = button.id();
        let mut matches = Vec::new();
        for element in self.dialog_buttons(window, process, button)? {
            if element.is_enabled().map_err(io::Error::other)? {
                matches.push(element);
            }
        }
        if matches.len() > 1 {
            return Err(io::Error::other(format!(
                "{} enabled dialog buttons have the identifier {id}",
                matches.len()
            )));
        }
        let Some(element) = matches.pop() else {
            return Ok(false);
        };
        let pattern: UIInvokePattern = element.get_pattern().map_err(io::Error::other)?;
        pattern.invoke().map_err(io::Error::other)?;
        Ok(true)
    }

    pub(super) fn dialog_button_enabled(
        &self,
        window: &winsafe::HWND,
        process: u32,
        button: DialogButton,
    ) -> io::Result<Option<bool>> {
        self.dialog_buttons(window, process, button)?
            .first()
            .map(|element| Control::read(element).map(|control| control.enabled))
            .transpose()
    }

    // A closable banner's close button has the same WinUI identifier as a
    // dialog's close button.
    fn dialog_buttons(
        &self,
        window: &winsafe::HWND,
        process: u32,
        button: DialogButton,
    ) -> io::Result<Vec<UIElement>> {
        let walker = self
            .client
            .get_control_view_walker()
            .map_err(io::Error::other)?;
        let mut buttons = Vec::new();
        for element in self.elements(window, process)? {
            if element_id(&element)? != button.id() {
                continue;
            }
            if element.get_control_type().map_err(io::Error::other)? != ControlType::Button {
                continue;
            }
            let mut current = element.clone();
            let mut banner = false;
            while let Ok(parent) = walker.get_parent(&current) {
                banner |= element_id(&parent)?.starts_with("banner-");
                current = parent;
            }
            if !banner {
                buttons.push(element);
            }
        }
        Ok(buttons)
    }

    pub(super) fn toggled(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<bool>> {
        use uiautomation::patterns::UITogglePattern;
        use uiautomation::types::ToggleState;
        self.find(window, process, id)?
            .map(|element| {
                let pattern: UITogglePattern = element.get_pattern().map_err(io::Error::other)?;
                pattern
                    .get_toggle_state()
                    .map(|state| state == ToggleState::On)
                    .map_err(io::Error::other)
            })
            .transpose()
    }

    pub(super) fn focus_control(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<()> {
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        element.set_focus().map_err(io::Error::other)
    }

    pub(super) fn new() -> io::Result<Self> {
        let apartment =
            winsafe::CoInitializeEx(co::COINIT::MULTITHREADED).map_err(io::Error::other)?;
        let client = UIAutomation::new_direct().map_err(io::Error::other)?;
        Ok(Self {
            client,
            _apartment: apartment,
        })
    }

    pub(super) fn root(&self, window: &winsafe::HWND, process: u32) -> io::Result<UIElement> {
        let root = self
            .client
            .element_from_handle(window.ptr().addr().cast_signed().into())
            .map_err(io::Error::other)?;
        let owner = root.get_process_id().map_err(io::Error::other)?;
        if owner != process {
            return Err(io::Error::other(
                "automation window belongs to another process",
            ));
        }
        Ok(root)
    }

    pub(super) fn find(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<UIElement>> {
        self.elements(window, process)?
            .into_iter()
            .find_map(|element| {
                element_id(&element)
                    .map(|actual| (actual == id).then_some(element))
                    .transpose()
            })
            .transpose()
    }

    pub(super) fn same_control(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
        previous: &UIElement,
    ) -> io::Result<bool> {
        let current = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        self.client
            .compare_elements(previous, &current)
            .map_err(io::Error::other)
    }

    pub(super) fn elements(
        &self,
        window: &winsafe::HWND,
        process: u32,
    ) -> io::Result<Vec<UIElement>> {
        let root = self.root(window, process)?;
        self.descendants(&root)
    }

    fn descendants(&self, root: &UIElement) -> io::Result<Vec<UIElement>> {
        let condition = self
            .client
            .create_true_condition()
            .map_err(io::Error::other)?;
        root.find_all(TreeScope::Descendants, &condition)
            .map_err(io::Error::other)
    }

    pub(super) fn text(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<String>> {
        self.find(window, process, id)?
            .map(|element| element.get_name().map_err(io::Error::other))
            .transpose()
    }

    pub(super) fn controls_within(
        &self,
        window: &winsafe::HWND,
        process: u32,
        container: &str,
    ) -> io::Result<Vec<Control>> {
        let container = self
            .find(window, process, container)?
            .ok_or_else(|| io::Error::other(format!("container {container} is absent")))?;
        self.descendants(&container)?
            .iter()
            .map(Control::read)
            .collect()
    }

    pub(super) fn controls_with_parents(
        &self,
        window: &winsafe::HWND,
        process: u32,
    ) -> io::Result<Vec<(Control, Option<Control>)>> {
        let walker = self
            .client
            .get_control_view_walker()
            .map_err(io::Error::other)?;
        self.elements(window, process)?
            .iter()
            .map(|element| {
                let parent = walker
                    .get_parent(element)
                    .ok()
                    .map(|parent| Control::read(&parent))
                    .transpose()?;
                Ok((Control::read(element)?, parent))
            })
            .collect()
    }

    pub(super) fn focused(&self, process: u32) -> io::Result<Option<Focused>> {
        let element = self
            .client
            .get_focused_element()
            .map_err(io::Error::other)?;
        let owner = element.get_process_id().map_err(io::Error::other)?;
        if owner != process {
            return Ok(None);
        }
        let control = Control::read(&element)?;
        let walker = self
            .client
            .get_control_view_walker()
            .map_err(io::Error::other)?;
        let mut identifiers = vec![control.id.clone()];
        let mut current = element;
        while let Ok(parent) = walker.get_parent(&current) {
            identifiers.push(element_id(&parent)?);
            current = parent;
        }
        identifiers.retain(|id| !id.is_empty());
        Ok(Some(Focused {
            control,
            identifiers,
        }))
    }

    pub(super) fn names(&self, window: &winsafe::HWND, process: u32) -> io::Result<Vec<String>> {
        self.elements(window, process)?
            .iter()
            .map(|element| element.get_name().map_err(io::Error::other))
            .collect()
    }

    pub(super) fn invoke(&self, window: &winsafe::HWND, process: u32, id: &str) -> io::Result<()> {
        let element = self.find(window, process, id)?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("control {id} is absent"))
        })?;
        let pattern: UIInvokePattern = element.get_pattern().map_err(io::Error::other)?;
        pattern.invoke().map_err(io::Error::other)
    }

    pub(super) fn close(&self, window: &winsafe::HWND, process: u32) -> io::Result<()> {
        let root = self.root(window, process)?;
        let pattern: UIWindowPattern = root.get_pattern().map_err(io::Error::other)?;
        pattern.close().map_err(io::Error::other)
    }

    pub(super) fn focus(&self, window: &winsafe::HWND, process: u32) -> io::Result<()> {
        let root = self.root(window, process)?;
        root.set_focus().map_err(io::Error::other)
    }

    pub(super) fn minimize(&self, window: &winsafe::HWND, process: u32) -> io::Result<()> {
        let root = self.root(window, process)?;
        let pattern: UIWindowPattern = root.get_pattern().map_err(io::Error::other)?;
        pattern
            .set_window_visual_state(WindowVisualState::Minimized)
            .map_err(io::Error::other)
    }
}

pub(super) struct Scroll {
    pub(super) id: String,
    pattern: uiautomation::patterns::UIScrollPattern,
}

fn optional_scroll(
    result: uiautomation::Result<uiautomation::patterns::UIScrollPattern>,
) -> io::Result<Option<uiautomation::patterns::UIScrollPattern>> {
    match result {
        Ok(pattern) => Ok(Some(pattern)),
        // WinUI can return S_OK with a null pattern; the projection reports
        // Error::empty().
        Err(error)
            if error.code() == 0
                || error.code() == windows_sys::Win32::winerror::E_NOINTERFACE
                || error.code().cast_unsigned()
                    == windows::Win32::uiautomationcoreapi::UIA_E_NOTSUPPORTED =>
        {
            Ok(None)
        }
        Err(error) => Err(io::Error::other(error)),
    }
}

impl Scroll {
    pub(super) fn state(&self) -> io::Result<Option<(f64, f64)>> {
        if !self
            .pattern
            .is_vertically_scrollable()
            .map_err(io::Error::other)?
        {
            return Ok(None);
        }
        let position = self
            .pattern
            .get_vertical_scroll_percent()
            .map_err(io::Error::other)?;
        let view = self
            .pattern
            .get_vertical_view_size()
            .map_err(io::Error::other)?;
        Ok(Some((position, view)))
    }

    pub(super) fn set(&self, position: f64) -> io::Result<()> {
        if !position.is_finite() || !(0.0..=100.0).contains(&position) {
            return Err(io::Error::other("invalid scroll percentage"));
        }
        self.pattern
            .set_scroll_percent(-1.0, position)
            .map_err(io::Error::other)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Control {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) kind: ControlType,
    pub(super) enabled: bool,
    pub(super) bounds: (i32, i32, i32, i32),
}

impl Control {
    fn read(element: &UIElement) -> io::Result<Self> {
        let name = element.get_name().map_err(io::Error::other)?;
        let kind = element.get_control_type().map_err(io::Error::other)?;
        let enabled = element.is_enabled().map_err(io::Error::other)?;
        let bounds = element.get_bounding_rectangle().map_err(io::Error::other)?;
        Ok(Self {
            id: element_id(element)?,
            name,
            kind,
            enabled,
            bounds: (
                bounds.get_left(),
                bounds.get_top(),
                bounds.get_right(),
                bounds.get_bottom(),
            ),
        })
    }

    pub(super) fn interactive(&self) -> bool {
        [
            ControlType::Button,
            ControlType::CheckBox,
            ControlType::ComboBox,
            ControlType::Edit,
            ControlType::Hyperlink,
            ControlType::Slider,
            ControlType::Spinner,
        ]
        .contains(&self.kind)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Focused {
    pub(super) control: Control,
    pub(super) identifiers: Vec<String>,
}

pub(super) fn press_tab() -> io::Result<()> {
    press_key(co::VK::TAB)
}

pub(super) fn press_key(key: co::VK) -> io::Result<()> {
    let inputs = [
        winsafe::HwKbMouse::Kb(winsafe::KEYBDINPUT {
            wVk: key,
            ..winsafe::KEYBDINPUT::default()
        }),
        winsafe::HwKbMouse::Kb(winsafe::KEYBDINPUT {
            wVk: key,
            dwFlags: co::KEYEVENTF::KEYUP,
            ..winsafe::KEYBDINPUT::default()
        }),
    ];
    send_input(&inputs)
}

pub(super) fn send_input(inputs: &[winsafe::HwKbMouse]) -> io::Result<()> {
    send_input_with(inputs, |events| {
        winsafe::SendInput(events).map_err(io::Error::other)
    })
}

fn send_input_with(
    inputs: &[winsafe::HwKbMouse],
    mut dispatch: impl FnMut(&[winsafe::HwKbMouse]) -> io::Result<u32>,
) -> io::Result<()> {
    let sent = dispatch(inputs)?;
    if usize::try_from(sent).ok() == Some(inputs.len()) {
        return Ok(());
    }
    let releases: Vec<_> = inputs
        .iter()
        .copied()
        .filter(|event| match event {
            winsafe::HwKbMouse::Kb(key) => key.dwFlags.has(co::KEYEVENTF::KEYUP),
            winsafe::HwKbMouse::Mouse(mouse) => mouse.dwFlags.has(co::MOUSEEVENTF::LEFTUP),
            winsafe::HwKbMouse::Hw(_) => false,
        })
        .collect();
    match dispatch(&releases) {
        Ok(cleaned) => Err(io::Error::other(format!(
            "input insertion incomplete ({sent}/{}); released {cleaned}/{} cleanup events",
            inputs.len(),
            releases.len(),
        ))),
        Err(source) => Err(io::Error::other(InputCleanupFailure {
            sent,
            expected: inputs.len(),
            source,
        })),
    }
}

#[derive(Debug, thiserror::Error)]
#[error("input insertion incomplete ({sent}/{expected}); releasing input also failed")]
struct InputCleanupFailure {
    sent: u32,
    expected: usize,
    #[source]
    source: io::Error,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum DialogButton {
    Primary,
    Close,
}

impl DialogButton {
    // WinUI names the buttons of every ContentDialog with these identifiers.
    fn id(self) -> &'static str {
        match self {
            Self::Primary => "PrimaryButton",
            Self::Close => "CloseButton",
        }
    }
}

pub(super) fn wait<T>(
    description: &str,
    check: impl FnMut() -> io::Result<Option<T>>,
) -> io::Result<T> {
    wait_until(
        description,
        Instant::now() + crate::lifecycle::WAIT_TIMEOUT,
        check,
    )
}

pub(super) fn wait_until<T>(
    description: &str,
    deadline: Instant,
    mut check: impl FnMut() -> io::Result<Option<T>>,
) -> io::Result<T> {
    crate::lifecycle::wait_until(description, deadline, || match check() {
        Err(error)
            if error
                .get_ref()
                .and_then(|source| source.downcast_ref::<uiautomation::Error>())
                .is_some_and(|source| {
                    matches!(
                        source.code().cast_unsigned(),
                        windows::Win32::uiautomationcoreapi::UIA_E_ELEMENTNOTAVAILABLE
                            | windows::Win32::uiautomationcoreapi::UIA_E_TIMEOUT
                    )
                }) =>
        {
            Ok(None)
        }
        result => result,
    })
}

fn element_id(element: &UIElement) -> io::Result<String> {
    element.get_automation_id().map_err(io::Error::other)
}

pub(super) fn click_point(x: i32, y: i32) -> io::Result<()> {
    winsafe::SetCursorPos(x, y).map_err(io::Error::other)?;
    let inputs = [
        winsafe::HwKbMouse::Mouse(winsafe::MOUSEINPUT {
            dwFlags: winsafe::co::MOUSEEVENTF::LEFTDOWN,
            ..winsafe::MOUSEINPUT::default()
        }),
        winsafe::HwKbMouse::Mouse(winsafe::MOUSEINPUT {
            dwFlags: winsafe::co::MOUSEEVENTF::LEFTUP,
            ..winsafe::MOUSEINPUT::default()
        }),
    ];
    send_input(&inputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;
    use windows::Win32::uiautomationcoreapi::UIA_E_ELEMENTNOTAVAILABLE;
    use windows::Win32::uiautomationcoreapi::UIA_E_TIMEOUT;

    #[test_case(0 ; "null successful pattern")]
    #[test_case(windows_sys::Win32::winerror::E_NOINTERFACE.cast_unsigned() ; "unsupported interface")]
    #[test_case(windows::Win32::uiautomationcoreapi::UIA_E_NOTSUPPORTED ; "unsupported pattern")]
    fn absent_scroll_patterns_are_skipped(code: u32) {
        let error = uiautomation::Error::new(code.cast_signed(), "automation fixture");
        assert!(matches!(optional_scroll(Err(error)), Ok(None)));
    }

    #[test_case(UIA_E_ELEMENTNOTAVAILABLE)]
    #[test_case(UIA_E_TIMEOUT)]
    #[test_case(0x8007_0005)]
    fn scroll_observation_failures_are_reported(code: u32) {
        let error = uiautomation::Error::new(code.cast_signed(), "automation fixture");
        assert!(optional_scroll(Err(error)).is_err());
    }

    const ID_METHODS: [&str; 11] = [
        "invoke",
        "text",
        "enabled",
        "value",
        "set_value",
        "toggle",
        "toggled",
        "number",
        "set_number",
        "select",
        "focus_control",
    ];

    fn arguments(source: &str) -> Vec<&str> {
        let mut depth = 0_usize;
        let mut quoted = false;
        let mut start = 0;
        let mut found = Vec::new();
        for (index, character) in source.char_indices() {
            match character {
                '"' => quoted = !quoted,
                '(' | '[' | '{' if !quoted => depth += 1,
                ')' | ']' | '}' if !quoted && depth == 0 => {
                    found.extend(source.get(start..index).map(str::trim));
                    return found;
                }
                ')' | ']' | '}' if !quoted => depth -= 1,
                ',' if !quoted && depth == 0 => {
                    found.extend(source.get(start..index).map(str::trim));
                    start = index + 1;
                }
                _ => {}
            }
        }
        found
    }

    fn unlisted(source: &str) -> Vec<String> {
        ID_METHODS
            .iter()
            .flat_map(|method| {
                let call = format!(".{method}(");
                source
                    .match_indices(&call)
                    .map(|(at, _)| at + call.len())
                    .collect::<Vec<_>>()
            })
            .filter_map(|after| source.get(after..))
            .filter_map(|rest| arguments(rest).get(2).copied())
            .filter_map(|id| id.strip_prefix('"')?.strip_suffix('"'))
            .filter(|id| !circadia::automation_id::FIXED.contains(id))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn harness_literal_ids_are_in_the_public_list() {
        let mut sources = vec![std::path::PathBuf::from("tools/native_lifecycle.rs")];
        for entry in fs_err::read_dir("tools/native").expect("harness sources") {
            let path = entry.expect("harness source").path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
        let unlisted: Vec<(std::path::PathBuf, Vec<String>)> = sources
            .into_iter()
            .map(|path| {
                let source = fs_err::read_to_string(&path).expect("harness source text");
                let checked = source
                    .split_once("#[cfg(test)]")
                    .map_or(source.as_str(), |(checked, _)| checked);
                (path, unlisted(checked))
            })
            .filter(|(_, ids)| !ids.is_empty())
            .collect();
        assert_eq!(unlisted, Vec::new());
    }

    #[test]
    fn unlisted_literal_ids_are_reported() {
        let source = r#"
            automation.text(settings, process, "draft-status")?;
            automation.invoke(&settings, std::process::id(), "preview")?;
            automation.set_number(settings, process, automation_id::SLEEP_NUMBER, 3000.0)?;
            automation.enabled(
                settings,
                process,
                "save",
            )?;
        "#;
        assert_eq!(unlisted(source), ["draft-status", "save"]);
    }

    #[test_case(UIA_E_ELEMENTNOTAVAILABLE)]
    #[test_case(UIA_E_TIMEOUT)]
    fn transient_observation_is_retried(code: u32) {
        let mut attempts = 0;
        let result = wait("reacquire replaced UI element", || {
            attempts += 1;
            if attempts == 1 {
                Err(io::Error::other(uiautomation::Error::new(
                    code.cast_signed(),
                    "automation fixture",
                )))
            } else {
                Ok(Some(42))
            }
        });
        assert_eq!(result.expect("reacquired element"), 42);
        assert_eq!(attempts, 2);
    }

    #[test]
    fn other_automation_errors_are_not_retried() {
        let mut attempts = 0;
        let result: io::Result<()> = wait("failed automation", || {
            attempts += 1;
            Err(io::Error::other(uiautomation::Error::new(
                0x8007_0005_u32.cast_signed(),
                "automation fixture",
            )))
        });
        let error = result.expect_err("access denied remains an error");
        let source = error
            .get_ref()
            .and_then(|error| error.source())
            .and_then(|error| error.downcast_ref::<io::Error>())
            .and_then(io::Error::get_ref)
            .and_then(|error| error.downcast_ref::<uiautomation::Error>());
        assert_eq!(
            source.map(|error| error.code().cast_unsigned()),
            Some(0x8007_0005)
        );
        assert_eq!(attempts, 1);
    }

    #[test_case(UIA_E_ELEMENTNOTAVAILABLE)]
    #[test_case(UIA_E_TIMEOUT)]
    fn failed_observation_does_not_satisfy_an_absence_check(code: u32) {
        let mut attempts = 0;
        wait("confirmed absence", || {
            attempts += 1;
            let observed: io::Result<Option<&str>> = match attempts {
                1 => Err(io::Error::other(uiautomation::Error::new(
                    code.cast_signed(),
                    "automation fixture",
                ))),
                2 => Ok(Some("present control")),
                _ => Ok(None),
            };
            observed.map(|element| element.is_none().then_some(()))
        })
        .expect("a successful observation establishes absence");
        assert_eq!(attempts, 3);
    }
    fn key_events() -> [winsafe::HwKbMouse; 2] {
        [
            winsafe::HwKbMouse::Kb(winsafe::KEYBDINPUT {
                wVk: co::VK::TAB,
                ..winsafe::KEYBDINPUT::default()
            }),
            winsafe::HwKbMouse::Kb(winsafe::KEYBDINPUT {
                wVk: co::VK::TAB,
                dwFlags: co::KEYEVENTF::KEYUP,
                ..winsafe::KEYBDINPUT::default()
            }),
        ]
    }

    #[test]
    fn complete_input_does_not_send_cleanup() {
        let mut calls = 0;
        send_input_with(&key_events(), |events| {
            calls += 1;
            assert_eq!(events.len(), 2);
            Ok(2)
        })
        .expect("complete insertion");
        assert_eq!(calls, 1);
    }

    #[test]
    fn partial_keyboard_input_releases_the_key() {
        let mut calls = 0;
        let result = send_input_with(&key_events(), |events| {
            calls += 1;
            if calls == 1 {
                assert_eq!(events.len(), 2);
            } else {
                assert!(
                    matches!(events, [winsafe::HwKbMouse::Kb(key)] if key.wVk == co::VK::TAB && key.dwFlags == co::KEYEVENTF::KEYUP)
                );
            }
            Ok(1)
        });
        assert_eq!(calls, 2);
        assert!(
            result
                .expect_err("partial insertion")
                .to_string()
                .contains("released 1/1")
        );
    }

    #[test]
    fn partial_mouse_input_releases_the_button() {
        let events = [
            winsafe::HwKbMouse::Mouse(winsafe::MOUSEINPUT {
                dwFlags: co::MOUSEEVENTF::LEFTDOWN,
                ..winsafe::MOUSEINPUT::default()
            }),
            winsafe::HwKbMouse::Mouse(winsafe::MOUSEINPUT {
                dwFlags: co::MOUSEEVENTF::LEFTUP,
                ..winsafe::MOUSEINPUT::default()
            }),
        ];
        let mut calls = 0;
        let result = send_input_with(&events, |events| {
            calls += 1;
            if calls == 2 {
                assert!(
                    matches!(events, [winsafe::HwKbMouse::Mouse(mouse)] if mouse.dwFlags == co::MOUSEEVENTF::LEFTUP)
                );
            }
            Ok(1)
        });
        assert_eq!(calls, 2);
        assert!(result.is_err());
    }

    #[test]
    fn cleanup_failure_preserves_the_source() {
        let mut calls = 0;
        let error = send_input_with(&key_events(), |_| {
            calls += 1;
            if calls == 1 {
                Ok(1)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "fixture denial",
                ))
            }
        })
        .expect_err("cleanup failure");
        let failure = error
            .get_ref()
            .and_then(|error| error.downcast_ref::<InputCleanupFailure>())
            .expect("typed cleanup error");
        assert_eq!(failure.sent, 1);
        assert_eq!(failure.expected, 2);
        assert_eq!(failure.source.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn incomplete_release_is_reported() {
        let mut calls = 0;
        let error = send_input_with(&key_events(), |_| {
            calls += 1;
            Ok(u32::from(calls == 1))
        })
        .expect_err("partial cleanup");
        assert!(error.to_string().contains("released 0/1"));
    }

    #[test]
    fn rejected_initial_input_does_not_send_cleanup() {
        let mut calls = 0;
        let error = send_input_with(&key_events(), |_| {
            calls += 1;
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "fixture denial",
            ))
        })
        .expect_err("initial failure");
        assert_eq!(calls, 1);
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
