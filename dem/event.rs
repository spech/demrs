// ─────────────────────────────────────────────
// Status
// ─────────────────────────────────────────────

use crate::UdsStatusByte;

/// The EventStatus signal passed to `Status` to [`Debouncer`].
///
/// - `PreFailed` — counter moves toward `i16::MAX` via `saturating_add(step_up)`.
/// - `PrePassed` — counter moves toward `i16::MAX` via `saturating_sub(step_down)`.
/// - `Failed`    — counter is immediately set to `i16::MIN`; tf holds.
/// - `Passed`    — counter is immediately set to `i16::MAX`; tf holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    PreFailed,
    PrePassed,
    Failed,
    Passed,
}

// ─────────────────────────────────────────────
// DebounceBehavior
// ─────────────────────────────────────────────

/// Controls what happens to `debounce_counter` when `enable` is `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebounceBehavior {
    /// Leave `debounce_counter` unchanged while disabled.
    Freeze,
    /// Reset `debounce_counter` to zero while disabled.
    Reset,
}

// ─────────────────────────────────────────────
// DebounceType
// ─────────────────────────────────────────────

/// Controls how `debounce_counter` is changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebounceType {
    /// CounterBased in `step_up`/`step-down` represents the number of ticks needed to reach `max` or `min`.
    CounterBased,
    /// TimeBased in `step_up`/`step-down` represents the time needed to reach `max` or `min`.
    TimeBased,
}

// ─────────────────────────────────────────────
// CalibConfig
// ─────────────────────────────────────────────

/// Calibration configuration for [`Debouncer`].
///
/// Holds the step counts and debounce behavior as lifetime references,
/// allowing a single config to be shared across multiple confirmators.
pub struct CalibConfig<'a> {
    /// Number of `PreFailed` ticks to reach `i16::MAX`.
    /// `0` → counter unchanged, `1` → immediate snap, `n` → gradual.
    pub step_up: &'a i16,
    /// Number of `PrePassed` ticks to reach `i16::MIN`.
    /// `0` → counter unchanged, `1` → immediate snap, `n` → gradual.
    pub step_down: &'a i16,
    /// What to do with `debounce_counter` when `enable` is `false`.
    pub debounce_behavior: &'a DebounceBehavior,
        /// how `debounce_counter` is changed.
    pub debounce_type: &'a DebounceType,
}

// ─────────────────────────────────────────────
// Debouncer
// ─────────────────────────────────────────────

/// A confirmator with two thresholds driven by a [`Status`] signal.
///
/// - `PreFailed` accumulates toward `i16::MAX` via `saturating_add(step_up)`.
/// - `PrePassed` accumulates toward `i16::MIN` via `saturating_sub(step_down)`.
/// - `Failed`    immediately sets `debounce_counter` to `i16::MAX`; tf holds.
/// - `Passed`    immediately sets `debounce_counter` to `i16::MIN`; tf holds.
/// - `tf` (bit 0 of `status`) becomes `true` when `debounce_counter == T::MAX`, `false` when `== T::MIN`.
/// - `tnctoc` (bit 4 of `status`) starts `true` and is permanently cleared once either threshold is reached.
pub struct Debouncer<'a, 'b> {
    debounce_counter: i16,
    uds_status_old: UdsStatusByte,
    uds_status: &'b mut UdsStatusByte,
    config: &'a CalibConfig<'a>,
}

impl<'a, 'b> Debouncer<'a, 'b> {
    /// Create a new Debouncer.
    ///
    /// # Arguments
    /// * `status`  - A mutable reference to the `UdsStatusByte` backing the confirmator's flags.
    /// * `config`  - Calibration config holding step counts and debounce behavior.
    pub fn new(status: &'b mut UdsStatusByte, config: &'a  CalibConfig<'a>) -> Self {
        Self {
            debounce_counter: 0i16,
            uds_status_old: *status,
            uds_status: status,
            config: config,
        }
    }

    /// Clear the confirmator: resets `debounce_counter` to zero, sets `tf` to `false`,
    /// `tnctoc` to `true`, `tncslc` to `true`, clears `tfslc`; preserves `tftoc`.
    pub fn clear(&mut self) {
        self.reset_counter();
        self.uds_status.clear();
        self.uds_status_old = *self.uds_status;
    }

