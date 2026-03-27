// ─────────────────────────────────────────────
// Event Management
// ─────────────────────────────────────────────

use crate::UdsStatusByte;

/// # Event Module
///
/// This module implements event debouncing and status management for diagnostic systems.
/// It provides structures and logic to handle event confirmation, latching, and lifecycle
/// tracking based on input signals and calibration parameters.
///
/// ## Key Concepts
///
/// - **Debouncing**: Events are confirmed only after a threshold of consistent signals.
/// - **Status Signals**: [`Status`] enums drive the debouncing process.
/// - **Configuration**: [`CalibConfig`] for calibration, [`NvmConfig`] for persistent state.
/// - **Event Struct**: Manages the debouncing state and updates [`UdsStatusByte`].
///
/// ## State Machine
///
/// Events transition through states based on [`Status`] inputs:
/// - **Not Complete**: Initial state, waiting for confirmation.
/// - **Pre-Failed/Passed**: Accumulating toward threshold.
/// - **Confirmed Failed/Passed**: Threshold reached, status latched.
///
/// See [`UdsStatusByte`] for the underlying status bitfield.

// ─────────────────────────────────────────────
// Status
// ─────────────────────────────────────────────

/// The status signal passed to [`Event`] to drive debouncing.
///
/// This enum represents the input conditions that influence event confirmation.
/// The debouncing logic interprets these signals to update counters and flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Signal indicating potential failure; counter moves toward `i16::MAX`.
    PreFailed,
    /// Signal indicating potential pass; counter moves toward `i16::MIN`.
    PrePassed,
    /// Immediate failure confirmation; sets counter to `i16::MAX` and latches.
    Failed,
    /// Immediate pass confirmation; sets counter to `i16::MIN` and latches.
    Passed,
}

// ─────────────────────────────────────────────
// DebounceBehavior
// ─────────────────────────────────────────────

/// Controls the behavior of the debounce counter when the event is disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebounceBehavior {
    /// Leave the `debounce_counter` unchanged while disabled.
    Freeze,
    /// Reset the `debounce_counter` to zero while disabled.
    Reset,
}

// ─────────────────────────────────────────────
// DebounceType
// ─────────────────────────────────────────────

/// Specifies how the debounce counter is incremented/decremented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebounceType {
    /// `step_up`/`step_down` represent the number of ticks needed to reach max/min.
    CounterBased,
    /// `step_up`/`step_down` represent the time needed to reach max/min (requires sampling period).
    TimeBased,
}

// ─────────────────────────────────────────────
// CalibConfig
// ─────────────────────────────────────────────

/// Calibration configuration for [`Event`].
///
/// This struct holds references to step counts, debounce behavior, and type,
/// allowing a single configuration to be shared across multiple events.
/// All fields are lifetime-bound to ensure the config outlives the event.
pub struct CalibConfig<'a> {
    /// Number of `PreFailed` ticks to reach `i16::MAX`.
    /// - `0`: Counter unchanged (no debouncing).
    /// - `1`: Immediate snap to confirmed.
    /// - `n > 1`: Gradual accumulation.
    pub step_up: &'a i16,
    /// Number of `PrePassed` ticks to reach `i16::MIN`.
    /// - `0`: Counter unchanged.
    /// - `1`: Immediate snap.
    /// - `n > 1`: Gradual accumulation.
    pub step_down: &'a i16,
    /// Behavior when the event is disabled.
    pub debounce_behavior: &'a DebounceBehavior,
    /// How the counter is changed (tick-based or time-based).
    pub debounce_type: &'a DebounceType,
    /// Number of cycles where failure must be confirmed before setting `cdtc`.
    pub confirmation_threshold: &'a u8,
    /// Number of aging cycles before clearing `cdtc`.
    pub aging_threshold: &'a u8,
}

// ─────────────────────────────────────────────
// NvmConfig
// ─────────────────────────────────────────────

/// Non-volatile configuration for [`Event`].
///
/// Holds persistent state for the event, including status flags and counters.
/// This allows events to maintain history across power cycles or resets.
pub struct NvmConfig<'a> {
    /// The underlying UDS status byte for this event.
    pub uds_status: &'a mut UdsStatusByte,
    /// Occurrence counter: number of times `tf` transitioned from `false` to `true`.
    pub occurence_cntr: &'a mut u8,
    /// Aging cycles represents the number of consecutive cycles where a confirmed event is not failed
    pub aging_cycles: &'a mut u8,
    /// confirmation cycles represents the number of consecutive cycles where the an event is failed
    pub confirmation_cycles: &'a mut u8,
}

