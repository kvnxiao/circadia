use std::io;
use std::time::Instant;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::CLSCTX_INPROC_SERVER;
use windows::Win32::System::Com::CoCreateInstance;
use windows::Win32::UI::Accessibility::CUIAutomation;
use windows::Win32::UI::Accessibility::IUIAutomation;
use windows::Win32::UI::Accessibility::IUIAutomationElement;
use windows::Win32::UI::Accessibility::IUIAutomationInvokePattern;
use windows::Win32::UI::Accessibility::IUIAutomationValuePattern;
use windows::Win32::UI::Accessibility::IUIAutomationWindowPattern;
use windows::Win32::UI::Accessibility::TreeScope_Descendants;
use windows::Win32::UI::Accessibility::UIA_ButtonControlTypeId;
use windows::Win32::UI::Accessibility::UIA_InvokePatternId;
use windows::Win32::UI::Accessibility::UIA_ValuePatternId;
use windows::Win32::UI::Accessibility::UIA_WindowPatternId;
use windows::Win32::UI::Accessibility::WindowVisualState_Minimized;
use winsafe::co;
use winsafe::guard::CoUninitializeGuard;

pub(super) struct Automation {
    client: IUIAutomation,
    _apartment: CoUninitializeGuard,
}

impl Automation {
    #[cfg(feature = "graph-measurement")]
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
        use windows::Win32::UI::Accessibility::IUIAutomationRangeValuePattern;
        use windows::Win32::UI::Accessibility::UIA_RangeValuePatternId;
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
        reason = "select a navigation item in the owned settings window"
    )]
    pub(super) fn select(&self, window: &winsafe::HWND, process: u32, id: &str) -> io::Result<()> {
        use windows::Win32::UI::Accessibility::IUIAutomationSelectionItemPattern;
        use windows::Win32::UI::Accessibility::UIA_SelectionItemPatternId;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        // SAFETY: The pattern identifier matches the requested interface.
        let pattern: IUIAutomationSelectionItemPattern =
            unsafe { element.GetCurrentPatternAs(UIA_SelectionItemPatternId) }
                .map_err(io::Error::other)?;
        // SAFETY: The live pattern has no borrowed inputs or output pointers.
        unsafe { pattern.Select() }.map_err(io::Error::other)
    }

    #[cfg(feature = "graph-measurement")]
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
        use windows::Win32::UI::Accessibility::IUIAutomationRangeValuePattern;
        use windows::Win32::UI::Accessibility::UIA_RangeValuePatternId;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other(format!("control {id} is absent")))?;
        // SAFETY: The pattern identifier matches the requested interface.
        let pattern: IUIAutomationRangeValuePattern =
            unsafe { element.GetCurrentPatternAs(UIA_RangeValuePatternId) }
                .map_err(io::Error::other)?;
        // SAFETY: The live pattern takes a scalar.
        unsafe { pattern.SetValue(value) }.map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "toggle an identified control in the owned settings window"
    )]
    pub(super) fn toggle(&self, window: &winsafe::HWND, process: u32, id: &str) -> io::Result<()> {
        use windows::Win32::UI::Accessibility::IUIAutomationTogglePattern;
        use windows::Win32::UI::Accessibility::UIA_TogglePatternId;
        let element = self
            .find(window, process, id)?
            .ok_or_else(|| io::Error::other("toggle control not found"))?;
        // SAFETY: The requested interface matches the toggle pattern
        // identifier.
        let pattern: IUIAutomationTogglePattern =
            unsafe { element.GetCurrentPatternAs(UIA_TogglePatternId) }
                .map_err(io::Error::other)?;
        // SAFETY: The pattern belongs to the process-checked settings control.
        unsafe { pattern.Toggle() }.map_err(io::Error::other)
    }

    #[cfg(feature = "graph-measurement")]
    #[expect(
        unsafe_code,
        reason = "query and scroll the owned settings viewport through live UI Automation interfaces"
    )]
    pub(super) fn show_graph(&self, window: &winsafe::HWND, process: u32) -> io::Result<bool> {
        use windows::Win32::UI::Accessibility::IUIAutomationScrollPattern;
        use windows::Win32::UI::Accessibility::UIA_ScrollPatternId;
        let Some(graph) = self.find(window, process, "graph")? else {
            return Err(io::Error::other(
                "generated graph has no UI Automation element",
            ));
        };
        let Some(scroll) = self.find(window, process, "settings-scroll")? else {
            return Err(io::Error::other(
                "settings viewport has no UI Automation element",
            ));
        };
        // SAFETY: The live graph returns its screen rectangle into owned
        // binding storage.
        let bounds = unsafe { graph.CurrentBoundingRectangle() }.map_err(io::Error::other)?;
        // SAFETY: The viewport is an owned UI Automation element.
        let viewport = unsafe { scroll.CurrentBoundingRectangle() }.map_err(io::Error::other)?;
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
                    .map(|value| value.to_string())
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
        unsafe { pattern.SetValue(&value) }.map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "invoke a uniquely named enabled dialog button in the owned window"
    )]
    pub(super) fn dialog_button(
        &self,
        window: &winsafe::HWND,
        process: u32,
        name: &str,
    ) -> io::Result<bool> {
        let mut matches = Vec::new();
        for element in self.elements(window, process)? {
            if !matches!(
                element_id(&element)?.as_str(),
                "PrimaryButton" | "SecondaryButton" | "CloseButton"
            ) {
                continue;
            }
            // SAFETY: The live interface returns a scalar through owned binding
            // storage.
            if unsafe { element.CurrentControlType() }.map_err(io::Error::other)?
                != UIA_ButtonControlTypeId
            {
                continue;
            }
            // SAFETY: The binding returns an owned BSTR from the live element.
            if unsafe { element.CurrentName() }.map_err(io::Error::other)? != name {
                continue;
            }
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
            let identifiers = matches
                .iter()
                .map(element_id)
                .collect::<io::Result<Vec<_>>>()?;
            return Err(io::Error::other(format!(
                "dialog button {name} is ambiguous: {identifiers:?}"
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
        unsafe { pattern.Invoke() }.map_err(io::Error::other)?;
        Ok(true)
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
        let client = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
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
        let root = unsafe { self.client.ElementFromHandle(HWND(window.ptr())) }
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

    fn find(
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
        reason = "enumerate a verified window's UI Automation subtree"
    )]
    pub(super) fn elements(
        &self,
        window: &winsafe::HWND,
        process: u32,
    ) -> io::Result<Vec<IUIAutomationElement>> {
        let root = self.root(window, process)?;
        // SAFETY: The initialized apartment and live client own the result.
        let condition = unsafe { self.client.CreateTrueCondition() }.map_err(io::Error::other)?;
        // SAFETY: Both interfaces remain live during the synchronous call.
        // Search is restricted to the verified window, not the desktop.
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
                    .map(|name| name.to_string())
                    .map_err(io::Error::other)
            })
            .transpose()
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
        unsafe { pattern.Invoke() }.map_err(io::Error::other)
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
        unsafe { pattern.Close() }.map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "focus only the owned application's settings window"
    )]
    pub(super) fn focus(&self, window: &winsafe::HWND, process: u32) -> io::Result<()> {
        let root = self.root(window, process)?;
        // SAFETY: The live interface belongs to the checked child process;
        // SetFocus has no pointer parameters.
        unsafe { root.SetFocus() }.map_err(io::Error::other)
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
            .map_err(io::Error::other)
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
                        windows::Win32::UI::Accessibility::UIA_E_ELEMENTNOTAVAILABLE
                            | windows::Win32::UI::Accessibility::UIA_E_TIMEOUT
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
        .map(|id| id.to_string())
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;
    use windows::Win32::UI::Accessibility::UIA_E_ELEMENTNOTAVAILABLE;
    use windows::Win32::UI::Accessibility::UIA_E_TIMEOUT;

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
}
