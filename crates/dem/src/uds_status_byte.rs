// ─────────────────────────────────────────────
// UdsStatusByte
// ─────────────────────────────────────────────

/// # UDS Status Byte
///
/// This module implements the `UdsStatusByte`, a bitfield representing the status of a diagnostic event
/// or DTC (Diagnostic Trouble Code) in accordance with UDS (Unified Diagnostic Services) principles.
/// It tracks confirmation states, latching behavior, and lifecycle flags to manage event debouncing
/// and reporting in automotive diagnostics.
///
/// ## Bitfield Overview
///
/// | Bit | Name                              | Meaning                                                                 |
/// |-----|-----------------------------------|-------------------------------------------------------------------------|
/// |  0  | Test Failed (tf)                 | Confirmed failure flag (`true` = event confirmed as failed)           |
/// |  1  | Test Failed This Operating Cycle (tftoc) | Latched failure flag; set with tf, never cleared                 |
/// |  2  | Pending DTC (pdtc)               | Indicates a pending diagnostic trouble code                           |
/// |  3  | Confirmed DTC (cdtc)             | Indicates a confirmed diagnostic trouble code                        |
/// |  4  | Test Not Complete Since Last Clear (tncslc) | Not-complete status since last clear                          |
/// |  5  | Test Failed Since Last Clear (tfslc) | Failure occurred since last clear                                |
/// |  6  | Test Not Complete This Operating Cycle (tnctoc) | Not-complete flag (`true` = event not yet confirmed)         |
/// |  7  | Warning Indicator Requested (wir) | Warning indicator (MIL) activation flag                                |
///
/// ## State Machine
///
/// Events start in a "not complete" state (`tnctoc = true`). As debouncing progresses, they may enter
/// "pre-failed" or "pre-passed" states. Once a threshold is reached, the event becomes "confirmed"
/// (`tf` set), and `tnctoc` is cleared. Latched flags like `tftoc` preserve history across cycles.
///
/// ## Design Notes
///
/// - Latched flags (e.g., `tftoc`, `tfslc`) are never cleared to maintain diagnostic history.
/// - `tnctoc` starts `true` and is cleared once an event reaches a confirmation threshold.
/// - See [`super::Event`] for how these flags are updated during event processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdsStatusByte(u8);

impl UdsStatusByte {
    pub const TF_BIT: u8 = 1 << 0; // bit 0
    pub const TFTOC_BIT: u8 = 1 << 1; // bit 1
    pub const PDTC_BIT: u8 = 1 << 2; // bit 2
    pub const CDTC_BIT: u8 = 1 << 3; // bit 3
    pub const TNCSLC_BIT: u8 = 1 << 4; // bit 4
    pub const TFSLC_BIT: u8 = 1 << 5; // bit 5
    pub const TNCTOC_BIT: u8 = 1 << 6; // bit 6
    pub const WIR_BIT: u8 = 1 << 7; // bit 7

    pub const fn from_raw(raw: u8) -> Self {
        UdsStatusByte(raw)
    }

    /// Initializes the status byte with the provided `mask`, but forces `tf` (bit 0) and `tftoc` (bit 1)
    /// to `false` (starting unconfirmed), and `tnctoc` (bit 6) to `true` (not yet complete).
    /// All other bits are taken from `mask`.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let s = UdsStatusByte::from_raw(255u8);
    /// assert!(s.tf());
    /// assert!(s.tnctoc());
    /// ```
    pub fn init(&mut self) {
        self.0 = (self.0 & !(Self::TF_BIT | Self::TFTOC_BIT)) | Self::TNCTOC_BIT;
    }

    /// Returns the `tf` flag (bit 0) — confirmed failure state.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0u8);
    /// assert!(!s.tf());
    /// s.set_tf(true);
    /// assert!(s.tf());
    /// ```
    pub fn tf(&self) -> bool {
        self.0 & Self::TF_BIT != 0
    }