// ─────────────────────────────────────────────
// Event
// ─────────────────────────────────────────────

/// An event with debouncing logic driven by [`Status`] signals.
///
/// The `Event` manages a debounce counter and updates a [`UdsStatusByte`] based on input conditions.
/// It supports configurable thresholds, behaviors, and types for flexible diagnostic event handling.
///
/// ## Debouncing Behavior
///
/// - `PreFailed`: Accumulates toward `i16::MAX` via `saturating_add(step_up)`.
/// - `PrePassed`: Accumulates toward `i16::MIN` via `saturating_sub(step_down)`.
/// - `Failed`: Immediately sets counter to `i16::MAX`; latches `tf`.
/// - `Passed`: Immediately sets counter to `i16::MIN`; latches `tf` to false.
/// - `tf` (bit 0) becomes `true` when counter == `i16::MAX`, `false` when == `i16::MIN`.
/// - `tnctoc` (bit 6) starts `true` and is cleared once a threshold is reached.
///
/// ## Usage
///
/// Create an `Event` with [`CalibConfig`] and [`NvmConfig`], then call [`step`] with status signals.
pub struct Event<'a, 'b> {
    debounce_counter: i16,
    uds_status_old: UdsStatusByte,
    disabled: bool,
    nv_config: &'b mut NvmConfig<'b>,
    cal_config: &'a CalibConfig<'a>,
}

impl<'a, 'b> Event<'a, 'b> {
    /// Creates a new `Event` with the given configurations.
    ///
    /// # Arguments
    ///
    /// * `nv_config` - Non-volatile config holding persistent state.
    /// * `cal_config` - Calibration config with step counts and behavior.
    ///
    /// # Returns
    ///
    /// A new `Event` instance, initialized with counter at 0 and status copied.
    pub fn new(nv_config: &'b mut NvmConfig<'b>, cal_config: &'a  CalibConfig<'a>) -> Self {
        Self {
            debounce_counter: 0i16,
            uds_status_old: *nv_config.uds_status,
            disabled: false,
            nv_config: nv_config,
            cal_config: cal_config,
        }
    }

    /// Enables or disables the event debouncing.
    ///
    /// # Arguments
    ///
    /// * `turnoff` - If `true`, disables debouncing; if `false`, enables it.
    pub fn disable(&mut self, turnoff: bool) {
        self.disabled = turnoff;
    }

    /// Stops the event, typically at shutdown.
    ///
    /// Updates cycle counters based on current status and disables the event.
    pub fn stop(&mut self) {
        if !self.nv_config.uds_status.tftoc() && !self.nv_config.uds_status.tnctoc() {
            self.nv_config.uds_status.set_pdtc(false);
            if self.nv_config.uds_status.cdtc() {
                *self.nv_config.aging_cycles = self.nv_config.aging_cycles.saturating_add(1u8);
                if *self.nv_config.aging_cycles == *self.cal_config.aging_threshold {
                    self.nv_config.uds_status.set_cdtc(false);
                    *self.nv_config.confirmation_cycles = 0u8;
                }
            }
        }
        if self.nv_config.uds_status.tftoc() {
            *self.nv_config.confirmation_cycles = self.nv_config.confirmation_cycles.saturating_add(1u8);
            if *self.nv_config.confirmation_cycles == *self.cal_config.confirmation_threshold {
                self.nv_config.uds_status.set_cdtc(true);
                *self.nv_config.aging_cycles = 0u8;
            }
        }
        self.disabled = true;
    }

    /// Clears the event state: resets counters, flags, and status.
    ///
    /// Resets `debounce_counter` to zero, sets `tf` to `false`, `tnctoc` and `tncslc` to `true`,
    /// clears `tfslc`, and resets occurrence counters. Preserves `tftoc`.
    pub fn clear(&mut self) {
        self.reset_counter();
        *self.nv_config.occurence_cntr = 0u8;
        *self.nv_config.confirmation_cycles = 0u8;
        *self.nv_config.aging_cycles = 0u8;
        self.nv_config.uds_status.clear();
        self.uds_status_old.clear();
    }

