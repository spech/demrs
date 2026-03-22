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
    /// Create a new ErrorConfirmator.
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
    ) -> bool {
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
                        let increment = i16::MAX / *self.config.step_up;
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
                        let decrement = (i16::MIN / *self.config.step_down).abs();
                        self.debounce_counter = self.debounce_counter.saturating_sub(decrement);
                        if self.debounce_counter == i16::MIN {
                            self.snap_passed();
                        }
                    }
                }
                Status::Failed => self.snap_failed(),
                Status::Passed => self.snap_passed(),
            }
        }

        self.uds_status.tf()
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
