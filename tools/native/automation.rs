use std::io;
use std::time::Instant;
use windows::Win32::combaseapi::CoCreateInstance;
use windows::Win32::uiautomationclient::CUIAutomation;
use windows::Win32::uiautomationclient::IUIAutomation;
use windows::Win32::uiautomationclient::IUIAutomationElement;
use windows::Win32::uiautomationclient::IUIAutomationInvokePattern;
use windows::Win32::uiautomationclient::IUIAutomationValuePattern;
use windows::Win32::uiautomationclient::IUIAutomationWindowPattern;
use windows::Win32::uiautomationclient::TreeScope_Descendants;
use windows::Win32::uiautomationclient::UIA_ButtonControlTypeId;
use windows::Win32::uiautomationclient::UIA_CheckBoxControlTypeId;
use windows::Win32::uiautomationclient::UIA_ComboBoxControlTypeId;
use windows::Win32::uiautomationclient::UIA_EditControlTypeId;
use windows::Win32::uiautomationclient::UIA_HyperlinkControlTypeId;
use windows::Win32::uiautomationclient::UIA_InvokePatternId;
use windows::Win32::uiautomationclient::UIA_SliderControlTypeId;
use windows::Win32::uiautomationclient::UIA_SpinnerControlTypeId;
use windows::Win32::uiautomationclient::UIA_ValuePatternId;
use windows::Win32::uiautomationclient::UIA_WindowPatternId;
use windows::Win32::uiautomationcore::WindowVisualState_Minimized;
use windows::Win32::wtypesbase::CLSCTX_INPROC_SERVER;
use winsafe::co;
use winsafe::guard::CoUninitializeGuard;

pub(super) struct Automation {
    client: IUIAutomation,
    _apartment: CoUninitializeGuard,
}