    /// Sets the `tf` flag (bit 0).
    ///
    /// ## Side Effects
    ///
    /// If `val` is `true`, also sets `tftoc`, `pdtc`, and `tfslc` to latch the failure state,
    /// and clears `tnctoc` (event is now complete). If `val` is `false`, only `tf` is cleared;
    /// latched flags remain set.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_tf(true);
    /// assert!(s.tf());
    /// assert!(s.tftoc()); // latched
    /// assert!(s.tfslc()); // set
    /// assert!(!s.tnctoc()); // cleared
    /// ```
    pub fn set_tf(&mut self, val: bool) {
        if val {
            self.0 = self.0 | Self::TF_BIT | Self::TFTOC_BIT | Self::PDTC_BIT | Self::TFSLC_BIT; // sets
            self.set_tnctoc(false); // clears
        } else {
            self.0 &= !Self::TF_BIT;
            // tftoc and tfslc are not cleared here
            // tnctoc remains cleared
        }
    }

    /// Returns the `pdtc` flag (bit 2) — pending DTC indicator.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(1u8);
    /// s.set_tf(true);
    /// assert!(s.pdtc());
    /// ```
    pub fn pdtc(&self) -> bool {
        self.0 & Self::PDTC_BIT != 0
    }

    /// Sets the `pdtc` flag (bit 2).
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_pdtc(true);
    /// assert!(s.pdtc());
    /// ```
    pub fn set_pdtc(&mut self, val: bool) {
        if val {
            self.0 |= Self::PDTC_BIT; // sets
        } else {
            self.0 &= !Self::PDTC_BIT;
        }
    }

    /// Returns the `cdtc` flag (bit 3) — confirmed DTC indicator.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(1u8);
    /// assert!(!s.cdtc());
    /// s.set_cdtc(true);
    /// assert!(s.cdtc());
    /// ```
    pub fn cdtc(&self) -> bool {
        self.0 & Self::CDTC_BIT != 0
    }

    /// Sets the `cdtc` flag (bit 3).
    ///
    /// When `cdtc` is set to `true`, the `wir` (Warning Indicator Requested) flag is also set.
    /// When `cdtc` is cleared, `wir` is also cleared.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_cdtc(true);
    /// assert!(s.cdtc());
    /// assert!(s.wir());
    /// ```
    pub fn set_cdtc(&mut self, val: bool) {
        if val {
            self.0 = (self.0 | Self::CDTC_BIT | Self::WIR_BIT) & !Self::PDTC_BIT;
        } else {
            self.0 &= !(Self::CDTC_BIT | Self::WIR_BIT);
        }
    }

    /// Returns the `tftoc` flag (bit 1) — latched failure flag.
    ///
    /// Mirrors `tf` but latches permanently once set to `true`.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_tf(true);
    /// assert!(s.tftoc());
    /// ```
    pub fn tftoc(&self) -> bool {
        self.0 & Self::TFTOC_BIT != 0
    }

    /// Sets the `tftoc` flag (bit 1).
    ///
    /// Once `true`, clearing is a no-op by design to preserve failure history.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_tftoc(true);
    /// assert!(s.tftoc());
    /// ```
    pub fn set_tftoc(&mut self, val: bool) {
        if val {
            self.0 |= Self::TFTOC_BIT;
        }
        // never cleared
    }

    /// Returns the `tncslc` flag (bit 4) — not-complete since last clear.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// assert!(!s.tncslc());
    /// s.set_tncslc(true);
    /// assert!(s.tncslc());
    /// ```
    pub fn tncslc(&self) -> bool {
        self.0 & Self::TNCSLC_BIT != 0
    }

    /// Sets the `tncslc` flag (bit 4).
    ///
    /// Also cleared automatically when `tnctoc` is cleared.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_tncslc(true);
    /// assert!(s.tncslc());
    /// ```
    pub fn set_tncslc(&mut self, val: bool) {
        if val {
            self.0 |= Self::TNCSLC_BIT;
        } else {
            self.0 &= !Self::TNCSLC_BIT;
        }
    }

