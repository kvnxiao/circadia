pub(crate) mod clock;
pub(crate) mod event_log;
pub(crate) mod events;
pub(crate) mod foreground;
pub(crate) mod instance;
pub(crate) mod lifecycle_inputs;
#[expect(
    dead_code,
    reason = "the solar timing settings request the Windows location"
)]
pub(crate) mod location;
pub(crate) mod nls;
#[expect(dead_code, reason = "the App suppression page browses for executables")]
pub(crate) mod picker;
pub(crate) mod settings_window;
pub(crate) mod startup;
pub(crate) mod wake;