impl Automation {
    #[expect(
        unsafe_code,
        reason = "enumerate scroll patterns in the process-checked window"
    )]
    pub(super) fn scrolls(&self, window: &winsafe::HWND, process: u32) -> io::Result<Vec<Scroll>> {
        use windows::Win32::uiautomationclient::UIA_ScrollPatternId;
        let mut scrolls = Vec::new();
        for element in self.elements(window, process)? {
            // SAFETY: The live element returns its visibility into
            // binding-owned storage.
            if unsafe { element.CurrentIsOffscreen() }
                .map_err(io::Error::other)?
                .as_bool()
            {
                continue;
            }
            // SAFETY: The identifier matches the requested interface;
            // unsupported patterns fail.
            let Some(pattern) =
                optional_scroll(unsafe { element.GetCurrentPatternAs(UIA_ScrollPatternId) })?
            else {
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

    #[expect(
        unsafe_code,
        reason = "open and select a process-checked language combo box"
    )]
    pub(super) fn expand(&self, window: &winsafe::HWND, process: u32, id: &str) -> io::Result<()> {
        use windows::Win32::uiautomationclient::IUIAutomationExpandCollapsePattern;
        use windows::Win32::uiautomationclient::UIA_ExpandCollapsePatternId;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other("combo box is absent"))?;
        // SAFETY: The pattern identifier matches the requested interface.
        let pattern: IUIAutomationExpandCollapsePattern =
            unsafe { element.GetCurrentPatternAs(UIA_ExpandCollapsePatternId) }
                .map_err(io::Error::other)?;
        // SAFETY: The live pattern belongs to the checked window.
        unsafe { pattern.Expand() }.ok().map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "select a named language item within the process-checked popup"
    )]
    pub(super) fn select_language(
        &self,
        window: &winsafe::HWND,
        process: u32,
        label: &str,
    ) -> io::Result<bool> {
        use windows::Win32::uiautomationclient::IUIAutomationSelectionItemPattern;
        use windows::Win32::uiautomationclient::UIA_ListItemControlTypeId;
        use windows::Win32::uiautomationclient::UIA_SelectionItemPatternId;
        for element in self.elements(window, process)? {
            let control = Control::read(&element)?;
            if control.name != label || control.kind != UIA_ListItemControlTypeId {
                continue;
            }
            // SAFETY: The pattern identifier matches the requested interface.
            let pattern: IUIAutomationSelectionItemPattern =
                unsafe { element.GetCurrentPatternAs(UIA_SelectionItemPatternId) }
                    .map_err(io::Error::other)?;
            // SAFETY: The live pattern belongs to the checked window.
            unsafe { pattern.Select() }.ok().map_err(io::Error::other)?;
            return Ok(true);
        }
        Ok(false)
    }

    #[expect(
        unsafe_code,
        reason = "read a process-checked control's screen bounds for pixel capture"
    )]
    pub(super) fn bounds(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<winsafe::RECT> {
        let graph = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        // SAFETY: The process-checked live element returns an owned rectangle.
        let bounds = unsafe { graph.CurrentBoundingRectangle() }.map_err(io::Error::other)?;
        Ok(winsafe::RECT {
            left: bounds.left,
            top: bounds.top,
            right: bounds.right,
            bottom: bounds.bottom,
        })
    }

    #[expect(
        unsafe_code,
        reason = "read visibility from the process-checked automation element"
    )]
    pub(super) fn visible(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<bool> {
        let Some(element) = self.find(window, process, id)? else {
            return Ok(false);
        };
        // SAFETY: The live process-checked element returns a scalar visibility
        // flag.
        Ok(!unsafe { element.CurrentIsOffscreen() }
            .map_err(io::Error::other)?
            .as_bool())
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
                control.kind == UIA_ButtonControlTypeId
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
    #[expect(
        unsafe_code,
        reason = "read the live-region setting from the process-checked automation element"
    )]
    pub(super) fn live_setting(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<i32>> {
        use windows::Win32::uiautomationclient::IUIAutomationElement2;
        use windows::core::Interface;
        self.find(window, process, id)?
            .map(|element| {
                let element: IUIAutomationElement2 = element.cast().map_err(io::Error::other)?;
                // SAFETY: The live interface returns a scalar through owned
                // binding storage.
                unsafe { element.CurrentLiveSetting() }.map_err(io::Error::other)
            })
            .transpose()
    }

    #[expect(
        unsafe_code,
        reason = "read a number control through its UI Automation range value pattern"
    )]
    pub(super) fn number(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<f64>> {
        use windows::Win32::uiautomationclient::IUIAutomationRangeValuePattern;
        use windows::Win32::uiautomationclient::UIA_RangeValuePatternId;
        self.find(window, process, id)?
            .map(|element| {
                // SAFETY: The pattern identifier matches the requested
                // interface.
                let pattern: IUIAutomationRangeValuePattern =
                    unsafe { element.GetCurrentPatternAs(UIA_RangeValuePatternId) }
                        .map_err(io::Error::other)?;
                // SAFETY: The live pattern returns a scalar.
                unsafe { pattern.CurrentValue() }.map_err(io::Error::other)
            })
            .transpose()
    }

    #[cfg(feature = "graph-measurement")]
    #[expect(
        unsafe_code,
        reason = "read range semantics from the process-checked native progress control"
    )]
    pub(super) fn range(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<(f64, f64, f64)> {
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        // SAFETY: The pattern identifier matches the owned interface.
        let pattern: windows::Win32::uiautomationclient::IUIAutomationRangeValuePattern = unsafe {
            element.GetCurrentPatternAs(windows::Win32::uiautomationclient::UIA_RangeValuePatternId)
        }
        .map_err(io::Error::other)?;
        // SAFETY: Each getter returns a scalar through binding-owned storage.
        let value = unsafe { pattern.CurrentValue() }.map_err(io::Error::other)?;
        // SAFETY: The live interface writes only its owned scalar output.
        let minimum = unsafe { pattern.CurrentMinimum() }.map_err(io::Error::other)?;
        // SAFETY: The live interface writes only its owned scalar output.
        let maximum = unsafe { pattern.CurrentMaximum() }.map_err(io::Error::other)?;
        Ok((value, minimum, maximum))
    }

    #[expect(
        unsafe_code,
        reason = "select a navigation item in the owned settings window"
    )]
    pub(super) fn select(&self, window: &winsafe::HWND, process: u32, id: &str) -> io::Result<()> {
        use windows::Win32::uiautomationclient::IUIAutomationSelectionItemPattern;
        use windows::Win32::uiautomationclient::UIA_SelectionItemPatternId;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        // SAFETY: The pattern identifier matches the requested interface.
        let pattern: IUIAutomationSelectionItemPattern =
            unsafe { element.GetCurrentPatternAs(UIA_SelectionItemPatternId) }
                .map_err(io::Error::other)?;
        // SAFETY: The live pattern has no borrowed inputs or output pointers.
        unsafe { pattern.Select() }.ok().map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "commit a number control through its UI Automation range value pattern"
    )]
    pub(super) fn set_number(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
        value: f64,
    ) -> io::Result<()> {
        use windows::Win32::uiautomationclient::IUIAutomationRangeValuePattern;
        use windows::Win32::uiautomationclient::UIA_RangeValuePatternId;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        // SAFETY: The pattern identifier matches the requested interface.
        let pattern: IUIAutomationRangeValuePattern =
            unsafe { element.GetCurrentPatternAs(UIA_RangeValuePatternId) }
                .map_err(io::Error::other)?;
        // SAFETY: The live pattern takes a scalar.
        unsafe { pattern.SetValue(value) }
            .ok()
            .map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "toggle an identified control in the owned settings window"
    )]
    pub(super) fn toggle(&self, window: &winsafe::HWND, process: u32, id: &str) -> io::Result<()> {
        use windows::Win32::uiautomationclient::IUIAutomationTogglePattern;
        use windows::Win32::uiautomationclient::UIA_TogglePatternId;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other("toggle control not found"))?;
        // SAFETY: The requested interface matches the toggle pattern
        // identifier.
        let pattern: IUIAutomationTogglePattern =
            unsafe { element.GetCurrentPatternAs(UIA_TogglePatternId) }
                .map_err(io::Error::other)?;
        // SAFETY: The pattern belongs to the process-checked settings control.
        unsafe { pattern.Toggle() }.ok().map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "query and scroll the owned settings viewport through live UI Automation interfaces"
    )]
    pub(super) fn show_control(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<bool> {
        use circadia::automation_id;
        use windows::Win32::uiautomationclient::IUIAutomationScrollPattern;
        use windows::Win32::uiautomationclient::UIA_ScrollPatternId;
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
        // SAFETY: The live graph returns its screen rectangle into owned
        // binding storage.
        let bounds = unsafe { graph.CurrentBoundingRectangle() }.map_err(io::Error::other)?;
        // SAFETY: The viewport is an owned UI Automation element.
        let mut viewport =
            unsafe { scroll.CurrentBoundingRectangle() }.map_err(io::Error::other)?;
        let outer = window.GetWindowRect().map_err(io::Error::other)?;
        viewport.top = viewport.top.max(outer.top);
        viewport.bottom = viewport.bottom.min(outer.bottom);
        if bounds.top >= viewport.top
            && bounds.bottom <= viewport.bottom
            && bounds.bottom > bounds.top
            && bounds.right > bounds.left
        {
            return Ok(true);
        }
        // SAFETY: The requested interface matches the scroll pattern
        // identifier.
        let pattern: IUIAutomationScrollPattern =
            unsafe { scroll.GetCurrentPatternAs(UIA_ScrollPatternId) }.map_err(io::Error::other)?;
        // SAFETY: The live pattern returns scalar scroll metadata.
        let visible = unsafe { pattern.CurrentVerticalViewSize() }.map_err(io::Error::other)?;
        // SAFETY: The live pattern returns its current vertical percentage.
        let current =
            unsafe { pattern.CurrentVerticalScrollPercent() }.map_err(io::Error::other)?;
        let extent = f64::from(viewport.bottom - viewport.top) * (100.0 / visible - 1.0);
        if !extent.is_finite() || extent <= 0.0 {
            return Err(io::Error::other(format!(
                "invalid graph scroll extent: {extent}; view size {visible}; graph {bounds:?}; viewport {viewport:?}"
            )));
        }
        let next = if bounds.bottom <= bounds.top {
            if current >= 100.0 {
                return Err(io::Error::other(
                    "graph did not become visible after scrolling",
                ));
            }
            current + 5.0
        } else {
            let delta =
                f64::from(bounds.top + bounds.bottom - viewport.top - viewport.bottom) / 2.0;
            current + delta / extent * 100.0
        };
        // SAFETY: Both percentages are valid; -1 preserves horizontal
        // scrolling.
        unsafe { pattern.SetScrollPercent(-1.0, next.clamp(0.0, 100.0)) }
            .ok()
            .map_err(io::Error::other)?;
        Ok(false)
    }

    #[expect(
        unsafe_code,
        reason = "read the enabled state of an owned settings control"
    )]
    pub(super) fn enabled(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<bool>> {
        self.find(window, process, id)?
            .map(|element| {
                // SAFETY: The live interface returns a scalar through owned
                // binding storage.
                unsafe { element.CurrentIsEnabled() }
                    .map(windows::core::BOOL::as_bool)
                    .map_err(io::Error::other)
            })
            .transpose()
    }

    #[expect(
        unsafe_code,
        reason = "read a text value from the owned settings window"
    )]
    pub(super) fn value(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<String>> {
        self.find(window, process, id)?
            .map(|element| {
                // SAFETY: The pattern identifier matches the requested
                // interface.
                let pattern: IUIAutomationValuePattern =
                    unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) }
                        .map_err(io::Error::other)?;
                // SAFETY: The binding returns an owned BSTR from the live
                // pattern.
                unsafe { pattern.CurrentValue() }
                    .map(|value| value.display().to_string())
                    .map_err(io::Error::other)
            })
            .transpose()
    }

    #[expect(
        unsafe_code,
        reason = "edit an identified control through its UI Automation value pattern"
    )]
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
        // SAFETY: The pattern identifier matches the requested interface.
        let pattern: IUIAutomationValuePattern =
            unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) }.map_err(io::Error::other)?;
        let value = windows::core::BSTR::from(value);
        // SAFETY: The owned BSTR remains live for this synchronous call.
        unsafe { pattern.SetValue(&value) }
            .ok()
            .map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "invoke the one enabled button of an open dialog in the owned window"
    )]
    pub(super) fn dialog_button(
        &self,
        window: &winsafe::HWND,
        process: u32,
        button: DialogButton,
    ) -> io::Result<bool> {
        let id = button.id();
        let mut matches = Vec::new();
        for element in self.dialog_buttons(window, process, button)? {
            // SAFETY: The live interface returns a scalar through owned binding
            // storage.
            if unsafe { element.CurrentIsEnabled() }
                .map_err(io::Error::other)?
                .as_bool()
            {
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
        // SAFETY: The pattern identifier matches the requested interface.
        let pattern: IUIAutomationInvokePattern =
            unsafe { element.GetCurrentPatternAs(UIA_InvokePatternId) }
                .map_err(io::Error::other)?;
        // SAFETY: The live pattern has no borrowed arguments.
        unsafe { pattern.Invoke() }.ok().map_err(io::Error::other)?;
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
    #[expect(
        unsafe_code,
        reason = "select dialog buttons by type and ancestry in the owned window"
    )]
    fn dialog_buttons(
        &self,
        window: &winsafe::HWND,
        process: u32,
        button: DialogButton,
    ) -> io::Result<Vec<IUIAutomationElement>> {
        // SAFETY: The initialized apartment and live client own the walker.
        let walker = unsafe { self.client.ControlViewWalker() }.map_err(io::Error::other)?;
        let mut buttons = Vec::new();
        for element in self.elements(window, process)? {
            if element_id(&element)? != button.id() {
                continue;
            }
            // SAFETY: The live interface returns a scalar through owned binding
            // storage.
            if unsafe { element.CurrentControlType() }.map_err(io::Error::other)?
                != UIA_ButtonControlTypeId
            {
                continue;
            }
            let mut current = element.clone();
            let mut banner = false;
            // SAFETY: Both interfaces are live; the root's missing parent
            // returns an error that ends the walk.
            while let Ok(parent) = unsafe { walker.GetParentElement(&current) } {
                banner |= element_id(&parent)?.starts_with("banner-");
                current = parent;
            }
            if !banner {
                buttons.push(element);
            }
        }
        Ok(buttons)
    }

    #[expect(
        unsafe_code,
        reason = "read the toggle state of an owned settings control"
    )]
    pub(super) fn toggled(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<bool>> {
        use windows::Win32::uiautomationclient::IUIAutomationTogglePattern;
        use windows::Win32::uiautomationclient::UIA_TogglePatternId;
        use windows::Win32::uiautomationcore::ToggleState_On;
        self.find(window, process, id)?
            .map(|element| {
                // SAFETY: The requested interface matches the toggle pattern
                // identifier.
                let pattern: IUIAutomationTogglePattern =
                    unsafe { element.GetCurrentPatternAs(UIA_TogglePatternId) }
                        .map_err(io::Error::other)?;
                // SAFETY: The live pattern returns a scalar state.
                unsafe { pattern.CurrentToggleState() }
                    .map(|state| state == ToggleState_On)
                    .map_err(io::Error::other)
            })
            .transpose()
    }

    #[expect(
        unsafe_code,
        reason = "move keyboard focus to an identified control in the owned settings window"
    )]
    pub(super) fn focus_control(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<()> {
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        // SAFETY: The live element belongs to the checked settings process;
        // SetFocus has no pointer parameters.
        unsafe { element.SetFocus() }.ok().map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "create a UI Automation client in the initialized MTA"
    )]
    pub(super) fn new() -> io::Result<Self> {
        let apartment =
            winsafe::CoInitializeEx(co::COINIT::MULTITHREADED).map_err(io::Error::other)?;
        // SAFETY: COM is initialized on this thread. The owned client drops
        // before its apartment guard, and this tool uses both on this thread.
        let client =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER.cast_unsigned()) }
                .map_err(io::Error::other)?;
        Ok(Self {
            client,
            _apartment: apartment,
        })
    }

    #[expect(
        unsafe_code,
        reason = "query a live COM element for the owned child window"
    )]
    pub(super) fn root(
        &self,
        window: &winsafe::HWND,
        process: u32,
    ) -> io::Result<IUIAutomationElement> {
        // SAFETY: The handle is used as an opaque window identifier. COM is
        // initialized and owns the returned interface; stale handles can fail.
        let root = unsafe { self.client.ElementFromHandle(window.ptr().cast()) }
            .map_err(io::Error::other)?;
        // SAFETY: The interface is live in this apartment; the binding owns
        // its output storage.
        let owner = unsafe { root.CurrentProcessId() }.map_err(io::Error::other)?;
        if u32::try_from(owner).ok() != Some(process) {
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
    ) -> io::Result<Option<IUIAutomationElement>> {
        self.elements(window, process)?
            .into_iter()
            .find_map(|element| {
                element_id(&element)
                    .map(|actual| (actual == id).then_some(element))
                    .transpose()
            })
            .transpose()
    }

    #[expect(
        unsafe_code,
        reason = "compare two process-checked UI Automation elements"
    )]
    pub(super) fn same_control(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
        previous: &IUIAutomationElement,
    ) -> io::Result<bool> {
        let current = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        // SAFETY: Both elements and the client remain live in the initialized
        // apartment.
        unsafe { self.client.CompareElements(previous, &current) }
            .map(windows_core::BOOL::as_bool)
            .map_err(io::Error::other)
    }

    pub(super) fn elements(
        &self,
        window: &winsafe::HWND,
        process: u32,
    ) -> io::Result<Vec<IUIAutomationElement>> {
        let root = self.root(window, process)?;
        self.descendants(&root)
    }

    #[expect(
        unsafe_code,
        reason = "enumerate the UI Automation subtree of a process-checked element"
    )]
    fn descendants(&self, root: &IUIAutomationElement) -> io::Result<Vec<IUIAutomationElement>> {
        // SAFETY: The initialized apartment and live client own the result.
        let condition = unsafe { self.client.CreateTrueCondition() }.map_err(io::Error::other)?;
        // SAFETY: Both interfaces remain live during the synchronous call.
        // Search is restricted to the verified element's subtree.
        let elements =
            unsafe { root.FindAll(TreeScope_Descendants, &condition) }.map_err(io::Error::other)?;
        // SAFETY: The returned COM array remains live through enumeration.
        let count = unsafe { elements.Length() }.map_err(io::Error::other)?;
        let mut found = Vec::new();
        for index in 0..count {
            // SAFETY: The index is within the array length just queried.
            let element = unsafe { elements.GetElement(index) }.map_err(io::Error::other)?;
            found.push(element);
        }
        Ok(found)
    }

    #[expect(
        unsafe_code,
        reason = "read an accessible name from the owned application"
    )]
    pub(super) fn text(
        &self,
        window: &winsafe::HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Option<String>> {
        self.find(window, process, id)?
            .map(|element| {
                // SAFETY: The element is live in the initialized apartment;
                // the binding returns an owned BSTR.
                unsafe { element.CurrentName() }
                    .map(|name| name.display().to_string())
                    .map_err(io::Error::other)
            })
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

    #[expect(
        unsafe_code,
        reason = "read every element of the owned window with its UI Automation parent"
    )]
    pub(super) fn controls_with_parents(
        &self,
        window: &winsafe::HWND,
        process: u32,
    ) -> io::Result<Vec<(Control, Option<Control>)>> {
        // SAFETY: The initialized apartment and live client own the walker.
        let walker = unsafe { self.client.ControlViewWalker() }.map_err(io::Error::other)?;
        self.elements(window, process)?
            .iter()
            .map(|element| {
                // SAFETY: Both interfaces are live; an element without a parent
                // returns an error.
                let parent = unsafe { walker.GetParentElement(element) }
                    .ok()
                    .map(|parent| Control::read(&parent))
                    .transpose()?;
                Ok((Control::read(element)?, parent))
            })
            .collect()
    }

    #[expect(
        unsafe_code,
        reason = "read the keyboard focus and its identified ancestor in the owned process"
    )]
    pub(super) fn focused(&self, process: u32) -> io::Result<Option<Focused>> {
        // SAFETY: The initialized apartment and live client own the result.
        let element = unsafe { self.client.GetFocusedElement() }.map_err(io::Error::other)?;
        // SAFETY: The live element returns a scalar process identifier.
        let owner = unsafe { element.CurrentProcessId() }.map_err(io::Error::other)?;
        if u32::try_from(owner).ok() != Some(process) {
            return Ok(None);
        }
        let control = Control::read(&element)?;
        // SAFETY: The initialized apartment and live client own the walker.
        let walker = unsafe { self.client.ControlViewWalker() }.map_err(io::Error::other)?;
        let mut identifiers = vec![control.id.clone()];
        let mut current = element;
        // SAFETY: Both interfaces are live; the root's missing parent returns
        // an error that ends the walk.
        while let Ok(parent) = unsafe { walker.GetParentElement(&current) } {
            identifiers.push(element_id(&parent)?);
            current = parent;
        }
        identifiers.retain(|id| !id.is_empty());
        Ok(Some(Focused {
            control,
            identifiers,
        }))
    }

    #[expect(
        unsafe_code,
        reason = "read the accessible names in the owned application window"
    )]
    pub(super) fn names(&self, window: &winsafe::HWND, process: u32) -> io::Result<Vec<String>> {
        self.elements(window, process)?
            .iter()
            .map(|element| {
                // SAFETY: The element is live in the initialized apartment;
                // the binding returns an owned BSTR.
                unsafe { element.CurrentName() }
                    .map(|name| name.display().to_string())
                    .map_err(io::Error::other)
            })
            .collect()
    }

    #[expect(
        unsafe_code,
        reason = "invoke a named control in the owned application"
    )]
    pub(super) fn invoke(&self, window: &winsafe::HWND, process: u32, id: &str) -> io::Result<()> {
        let element = self.find(window, process, id)?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("control {id} is absent"))
        })?;
        // SAFETY: The requested pattern ID matches its interface type.
        let pattern: IUIAutomationInvokePattern =
            unsafe { element.GetCurrentPatternAs(UIA_InvokePatternId) }
                .map_err(io::Error::other)?;
        // SAFETY: The live pattern has no borrowed inputs or output pointers.
        unsafe { pattern.Invoke() }.ok().map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "close only the owned settings window through UI Automation"
    )]
    pub(super) fn close(&self, window: &winsafe::HWND, process: u32) -> io::Result<()> {
        let root = self.root(window, process)?;
        // SAFETY: The requested pattern ID matches its interface type.
        let pattern: IUIAutomationWindowPattern =
            unsafe { root.GetCurrentPatternAs(UIA_WindowPatternId) }.map_err(io::Error::other)?;
        // SAFETY: The live pattern has no borrowed inputs or output pointers.
        unsafe { pattern.Close() }.ok().map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "focus only the owned application's settings window"
    )]
    pub(super) fn focus(&self, window: &winsafe::HWND, process: u32) -> io::Result<()> {
        let root = self.root(window, process)?;
        // SAFETY: The live interface belongs to the checked child process;
        // SetFocus has no pointer parameters.
        unsafe { root.SetFocus() }.ok().map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "minimize the owned window before testing reactivation"
    )]
    pub(super) fn minimize(&self, window: &winsafe::HWND, process: u32) -> io::Result<()> {
        let root = self.root(window, process)?;
        // SAFETY: The requested pattern ID matches its interface type.
        let pattern: IUIAutomationWindowPattern =
            unsafe { root.GetCurrentPatternAs(UIA_WindowPatternId) }.map_err(io::Error::other)?;
        // SAFETY: The live pattern receives a documented scalar state.
        unsafe { pattern.SetWindowVisualState(WindowVisualState_Minimized) }
            .ok()
            .map_err(io::Error::other)
    }
}