    /// Advances the event by one step based on the input condition.
    ///
    /// # Arguments
    ///
    /// * `condition` - The status signal driving the debouncing.
    /// * `active` - If `false`, skips debouncing and returns previous status.
    /// * `sampling` - Sampling period (used for time-based debouncing).
    ///
    /// # Returns
    ///
    /// The updated [`UdsStatusByte`] after processing the step.
    pub fn step(
        &mut self,
        condition: Status,
        active: bool,
        sampling: f32
    ) -> UdsStatusByte {

        //store old status event before computing new status
        self.uds_status_old = *self.nv_config.uds_status;

        // if step is not active or the event is desabled: do not debounce
        if !active || self.disabled {
            if *self.cal_config.debounce_behavior == DebounceBehavior::Reset {
                self.reset_counter();
            }
        } else {

            match condition {
                Status::PreFailed => {
                    if *self.cal_config.step_up == 1i16 {
                        self.snap_failed();
                    } else if *self.cal_config.step_up != 0i16 {
                        if self.debounce_counter < 0i16 {
                            self.reset_counter();
                        }
                        let mut increment: i16 = *self.cal_config.step_up;
                        if *self.cal_config.debounce_type == DebounceType::TimeBased {
                            increment = div_round(*self.cal_config.step_up, sampling as i16);
                        }
                        increment = div_round(i16::MAX, increment);
                        self.debounce_counter = self.debounce_counter.saturating_add(increment);
                        if self.debounce_counter == i16::MAX{
                            self.snap_failed();
                        }
                    } // else error is disabled
                }
                Status::PrePassed => {
                    if *self.cal_config.step_down == 1i16 {
                        self.snap_passed();
                    } else if *self.cal_config.step_down != 0i16 {
                        if self.debounce_counter > 0i16 {
                            self.reset_counter();
                        }
                        let mut decrement = *self.cal_config.step_down;
                        if *self.cal_config.debounce_type == DebounceType::TimeBased {
                            decrement = div_round(*self.cal_config.step_down, sampling as i16);
                        }
                        decrement = div_round(i16::MAX, decrement);
                        self.debounce_counter = self.debounce_counter.saturating_sub(decrement);
                        if self.debounce_counter <= -i16::MAX {
                            self.snap_passed();
                        }
                    }
                } // else event cannot heal
                Status::Failed => self.snap_failed(),
                Status::Passed => self.snap_passed(),
            }
        }

        *self.nv_config.uds_status
    }

    /// Current accumulated debounce_counter.
    pub fn debounce_counter(&self) -> i16 {
        self.debounce_counter
    }

    /// Returns the current status byte.
    pub fn status(&self) -> UdsStatusByte {
        *self.nv_config.uds_status
    }

    /// Resets the debounce counter to zero without changing status flags.
    fn reset_counter(&mut self) {
        self.debounce_counter = 0i16;
    }

    /// Snaps to the failed state: sets counter to `i16::MAX`, `tf` to `true`, and latches flags.
    ///
    /// Also sets `tftoc`, `tfslc`, clears `tnctoc` and `tncslc`, and updates occurrence counter.
    fn snap_failed(&mut self) {
        self.debounce_counter = i16::MAX;
        self.nv_config.uds_status.set_tf(true);
        if self.uds_status_old.raw() & UdsStatusByte::TF_BIT != UdsStatusByte::TF_BIT  {
            *self.nv_config.occurence_cntr = self.nv_config.occurence_cntr.saturating_add(1u8);
        }
    }

    /// Snaps to the passed state: sets counter to `i16::MIN`, `tf` to `false`, and clears completion flags.
    fn snap_passed(&mut self) {
        self.debounce_counter = i16::MIN;
        self.nv_config.uds_status.set_tf(false);
        self.nv_config.uds_status.set_tnctoc(false);
    }
}

/// Rounds the division of `a` by `b` with half-up rounding.
///
/// Used for calculating increments in time-based debouncing.
fn div_round(a: i16, b: i16) -> i16 {
    ((a as i32 + b as i32 / 2) / b as i32) as i16
}

