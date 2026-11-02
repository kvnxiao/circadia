use std::cmp::Ordering;
use windows_sys::Win32::stringapiset::CompareStringOrdinal;

#[expect(
    unsafe_code,
    reason = "CompareStringOrdinal reads validated UTF-16 slices"
)]
#[expect(
    clippy::panic,
    reason = "constructed identities guarantee valid API parameters; an OS failure cannot supply an ordering"
)]
pub(crate) fn compare(left: &[u16], left_len: i32, right: &[u16], right_len: i32) -> Ordering {
    // SAFETY: Identity constructors validate the UTF-16 lengths, and both
    // slices remain allocated for the synchronous call.
    match unsafe { CompareStringOrdinal(left.as_ptr(), left_len, right.as_ptr(), right_len, 1) } {
        1 => Ordering::Less,
        2 => Ordering::Equal,
        3 => Ordering::Greater,
        _ => panic!("CompareStringOrdinal rejected validated executable identities"),
    }
}