pub(super) struct Scroll {
    pub(super) id: String,
    pattern: windows::Win32::uiautomationclient::IUIAutomationScrollPattern,
}

fn optional_scroll(
    result: windows_core::Result<windows::Win32::uiautomationclient::IUIAutomationScrollPattern>,
) -> io::Result<Option<windows::Win32::uiautomationclient::IUIAutomationScrollPattern>> {
    match result {
        Ok(pattern) => Ok(Some(pattern)),
        // WinUI can return S_OK with a null pattern; the projection reports Error::empty().
        Err(error)
            if error.code().0 == 0
                || error.code().0 == windows_sys::Win32::winerror::E_NOINTERFACE
                || error.code().0.cast_unsigned()
                    == windows::Win32::uiautomationcoreapi::UIA_E_NOTSUPPORTED =>
        {
            Ok(None)
        }
        Err(error) => Err(io::Error::other(error)),
    }
}

impl Scroll {
    #[expect(
        unsafe_code,
        reason = "read scalar scroll metadata from the live UI Automation pattern"
    )]
    pub(super) fn state(&self) -> io::Result<Option<(f64, f64)>> {
        // SAFETY: The live pattern writes scalar results into binding-owned
        // storage.
        if !unsafe { self.pattern.CurrentVerticallyScrollable() }
            .map_err(io::Error::other)?
            .as_bool()
        {
            return Ok(None);
        }
        // SAFETY: The live pattern writes a scalar into binding-owned storage.
        let position =
            unsafe { self.pattern.CurrentVerticalScrollPercent() }.map_err(io::Error::other)?;
        // SAFETY: The live pattern writes a scalar into binding-owned storage.
        let view = unsafe { self.pattern.CurrentVerticalViewSize() }.map_err(io::Error::other)?;
        Ok(Some((position, view)))
    }

    #[expect(
        unsafe_code,
        reason = "scroll a process-checked viewport with validated percentages"
    )]
    pub(super) fn set(&self, position: f64) -> io::Result<()> {
        if !position.is_finite() || !(0.0..=100.0).contains(&position) {
            return Err(io::Error::other("invalid scroll percentage"));
        }
        // SAFETY: The live pattern accepts a finite percentage; -1 preserves
        // horizontal scrolling.
        unsafe { self.pattern.SetScrollPercent(-1.0, position) }
            .ok()
            .map_err(io::Error::other)
    }
}

