// ─────────────────────────────────────────────
// UdsStatusByte
// ─────────────────────────────────────────────

/// Bitfield backing the `UdsStatusByte` status.
///
/// | Bit | Name     | Meaning                                          |
/// |-----|----------|--------------------------------------------------|
/// |  0  | `tf`     | Confirmed flag (`true` = confirmed/failed)       |
/// |  1  | `tftoc`  | Latched tf flag — set with tf, never cleared     |
/// |  4  | `tncslc` | Not-complete since last clear                    |
/// |  5  | `tfslc`  | tf since last clear                              |
/// |  6  | `tnctoc` | Not-complete flag (`true` = not yet complete)    |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdsStatusByte(u8);

impl UdsStatusByte {
    pub(crate) const TF_BIT:     u8 = 1 << 0; // bit 0
    pub(crate) const TFTOC_BIT:  u8 = 1 << 1; // bit 1
    pub(crate) const TNCSLC_BIT: u8 = 1 << 4; // bit 4
    pub(crate) const TFSLC_BIT:  u8 = 1 << 5; // bit 5
    pub(crate) const TNCTOC_BIT: u8 = 1 << 6; // bit 6

    /// Creates a `UdsStatusByte` from a raw bitmask.
    /// `tf` (bit 0) is always forced to `false`, `tnctoc` (bit 6) is always forced to `true`.
    /// All other bits are taken from `mask`.
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let s = UdsStatusByte::new(255u8);
    /// assert!(!s.tf());
    /// assert!(s.tnctoc());
    /// ```
    pub fn new(mask: u8) -> Self {
        UdsStatusByte((mask & !(Self::TF_BIT | Self::TFTOC_BIT)) | Self::TNCTOC_BIT)
    }

    /// Returns `tf` flag (bit 0).
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::new(1u8);
    /// assert!(!s.tf());
    /// s.set_tf(true);
    /// assert!(s.tf());
    /// ```
    pub fn tf(&self) -> bool {
        self.0 & Self::TF_BIT != 0
    }

    /// Sets `tf`. If `val` is `true`, also sets `tftoc` and sets `tfslc`.
    /// Clears `tnctoc`
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::new(0);
    /// s.set_tf(true);
    /// assert!(s.tf());
    /// assert!(s.tftoc()); // latched
    /// assert!(s.tfslc()); // set
    /// assert!(!s.tnctoc());
    /// ```
    pub fn set_tf(&mut self, val: bool) {
        if val {
            self.0 = (self.0 | (Self::TF_BIT | Self::TFTOC_BIT | Self::TFSLC_BIT)) // sets
                & (!Self::TNCTOC_BIT) ; // clears

        } else {
            self.0 &= !Self::TF_BIT;
            // tftoc and tfslc are not cleared here
            // tnctoc remains cleared
        }
    }

    /// `tftoc` mirrors `tf` but latches — once `true`, never cleared.
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::new(0);
    /// s.set_tf(true);
    /// assert!(s.tftoc());
    /// ```
    pub fn tftoc(&self) -> bool {
        self.0 & Self::TFTOC_BIT != 0
    }

    /// Sets `tftoc`. Once `true`, clearing is a no-op by design.
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::new(0);
    /// s.set_tftoc(true);
    /// assert!(s.tftoc());
    /// ```
    pub fn set_tftoc(&mut self, val: bool) {
        if val { self.0 |= Self::TFTOC_BIT; }
        // never cleared
    }

    /// Returns `tncslc` flag (bit 4) — not-complete since last clear.
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::new(0);
    /// assert!(!s.tncslc());
    /// s.set_tncslc(true);
    /// assert!(s.tncslc());
    /// ```
    pub fn tncslc(&self) -> bool {
        self.0 & Self::TNCSLC_BIT != 0
    }

