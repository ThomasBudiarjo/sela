//! Process-wide timer resolution for the safety lane.
//!
//! The only unsafe code in Sela (docs/security.md, "Unsafe code"): one
//! Windows API call without memory arguments.

#[cfg(windows)]
#[allow(unsafe_code)]
#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
}

/// Asks Windows for 1 ms timer granularity for the rest of the process.
/// The default 15.6 ms rounds every 16 ms poll or frame wait up to two ticks,
/// which delays Black/Clear/Logo receipts and their repaint by tens of ms.
#[allow(unsafe_code)]
pub fn raise_resolution() {
    #[cfg(windows)]
    // SAFETY: timeBeginPeriod takes a plain integer and has no memory
    // preconditions; the request lasts until the process exits.
    unsafe {
        timeBeginPeriod(1);
    }
}
