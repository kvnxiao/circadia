use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::require;
use circadia::automation_id;
use std::io;
use std::time::Duration;
use winsafe::HWND;

pub(super) fn resize_client(window: &HWND, width: i32, height: i32) -> io::Result<()> {
    let outer = window.GetWindowRect().map_err(io::Error::other)?;
    let client = window.GetClientRect().map_err(io::Error::other)?;
    window
        .SetWindowPos(
            winsafe::HwndPlace::None,
            winsafe::POINT::new(),
            winsafe::SIZE::with(
                width + outer.right - outer.left - client.right,
                height + outer.bottom - outer.top - client.bottom,
            ),
            winsafe::co::SWP::NOMOVE | winsafe::co::SWP::NOZORDER | winsafe::co::SWP::NOACTIVATE,
        )
        .map_err(io::Error::other)?;
    wait("requested client dimensions", || {
        let client = window.GetClientRect().map_err(io::Error::other)?;
        Ok((client.right == width && client.bottom == height).then_some(()))
    })?;
    std::thread::sleep(Duration::from_millis(250));
    Ok(())
}

pub(super) fn exercise(automation: &Automation, window: &HWND, process: u32) -> io::Result<()> {
    let area = window
        .MonitorFromWindow(winsafe::co::MONITOR::DEFAULTTONEAREST)
        .GetMonitorInfo()
        .map_err(io::Error::other)?
        .rcWork;
    let outer = window.GetWindowRect().map_err(io::Error::other)?;
    require(
        outer.left >= area.left
            && outer.top >= area.top
            && outer.right <= area.right
            && outer.bottom <= area.bottom,
        "initial outer window exceeds the actual monitor work area",
    )?;
    resize_client(window, 800, 600)?;
    require(
        automation.visible(window, process, automation_id::NAV_GENERAL)?,
        "800 epx navigation is not expanded",
    )?;
    resize_client(window, 799, 600)?;
    require(
        !automation.visible(window, process, automation_id::NAV_GENERAL)?,
        "799 epx navigation remains expanded",
    )?;
    let page = automation.bounds(window, process, automation_id::SCROLL)?;
    let button = automation.navigation_button(window, process)?;
    automation.focus_control(window, process, &button)?;
    automation.invoke(window, process, &button)?;
    wait("navigation overlay opens", || {
        Ok(automation
            .visible(window, process, automation_id::NAV_GENERAL)?
            .then_some(()))
    })?;
    std::thread::sleep(Duration::from_millis(250));
    let opened = automation.bounds(window, process, automation_id::SCROLL)?;
    require(
        page.left == opened.left && page.right == opened.right,
        "opening navigation moved or narrowed the page",
    )?;
    crate::automation::press_key(winsafe::co::VK::ESCAPE)?;
    wait("Escape closes navigation", || {
        Ok((!automation.visible(window, process, automation_id::NAV_GENERAL)?).then_some(()))
    })?;
    require(
        automation
            .focused(process)?
            .is_some_and(|focus| focus.identifiers.contains(&button)),
        "Escape did not return focus to the navigation button",
    )?;
    automation.invoke(window, process, &button)?;
    wait("navigation opens before outside dismissal", || {
        Ok(automation
            .visible(window, process, automation_id::NAV_GENERAL)?
            .then_some(()))
    })?;
    std::thread::sleep(Duration::from_millis(250));
    crate::automation::click_point(page.right - 20, page.top + 20)?;
    wait("outside click closes navigation", || {
        Ok((!automation.visible(window, process, automation_id::NAV_GENERAL)?).then_some(()))
    })?;
    std::thread::sleep(Duration::from_millis(250));
    automation.invoke(window, process, &button)?;
    wait("navigation overlay reopens", || {
        Ok(automation
            .visible(window, process, automation_id::NAV_GENERAL)?
            .then_some(()))
    })?;
    automation.select(window, process, automation_id::NAV_GENERAL)?;
    wait("selecting a page dismisses navigation", || {
        Ok((!automation.visible(window, process, automation_id::NAV_GENERAL)?).then_some(()))
    })?;
    require(
        automation
            .enabled(window, process, automation_id::LANGUAGE)?
            .is_some(),
        "narrow selection did not show General",
    )?;
    resize_client(window, 800, 600)?;
    require(
        automation
            .enabled(window, process, automation_id::LANGUAGE)?
            .is_some(),
        "resizing lost the selected page",
    )?;
    reflow_focus(automation, window, process)?;
    crate::lifecycle::report(
        "PASS: 799/800 epx navigation, overlay geometry, selection, Escape/outside dismissal, focus and number value through reflow",
    );
    Ok(())
}

fn reflow_focus(automation: &Automation, window: &HWND, process: u32) -> io::Result<()> {
    crate::lifecycle::show_page(automation, window, process, automation_id::NAV_TRANSITIONS)?;
    automation.focus_control(window, process, automation_id::RATE_NUMBER)?;
    let value = automation.number(window, process, automation_id::RATE_NUMBER)?;
    resize_client(window, 1200, 600)?;
    require(
        automation.focused(process)?.is_some_and(|focus| {
            focus
                .identifiers
                .iter()
                .any(|id| id == automation_id::RATE_NUMBER)
        }),
        "row reflow lost number-field focus",
    )?;
    require(
        automation.number(window, process, automation_id::RATE_NUMBER)? == value,
        "row reflow changed the number-field value",
    )?;
    Ok(())
}