    /// Advance the error confirmator.
    ///
    /// # Arguments
    /// * `condition` - The condition signal. See [`Status`].
    /// * `enable`    - When `false`, counting is skipped and previous `tf` is returned.
    ///
    /// # Returns
    /// - `true`  once `debounce_counter == i16::MAX (`PreFailed` accumulation).
    /// - `false` once `debounce_counter == i16::MION` (`PrePassed` accumulation).
    /// - Previous `tf` in all other cases (middle zone, `Failed`, `Passed`, or disabled).
    pub fn step(
        &mut self,
        condition: Status,
        enable: bool,
    ) -> UdsStatusByte {
        self.uds_status_old = *self.uds_status;
        if !enable {
            if *self.config.debounce_behavior == DebounceBehavior::Reset {
                self.reset_counter();
            }
        } else {

            match condition {
                Status::PreFailed => {
                    if *self.config.step_up == 1i16 {
                        self.snap_failed();
                    } else if *self.config.step_up != 0i16 {
                        if self.debounce_counter < 0i16 {
                            self.reset_counter();
                        }
                        let increment = div_round(i16::MAX, *self.config.step_up);
                        self.debounce_counter = self.debounce_counter.saturating_add(increment);
                        if self.debounce_counter == i16::MAX{
                            self.snap_failed();
                        }
                    }
                }
                Status::PrePassed => {
                    if *self.config.step_down == 1i16 {
                        self.snap_passed();
                    } else if *self.config.step_down != 0i16 {
                        if self.debounce_counter > 0i16 {
                            self.reset_counter();
                        }
                        let decrement = div_round(i16::MAX, *self.config.step_down);
                        self.debounce_counter = self.debounce_counter.saturating_sub(decrement);
                        if self.debounce_counter <= -i16::MAX {
                            self.snap_passed();
                        }
                    }
                }
                Status::Failed => self.snap_failed(),
                Status::Passed => self.snap_passed(),
            }
        }

        *self.uds_status
    }

    /// Current accumulated debounce_counter.
    pub fn debounce_counter(&self) -> i16 {
        self.debounce_counter
    }

    /// Current accumulated debounce_counter.
    pub fn status(&self) -> UdsStatusByte {
        *self.uds_status
    }

    /// Reset `debounce_counter` to zero without changing status flags.
    fn reset_counter(&mut self) {
        self.debounce_counter = 0i16;
    }

    /// Snap to the failed state: counter = i16::MAX, tf = true, tftoc = true, tfslc = true, tnctoc = false, tncslc = false.
    fn snap_failed(&mut self) {
        self.debounce_counter = i16::MAX;
        self.uds_status.set_tf(true);
        self.uds_status.set_tnctoc(false);
    }

    /// Snap to the passed state: counter = i16::MIN, tf = false, tnctoc = false, tncslc = false.
    fn snap_passed(&mut self) {
        self.debounce_counter = i16::MIN;
        self.uds_status.set_tf(false);
        self.uds_status.set_tnctoc(false);
    }
}

fn div_round(a: i16, b: i16) -> i16 {
    ((a as i32 + b as i32 / 2) / b as i32) as i16
}