    /// Returns the `tfslc` flag (bit 5) — failure since last clear.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// assert!(!s.tfslc());
    /// s.set_tf(true); // also sets tfslc
    /// assert!(s.tfslc());
    /// ```
    pub fn tfslc(&self) -> bool {
        self.0 & Self::TFSLC_BIT != 0
    }

    /// Sets the `tfslc` flag (bit 5).
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_tfslc(true);
    /// assert!(s.tfslc());
    /// s.set_tfslc(false);
    /// assert!(!s.tfslc());
    /// ```
    pub fn set_tfslc(&mut self, val: bool) {
        if val {
            self.0 |= Self::TFSLC_BIT;
        } else {
            self.0 &= !Self::TFSLC_BIT;
        }
    }

    /// Returns the `tnctoc` flag (bit 6) — not-complete this operating cycle.
    ///
    /// Starts `true` on initialization, indicating the event is not yet confirmed.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_tnctoc(true);
    /// assert!(s.tnctoc()); // always true on from_raw()
    /// ```
    pub fn tnctoc(&self) -> bool {
        self.0 & Self::TNCTOC_BIT != 0
    }

    /// Sets the `tnctoc` flag (bit 6).
    ///
    /// Clearing also clears `tncslc` (bit 4), as completion resets the "since last clear" state.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_tncslc(true);
    /// s.set_tnctoc(false);
    /// assert!(!s.tnctoc());
    /// assert!(!s.tncslc()); // dropped with tnctoc
    /// ```
    pub fn set_tnctoc(&mut self, val: bool) {
        if val {
            self.0 |= Self::TNCTOC_BIT;
        } else {
            self.0 &= !Self::TNCTOC_BIT & !Self::TNCSLC_BIT;
        }
    }

    /// Returns the `wir` flag (bit 7) — Warning Indicator Requested.
    ///
    /// Set when a confirmed DTC is active, requesting activation of the MIL/warning light.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// assert!(!s.wir());
    /// s.set_cdtc(true);
    /// assert!(s.wir());
    /// ```
    pub fn wir(&self) -> bool {
        self.0 & Self::WIR_BIT != 0
    }

    /// Sets the `wir` flag (bit 7).
    ///
    /// Note: WIR is also automatically set/cleared by `set_cdtc()`.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_wir(true);
    /// assert!(s.wir());
    /// s.set_wir(false);
    /// assert!(!s.wir());
    /// ```
    pub fn set_wir(&mut self, val: bool) {
        if val {
            self.0 |= Self::WIR_BIT;
        } else {
            self.0 &= !Self::WIR_BIT;
        }
    }

    /// Returns the raw underlying byte value.
    pub fn raw(&self) -> u8 {
        self.0
    }

    /// Clears all bits except `tnctoc` and `tncslc`, resetting to initial state.
    ///
    /// Specifically sets `tnctoc` and `tncslc` to `true`, and clears all others.
    /// This represents a "last clear" event, resetting history flags.
    ///
    /// # Example
    ///
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::from_raw(0);
    /// s.set_tf(true);
    /// assert!(!s.tnctoc());
    /// s.clear();
    /// assert!(s.tnctoc());
    /// assert!(s.tncslc());
    /// ```
    pub fn clear(&mut self) {
        self.0 = Self::TNCTOC_BIT | Self::TNCSLC_BIT;
    }
}