// ─────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create lifetime-bound thresholds
    fn create_config(
        step_up: i16,
        step_down: i16,
        confirmation_thr: u8,
        aging_thr: u8,
    ) -> (CalibConfig<'static>, i16, i16, u8, u8) {
        let step_up_val = Box::leak(Box::new(step_up));
        let step_down_val = Box::leak(Box::new(step_down));
        let confirmation_val = Box::leak(Box::new(confirmation_thr));
        let aging_val = Box::leak(Box::new(aging_thr));

        let cfg = CalibConfig {
            step_up: step_up_val,
            step_down: step_down_val,
            debounce_behavior: &DebounceBehavior::Freeze,
            debounce_type: &DebounceType::CounterBased,
            confirmation_threshold: confirmation_val,
            aging_threshold: aging_val,
        };

        (cfg, step_up, step_down, confirmation_thr, aging_thr)
    }

    // Helper to create lifetime-bound config with TimeBased debounce type
    fn create_timebased_config(
        step_up: i16,
        step_down: i16,
        confirmation_thr: u8,
        aging_thr: u8,
    ) -> (CalibConfig<'static>, i16, i16, u8, u8) {
        let step_up_val = Box::leak(Box::new(step_up));
        let step_down_val = Box::leak(Box::new(step_down));
        let confirmation_val = Box::leak(Box::new(confirmation_thr));
        let aging_val = Box::leak(Box::new(aging_thr));

        let cfg = CalibConfig {
            step_up: step_up_val,
            step_down: step_down_val,
            debounce_behavior: &DebounceBehavior::Freeze,
            debounce_type: &DebounceType::TimeBased,
            confirmation_threshold: confirmation_val,
            aging_threshold: aging_val,
        };

        (cfg, step_up, step_down, confirmation_thr, aging_thr)
    }

    // ────────────────────────────────────────────
    // Lifetime Configuration Tests
    // ────────────────────────────────────────────

    #[test]
    fn config_thresholds_are_referenced() {
        let (cfg, _, _, expected_conf, expected_aging) = create_config(2, 0, 3, 5);
        assert_eq!(*cfg.confirmation_threshold, expected_conf);
        assert_eq!(*cfg.aging_threshold, expected_aging);
    }

    #[test]
    fn nvm_config_uses_references() {
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 5u8;
        let mut aging = 2u8;
        let mut confirm = 3u8;

        let nvm = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        assert_eq!(*nvm.occurence_cntr, 5u8);
        assert_eq!(*nvm.aging_cycles, 2u8);
        assert_eq!(*nvm.confirmation_cycles, 3u8);
    }

    #[test]
    fn event_accesses_threshold_through_reference() {
        let (c_cfg, _, _, _, _) = create_config(1, 0, 4, 6);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let evt = Event::new(&mut nv, &c_cfg);
        assert_eq!(*evt.cal_config.confirmation_threshold, 4u8);
        assert_eq!(*evt.cal_config.aging_threshold, 6u8);
    }

    // ────────────────────────────────────────────
    // Debouncing Logic Tests
    // ────────────────────────────────────────────

    #[test]
    fn prefailed_increments_counter() {
        let (c_cfg, _, _, _, _) = create_config(1, 0, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        evt.step(Status::PreFailed, true, 0.0);
        assert_eq!(evt.debounce_counter(), i16::MAX);
        assert!(evt.status().tf());
    }

    #[test]
    fn prepassed_decrements_counter() {
        let (c_cfg, _, _, _, _) = create_config(0, 1, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        evt.step(Status::Failed, true, 0.0);
        evt.step(Status::PrePassed, true, 0.0);
        assert_eq!(evt.debounce_counter(), i16::MIN);
        assert!(!evt.status().tf());
    }

    #[test]
    fn failed_status_immediate_confirmation() {
        let (c_cfg, _, _, _, _) = create_config(2, 0, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        evt.step(Status::Failed, true, 0.0);
        assert_eq!(evt.debounce_counter(), i16::MAX);
        assert!(evt.status().tf());
    }

    #[test]
    fn passed_status_immediate_clear() {
        let (c_cfg, _, _, _, _) = create_config(0, 0, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        evt.step(Status::Failed, true, 0.0);
        assert!(evt.status().tf());
        evt.step(Status::Passed, true, 0.0);
        assert_eq!(evt.debounce_counter(), i16::MIN);
        assert!(!evt.status().tf());
    }

    #[test]
    fn counter_saturates_at_max() {
        let (c_cfg, _, _, _, _) = create_config(1, 0, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        evt.step(Status::PreFailed, true, 0.0);
        assert_eq!(evt.debounce_counter(), i16::MAX);
        evt.step(Status::PreFailed, true, 0.0);
        assert_eq!(evt.debounce_counter(), i16::MAX);
    }

    #[test]
    fn counter_saturates_at_min() {
        let (c_cfg, _, _, _, _) = create_config(0, 1, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        evt.step(Status::Failed, true, 0.0);
        evt.step(Status::PrePassed, true, 0.0);
        assert_eq!(evt.debounce_counter(), i16::MIN);
        evt.step(Status::PrePassed, true, 0.0);
        assert_eq!(evt.debounce_counter(), i16::MIN);
    }

    #[test]
    fn disabled_event_freezes_counter() {
        let (c_cfg, _, _, _, _) = create_config(1, 0, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        evt.step(Status::PreFailed, true, 0.0);
        let counter_val = evt.debounce_counter();
        evt.disable(true);
        evt.step(Status::PrePassed, true, 0.0);
        assert_eq!(evt.debounce_counter(), counter_val);
    }

    #[test]
    fn clear_resets_debounce_state() {
        let (c_cfg, _, _, _, _) = create_config(1, 0, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        evt.step(Status::PreFailed, true, 0.0);
        assert!(evt.status().tf());
        evt.clear();
        assert_eq!(evt.debounce_counter(), 0);
        assert!(!evt.status().tf());
    }

    // ────────────────────────────────────────────
    // Threshold Boundary Tests
    // ────────────────────────────────────────────

    #[test]
    fn threshold_references_compile_and_dereference() {
        let (c_cfg, _, _, _, _) = create_config(1, 0, 3, 5);
        // This test verifies that thresholds are properly lifetime-bound references
        assert_eq!(*c_cfg.confirmation_threshold, 3u8);
        assert_eq!(*c_cfg.aging_threshold, 5u8);
    }

    // ────────────────────────────────────────────
    // TimeBased Debounce Type Tests
    // ────────────────────────────────────────────

    #[test]
    fn timebased_prefailed_with_sampling_period() {
        let (c_cfg, _, _, _, _) = create_timebased_config(100, 0, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        // With sampling_period=10ms, should reduce effective step_up
        evt.step(Status::PreFailed, true, 10.0);
        assert!(evt.debounce_counter() > 0);
        assert!(!evt.status().tf()); // Not yet confirmed
    }

    #[test]
    fn timebased_prefailed_accumulates_with_multiple_steps() {
        let (c_cfg, _, _, _, _) = create_timebased_config(100, 0, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        let counter_after_first = {
            evt.step(Status::PreFailed, true, 10.0);
            evt.debounce_counter()
        };
        
        evt.step(Status::PreFailed, true, 10.0);
        let counter_after_second = evt.debounce_counter();
        
        // Counter should increase with each step
        assert!(counter_after_second >= counter_after_first);
    }

    #[test]
    fn timebased_prepassed_with_sampling_period() {
        let (c_cfg, _, _, _, _) = create_timebased_config(0, 100, 3, 5);
        let mut uds = UdsStatusByte::new(0);
        let mut occ = 0u8;
        let mut aging = 0u8;
        let mut confirm = 0u8;

        let mut nv = NvmConfig {
            uds_status: &mut uds,
            occurence_cntr: &mut occ,
            aging_cycles: &mut aging,
            confirmation_cycles: &mut confirm,
        };

        let mut evt = Event::new(&mut nv, &c_cfg);
        evt.step(Status::Failed, true, 10.0);
        assert!(evt.status().tf());
        
        // With sampling_period=10ms, should reduce effective step_down
        evt.step(Status::PrePassed, true, 10.0);
        assert!(evt.debounce_counter() < 0);
        assert!(evt.status().tf()); // Still true, not enough decrement
    }

    #[test]
    fn timebased_sampling_period_affects_counter() {
        let (c_cfg, _, _, _, _) = create_timebased_config(1000, 0, 3, 5);
        let mut uds1 = UdsStatusByte::new(0);
        let mut occ1 = 0u8;
        let mut aging1 = 0u8;
        let mut confirm1 = 0u8;

        let mut nv1 = NvmConfig {
            uds_status: &mut uds1,
            occurence_cntr: &mut occ1,
            aging_cycles: &mut aging1,
            confirmation_cycles: &mut confirm1,
        };

        let mut evt1 = Event::new(&mut nv1, &c_cfg);
        evt1.step(Status::PreFailed, true, 10.0);
        let counter_10ms = evt1.debounce_counter();

        let mut uds2 = UdsStatusByte::new(0);
        let mut occ2 = 0u8;
        let mut aging2 = 0u8;
        let mut confirm2 = 0u8;

        let mut nv2 = NvmConfig {
            uds_status: &mut uds2,
            occurence_cntr: &mut occ2,
            aging_cycles: &mut aging2,
            confirmation_cycles: &mut confirm2,
        };

        let mut evt2 = Event::new(&mut nv2, &c_cfg);
        evt2.step(Status::PreFailed, true, 100.0);
        let counter_100ms = evt2.debounce_counter();

        // Different sampling periods should result in different counter values
        assert_ne!(counter_10ms, counter_100ms);
        // Both should be positive
        assert!(counter_10ms > 0);
        assert!(counter_100ms > 0);
    }

}