    /// Sets `tncslc` (bit 4). Also cleared automatically when `tnctoc` is cleared.
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::new(0);
    /// s.set_tncslc(true);
    /// assert!(s.tncslc());
    /// ```
    pub fn set_tncslc(&mut self, val: bool) {
        if val { self.0 |= Self::TNCSLC_BIT; }
        else   { self.0 &= !Self::TNCSLC_BIT; }
    }

    /// Returns `tfslc` flag (bit 5) — tf since last clear.
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::new(0);
    /// assert!(!s.tfslc());
    /// s.set_tf(true); // also sets tfslc
    /// assert!(s.tfslc());
    /// ```
    pub fn tfslc(&self) -> bool {
        self.0 & Self::TFSLC_BIT != 0
    }

    /// Sets `tfslc` (bit 5).
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::new(0);
    /// s.set_tfslc(true);
    /// assert!(s.tfslc());
    /// s.set_tfslc(false);
    /// assert!(!s.tfslc());
    /// ```
    pub fn set_tfslc(&mut self, val: bool) {
        if val { self.0 |= Self::TFSLC_BIT; }
        else   { self.0 &= !Self::TFSLC_BIT; }
    }

    /// Returns `tnctoc` flag (bit 6) — not-complete flag, starts `true`.
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let s = UdsStatusByte::new(0);
    /// assert!(s.tnctoc()); // always true on new()
    /// ```
    pub fn tnctoc(&self) -> bool {
        self.0 & Self::TNCTOC_BIT != 0
    }

    /// Sets `tnctoc` (bit 6). Clearing also clears `tncslc` (bit 4).
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::new(0);
    /// s.set_tncslc(true);
    /// s.set_tnctoc(false);
    /// assert!(!s.tnctoc());
    /// assert!(!s.tncslc()); // dropped with tnctoc
    /// ```
    pub fn set_tnctoc(&mut self, val: bool) {
        if val { self.0 |= Self::TNCTOC_BIT; }
        else   {
            self.0 &= !Self::TNCTOC_BIT & !Self::TNCSLC_BIT;
        }
    }

    /// Raw underlying byte.
    pub fn raw(&self) -> u8 {
        self.0
    }

    /// Clears all bits and sets `tnctoc`
    ///
    /// # Example
    /// ```
    /// # use dem::UdsStatusByte;
    /// let mut s = UdsStatusByte::new(0);
    /// s.set_tf(true);
    /// assert!(!s.tnctoc());
    /// s.clear();
    /// assert!(s.tnctoc());
    /// ```
    pub fn clear(&mut self) {
        self.0 = 0b0101_0000;
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
    fn status_flags_initial_tf_false_tnctoc_true() {
        let s = UdsStatusByte::new(255u8);
        assert!(!s.tf());
        assert!(!s.tftoc());
        assert!(s.tnctoc());
    }

    #[test]
    fn status_flags_set_tf_and_clear_tnctoc_permanently() {
        let mut s = UdsStatusByte::new(0);
        s.set_tf(true);
        assert!(s.tf());
        assert!(!s.tnctoc());
        s.set_tf(false); 
        assert!(!s.tnctoc());
    }

    #[test]
    fn status_flags_set_tf_and_tftoc_latches_permanently() {
        let mut s = UdsStatusByte::new(0);
        s.set_tf(true);  // tftoc and tfslc latch
        assert!(s.tftoc());
        s.set_tf(false); // tf clears, tftoc and tfslc stay
        assert!(!s.tf());
        assert!(s.tftoc());
    }

    #[test]
    fn status_flags_set_tf_and_tfslc_latches_permanently() {
        let mut s = UdsStatusByte::new(0);
        s.set_tf(true);  // tftoc and tfslc latch
        assert!(s.tfslc());
        s.set_tf(false); // tf clears, tftoc and tfslc stay
        assert!(!s.tf());
        assert!(s.tfslc());
    }

    #[test]
    fn status_flags_clear() {
        let mut s = UdsStatusByte::new(255u8);
        assert_ne!(s.raw(), UdsStatusByte::TNCTOC_BIT);
        s.clear();
        assert_eq!(s.raw(), 0b0101_0000);
    }
}