// ─────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── UdsStatusByte ──────────────────────────

    #[test]
    fn fn_new_initial_tf_false_tnctoc_true() {
        let mut s = UdsStatusByte::from_raw(255u8);
        s.init();
        assert!(!s.tf());
        assert!(!s.tftoc());
        assert!(s.tnctoc());
    }

    #[test]
    fn fn_set_tf_clears_tnctoc_permanently() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        s.set_tf(true);
        assert!(s.tf());
        assert!(!s.tnctoc());
        s.set_tf(false);
        assert!(!s.tnctoc());
    }

    #[test]
    fn fn_set_tf_sets_tftoc_and_pdtc_permanently() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        s.set_tf(true); // tftoc and tfslc latch
        assert!(s.tftoc());
        assert!(s.pdtc());
        s.set_tf(false); // tf clears, tftoc and tfslc stay
        assert!(!s.tf());
        assert!(s.tftoc());
        assert!(s.pdtc());
    }

    #[test]
    fn fn_set_tf_sets_tfslc_permanently() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        s.set_tf(true); // tftoc and tfslc latch
        assert!(s.tfslc());
        s.set_tf(false); // tf clears, tftoc and tfslc stay
        assert!(!s.tf());
        assert!(s.tfslc());
    }

    #[test]
    fn fn_clear() {
        let mut s = UdsStatusByte::from_raw(255u8);
        s.init();
        assert_ne!(s.raw(), UdsStatusByte::TNCTOC_BIT);
        s.clear();
        assert_eq!(s.raw(), 0b0101_0000);
    }

    #[test]
    fn fn_set_cdtc() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        assert!(!s.cdtc());
        s.set_cdtc(true);
        assert!(s.cdtc());
        s.set_cdtc(false);
        assert!(!s.cdtc());
    }

    #[test]
    fn fn_set_cdtc_resets_pdtc() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        s.set_pdtc(true);
        assert!(s.pdtc());
        s.set_cdtc(true);
        assert!(s.cdtc());
        assert!(!s.pdtc());
    }

    #[test]
    fn fn_clear_resets_cdtc() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        s.set_cdtc(true);
        assert!(s.cdtc());
        s.clear();
        assert!(!s.cdtc());
    }

    #[test]
    fn fn_set_pdtc() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        assert!(!s.pdtc());
        s.set_pdtc(true);
        assert!(s.pdtc());
        s.set_pdtc(false);
        assert!(!s.pdtc());
    }

    #[test]
    fn fn_set_tftoc() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        assert!(!s.tftoc());
        s.set_tftoc(true);
        assert!(s.tftoc());
        // Cannot clear once set
        s.set_tftoc(false);
        assert!(s.tftoc());
    }

    #[test]
    fn fn_set_tfslc() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        assert!(!s.tfslc());
        s.set_tfslc(true);
        assert!(s.tfslc());
        s.set_tfslc(false);
        assert!(!s.tfslc());
    }

    #[test]
    fn fn_set_tncslc() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        assert!(!s.tncslc());
        s.set_tncslc(true);
        assert!(s.tncslc());
        s.set_tncslc(false);
        assert!(!s.tncslc());
    }

    #[test]
    fn fn_set_tnctoc() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        // from_raw() sets tnctoc to true
        assert!(s.tnctoc());
        s.set_tnctoc(false);
        assert!(!s.tnctoc());
        // Setting false clears tncslc too
        s.set_tncslc(true);
        assert!(s.tncslc());
        s.set_tnctoc(false);
        assert!(!s.tncslc());
    }

    #[test]
    fn fn_from_raw_with_mask() {
        // Test that from_raw() respects mask for non-forced bits
        let s = UdsStatusByte::from_raw(0b0000_1000); // bit 3 (cdtc) set
        assert!(s.cdtc());
        assert!(!s.tf()); // forced false
        assert!(!s.tftoc()); // forced false
        assert!(!s.tnctoc()); // forced true
    }

    #[test]
    fn fn_set_cdtc_sets_wir() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        assert!(!s.wir());
        s.set_cdtc(true);
        assert!(s.cdtc());
        assert!(s.wir());
    }

    #[test]
    fn fn_set_cdtc_false_clears_wir() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        s.set_cdtc(true);
        assert!(s.wir());
        s.set_cdtc(false);
        assert!(!s.cdtc());
        assert!(!s.wir());
    }

    #[test]
    fn fn_set_wir() {
        let mut s = UdsStatusByte::from_raw(0);
        s.init();
        assert!(!s.wir());
        s.set_wir(true);
        assert!(s.wir());
        s.set_wir(false);
        assert!(!s.wir());
    }
}
