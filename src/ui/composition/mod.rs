#[expect(
    clippy::allow_attributes,
    reason = "scope generated projection lint exceptions"
)]
#[allow(
    unsafe_code,
    dead_code,
    missing_docs,
    unreachable_pub,
    unnameable_types,
    elided_lifetimes_in_paths,
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::all,
    clippy::pedantic,
    clippy::undocumented_unsafe_blocks,
    clippy::multiple_unsafe_ops_per_block,
    reason = "metadata-generated WinRT projection"
)]
pub(crate) mod bindings;

pub(crate) fn on_rendering(
    callback: impl Fn(std::time::Instant) + 'static,
) -> windows_core::Result<windows_core::EventRevoker> {
    bindings::CompositionTarget::Rendering(move |_, _| {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            callback(std::time::Instant::now());
        }))
        .is_err()
        {
            std::process::abort();
        }
    })
}
