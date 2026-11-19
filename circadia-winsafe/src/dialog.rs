//! Close an active Windows file picker.

use std::io;
use winsafe::co;
use winsafe::prelude::*;

/// Close a file dialog with the cancellation HRESULT.
///
/// # Errors
/// Returns the COM error if closing the dialog fails.
#[expect(
    unsafe_code,
    reason = "IFileDialog::Close requires an HRESULT cancellation value"
)]
pub fn cancel(dialog: &winsafe::IFileOpenDialog) -> io::Result<()> {
    // SAFETY: Close forwards the raw value as HRESULT despite accepting ERROR.
    let cancelled = unsafe { co::ERROR::from_raw(co::ERROR::CANCELLED.to_hresult().raw()) };
    dialog.Close(cancelled).map_err(io::Error::other)
}