// ─────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

       // ── Debouncer ─────────────────────

    #[test]
    fn error_confirmator_initial_output_respected() {
        let c_cfg = CalibConfig { step_up: &2, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        // step_up=Some(2) → increment=i16::MAX; one tick → counter=i16::MAX, not at MAX
        assert!(!c.step(Status::PreFailed, true).tf());
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,2));
    }

    #[test]
    fn error_confirmator_count_2_confirms_in_2_ticks() {
        // i16::MAX = 127, count=2 → increment = i16::MAX per tick
        // tick1: 0 + i16::MAX = i16::MAX  (not confirmed)
        // tick2: i16::MAX + i16::MAX = 126 (not confirmed, 126 < 127)
        // need a third tick to saturate at 127
        let c_cfg = CalibConfig { step_up: &2, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::PreFailed, true);
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,2));
        assert!(!c.status().tf());
        assert!(c.step(Status::PreFailed, true).tf());
        assert_eq!(c.debounce_counter(), i16::MAX);
    }

    #[test]
    fn error_confirmator_count_1_confirms_in_1_tick() {
        // count=1 → increment = 127/1 = 127 → confirms immediately
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::PreFailed, true);
        assert_eq!(c.debounce_counter(), i16::MAX);
        assert!(c.status().tf());
    }

    #[test]
    fn error_confirmator_count_0_leaves_counter_unchanged() {
        // First advance with count=2 to get counter to i16::MAX
        let c_cfg = CalibConfig { step_up: &2, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::PreFailed, true); // counter = i16::MAX
        // Now switch to count=0 → no change
        let c2_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased };
        let mut c2_status = UdsStatusByte::new(0);
        let mut c2: Debouncer<'_, '_> = Debouncer::new(&mut c2_status, &c2_cfg);
        c2.step(Status::PreFailed, true);
        assert_eq!(c2.debounce_counter(), 0); // unchanged from 0
    }

    #[test]
    fn error_confirmator_prepassed_count_2_heals_in_ticks() {
        // decrement = i16::MIN / 2 = i16::MIN, abs() = 64 per tick toward T::MIN (-128)
        // tick1: 0   - 64 = i16::MIN  (not confirmed)
        // tick2: i16::MIN - 64 = -128 == T::MIN → tf = false
        let c_cfg = CalibConfig { step_up: &0, step_down: &2, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        //c_status.set_tf(true);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::Failed,true);
        c.step(Status::PrePassed, true);
        assert_eq!(c.debounce_counter(), -div_round(i16::MAX,2));
        assert!(c.status().tf());
        c.step(Status::PrePassed, true);
        assert_eq!(c.debounce_counter(), i16::MIN);
        assert!(!c.status().tf());
    }

    #[test]
    fn error_confirmator_holds_true_in_middle() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &2, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        // reach T::MAX with count=1 (snap)
        c.step(Status::PreFailed, true);
        assert!(c.status().tf());
        assert_eq!(c.debounce_counter(), i16::MAX);
        // heal tick count=2 → counter positive so reset to 0 first, then - 64 = i16::MIN, middle zone
        c.step(Status::PrePassed, true);
        assert!(c.status().tf()); // holds true
        assert_eq!(c.debounce_counter(), -div_round(i16::MAX,2));
    }

    #[test]
    fn error_confirmator_holds_false_in_middle() {
        let c_cfg = CalibConfig { step_up: &2, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        c_status.set_tf(true);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        // use Passed to snap instantly to T::MIN → tf = false
        c.step(Status::Passed, true);
        assert!(!c.status().tf());
        assert_eq!(c.debounce_counter(), i16::MIN);
        // PreFailed count=2 → counter negative so reset to 0 first, then +i16::MAX → i16::MAX, middle zone
        c.step(Status::PreFailed, true);
        assert!(!c.status().tf()); // holds false
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,2));
    }

    #[test]
    fn error_confirmator_clear_resets_state() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::PreFailed, true);
        assert!(c.status().tf());
        assert!(c.status().tfslc()); // tfslc set when tf was set
        assert!(!c.status().tnctoc());
        c.clear();
        assert_eq!(c.debounce_counter(), 0);
        assert!(!c.status().tf());             // tf set to false
        assert!(!c.status().tfslc());   // tfslc cleared
        assert!(c.status().tnctoc());          // tnctoc set to true
        assert!(c.status().tncslc());   // tncslc set to true
    }

    #[test]
    fn tfslc_set_with_tf_cleared_on_clear() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        assert!(!c.status().tfslc()); // starts false
        c.step(Status::PreFailed, true); // tf → true, tfslc latches
        assert!(c.status().tfslc());
        c.step(Status::Passed, true); // tf → false, tfslc stays
        assert!(!c.status().tf());
        assert!(c.status().tfslc()); // still set
        c.clear(); // tfslc reset
        assert!(!c.status().tfslc());
    }

    #[test]
    fn tncslc_drops_with_tnctoc() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.clear(); // sets tnctoc and tncslc to true
        assert!(c.status().tncslc());
        assert!(c.status().tnctoc());
        // reaching threshold drops tnctoc → tncslc drops too
        c.step(Status::PreFailed, true);
        assert!(!c.status().tnctoc());
        assert!(!c_status.tncslc());
    }

    #[test]
    fn error_confirmator_not_complete_starts_true() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        assert!(c.status().tnctoc());
    }

    #[test]
    fn error_confirmator_not_complete_false_on_positive_threshold() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        assert!(c.status().tnctoc());
        c.step(Status::PreFailed, true);
        assert!(!c.status().tnctoc());
    }

    #[test]
    fn error_confirmator_not_complete_false_on_negative_threshold() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        c_status.set_tf(true);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::Passed, true);
        assert!(!c.status().tnctoc());
    }

    #[test]
    fn error_confirmator_not_complete_stays_false_after_negative_threshold() {
        let c_cfg = CalibConfig { step_up: &2, step_down: &1, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        c_status.set_tf(true);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::PrePassed, true); // snaps to T::MIN
        assert!(!c.status().tnctoc());
        // PreFailed count=2 → resets to 0 (was negative), then +i16::MAX → i16::MAX, middle zone
        c.step(Status::PreFailed, true);
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,2));
        assert!(!c.status().tnctoc()); // stays false
    }

    #[test]
    fn error_confirmator_not_complete_stays_false_after_threshold() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &2, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::PreFailed, true);
        assert!(!c.status().tnctoc());
        // count=2 → counter positive so reset to 0 first, then - 64 = i16::MIN, middle zone
        c.step(Status::PrePassed, true);
        assert_eq!(c.debounce_counter(), -div_round(i16::MAX,2));
        assert!(!c.status().tnctoc()); // stays false
    }

    #[test]
    fn error_confirmator_freeze_holds_counter_and_tf() {
        let c_cfg = CalibConfig { step_up: &3, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        // count=2 → increment=i16::MAX; two ticks → counter=126
        c.step(Status::PreFailed, true);
        c.step(Status::PreFailed, true);
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,3)*2);
        c.step(Status::PreFailed, false); // disabled: Freeze
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,3)*2); // unchanged
        assert!(!c.status().tf());
    }

    #[test]
    fn error_confirmator_reset_zeroes_counter_when_disabled() {
        let c_cfg = CalibConfig { step_up: &2, step_down: &0, debounce_behavior: &DebounceBehavior::Reset , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::PreFailed, true);
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,2));
        c.step(Status::PreFailed, false); // disabled: Reset
        assert_eq!(c.debounce_counter(), 0);
        assert!(!c.status().tf());
    }

    #[test]
    fn error_confirmator_disabled_returns_previous_tf() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &2, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::PreFailed, true); // tf = true
        assert!(c.step(Status::PrePassed, false).tf());
        assert_eq!(c.debounce_counter(), i16::MAX); // holds true
        assert!(c.step(Status::PrePassed, false).tf()); // holds true
    }

    #[test]
    fn error_confirmator_saturates_at_max() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::PreFailed, true);
        c.step(Status::PreFailed, true); // still saturated
        assert_eq!(c.debounce_counter(), i16::MAX);
    }

    #[test]
    fn error_confirmator_saturates_at_min() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        c_status.set_tf(true);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::Passed, true);
        c.step(Status::Passed, true); // still at T::MIN
        assert_eq!(c.debounce_counter(), i16::MIN);
    }

    #[test]
    fn error_confirmator_failed_sets_counter_to_max_and_tf_true() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::Failed, true);
        assert_eq!(c.debounce_counter(), i16::MAX);
        assert!(c.status().tf());
        assert!(!c.status().tnctoc());
    }

    #[test]
    fn error_confirmator_passed_sets_counter_to_min_and_tf_false() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        c_status.set_tf(true);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::Passed, true);
        assert_eq!(c.debounce_counter(), i16::MIN);
        assert!(!c.status().tf());
        assert!(!c.status().tnctoc());
    }

    #[test]
    fn error_confirmator_failed_after_prefailed_stays_true() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        c.step(Status::PreFailed, true);
        assert!(c.status().tf());
        c.step(Status::Failed, true);
        assert_eq!(c.debounce_counter(), i16::MAX);
        assert!(c.status().tf());
    }

    #[test]
    fn error_confirmator_passed_after_prepassed_stays_false() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut c_status = UdsStatusByte::new(0);
        c_status.set_tf(true);
        let mut c: Debouncer<'_, '_> = Debouncer::new(&mut c_status, &c_cfg);
        // use Passed to reach T::MIN immediately
        c.step(Status::Passed, true);
        assert!(!c.status().tf());
        // Passed again: counter stays at T::MIN, tf stays false
        c.step(Status::Passed, true);
        assert_eq!(c.debounce_counter(), i16::MIN);
        assert!(!c.status().tf());
    }

}