#[derive(Clone, Debug)]
pub(super) struct Control {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) kind: i32,
    pub(super) enabled: bool,
    pub(super) bounds: (i32, i32, i32, i32),
}

impl Control {
    #[expect(
        unsafe_code,
        reason = "read scalar and string properties of a live UI Automation element"
    )]
    fn read(element: &IUIAutomationElement) -> io::Result<Self> {
        // SAFETY: The binding returns an owned BSTR from the live element.
        let name = unsafe { element.CurrentName() }.map_err(io::Error::other)?;
        // SAFETY: The live element returns a scalar control type.
        let kind = unsafe { element.CurrentControlType() }.map_err(io::Error::other)?;
        // SAFETY: The live element returns a scalar enabled state.
        let enabled = unsafe { element.CurrentIsEnabled() }.map_err(io::Error::other)?;
        // SAFETY: The live element returns its screen rectangle by value.
        let bounds = unsafe { element.CurrentBoundingRectangle() }.map_err(io::Error::other)?;
        Ok(Self {
            id: element_id(element)?,
            name: name.display().to_string(),
            kind,
            enabled: enabled.as_bool(),
            bounds: (bounds.left, bounds.top, bounds.right, bounds.bottom),
        })
    }

    pub(super) fn interactive(&self) -> bool {
        [
            UIA_ButtonControlTypeId,
            UIA_CheckBoxControlTypeId,
            UIA_ComboBoxControlTypeId,
            UIA_EditControlTypeId,
            UIA_HyperlinkControlTypeId,
            UIA_SliderControlTypeId,
            UIA_SpinnerControlTypeId,
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
                .and_then(|source| source.downcast_ref::<windows::core::Error>())
                .is_some_and(|source| {
                    matches!(
                        source.code().0.cast_unsigned(),
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

#[expect(
    unsafe_code,
    reason = "read an owned automation identifier from a live element"
)]
fn element_id(element: &IUIAutomationElement) -> io::Result<String> {
    // SAFETY: The live interface returns an owned BSTR through the binding.
    unsafe { element.CurrentAutomationId() }
        .map(|id| id.display().to_string())
        .map_err(io::Error::other)
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
        let error = windows_core::Error::from_hresult(windows_core::HRESULT(code.cast_signed()));
        assert!(matches!(optional_scroll(Err(error)), Ok(None)));
    }

    #[test_case(UIA_E_ELEMENTNOTAVAILABLE)]
    #[test_case(UIA_E_TIMEOUT)]
    #[test_case(0x8007_0005)]
    fn scroll_observation_failures_are_reported(code: u32) {
        let error = windows_core::Error::from_hresult(windows_core::HRESULT(code.cast_signed()));
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
                Err(io::Error::other(windows::core::Error::from_hresult(
                    windows::core::HRESULT(code.cast_signed()),
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
            Err(io::Error::other(windows::core::Error::from_hresult(
                windows::core::HRESULT(0x8007_0005_u32.cast_signed()),
            )))
        });
        let error = result.expect_err("access denied remains an error");
        let source = error
            .get_ref()
            .and_then(|error| error.source())
            .and_then(|error| error.downcast_ref::<io::Error>())
            .and_then(io::Error::get_ref)
            .and_then(|error| error.downcast_ref::<windows::core::Error>());
        assert_eq!(
            source.map(|error| error.code().0.cast_unsigned()),
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
                1 => Err(io::Error::other(windows::core::Error::from_hresult(
                    windows::core::HRESULT(code.cast_signed()),
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
