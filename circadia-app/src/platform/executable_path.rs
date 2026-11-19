use std::cmp::Ordering;

#[expect(
    clippy::panic,
    reason = "constructed identities guarantee valid API parameters; an OS failure cannot supply an ordering"
)]
pub(crate) fn compare(left: &[u16], right: &[u16]) -> Ordering {
    circadia_winsafe::ordinal::compare(left, right).unwrap_or_else(|error| {
        panic!("CompareStringOrdinal rejected validated executable identities: {error}")
    })
}
