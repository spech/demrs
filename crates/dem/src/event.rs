// ─────────────────────────────────────────────
// Event Management
// ─────────────────────────────────────────────

#[allow(unused_imports)]
use crate::event_config::{CalibConfig, DebounceBehavior, DebounceType, SaveTrigger};
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
/// - **Configuration**: [`CalibConfig`](crate::event_config::CalibConfig) for calibration, [`NvmConfig`] for persistent state.
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

/// Error type for [`Event`] operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventError {
    /// Sampling period must be positive for time-based debouncing.
    InvalidSampling,
}

// ─────────────────────────────────────────────
// NvmConfig
// ─────────────────────────────────────────────

/// Non-volatile configuration for [`Event`].
///
/// Holds persistent state for the event, including status flags and counters.
/// This allows events to maintain history across power cycles or resets.
#[derive(Clone)]
pub struct NvmConfig {
    /// The underlying UDS status byte for this event.
    pub uds_status: UdsStatusByte,
    /// Occurrence counter: number of times `tf` transitioned from `false` to `true`.
    pub occurence_cntr: u8,
    /// Aging cycles represents the number of consecutive cycles where a confirmed event is not failed
    pub aging_cycles: u8,
    /// Confirmation cycles: number of consecutive cycles where the event is confirmed failed.
    pub confirmation_cycles: u8,
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
pub struct Event {
    /// Accumulated debounce counter, positive toward Failed, negative toward Passed.
    pub debounce_counter: i16,
    /// Previous UDS status byte for detecting rising edges.
    pub uds_status_old: UdsStatusByte,
    /// Whether debouncing is disabled.
    pub disabled: bool,
    /// Reference to non-volatile configuration.
    pub nv_config: &'static mut NvmConfig,
    /// Reference to calibration configuration.
    pub cal_config: &'static CalibConfig,
}

impl Event {
    /// Enables or disables the event debouncing.
    ///
    /// # Arguments
    ///
    /// * `turnoff` - If `true`, disables debouncing; if `false`, enables it.
    pub fn disable(&mut self, turnoff: bool) {
        self.disabled = turnoff;
    }

    /// Initializes the event at the start of a new operating cycle.
    ///
    /// Resets `tf` (test failed) to false and sets `tnctoc` (test not complete this operating cycle) to true.
    pub fn init(&mut self) {
        self.nv_config.uds_status.clear();
        self.uds_status_old.clear();
        self.reset_counter();
        self.disabled = false;
    }

    /// Stops the event, typically at shutdown.
    ///
    /// Updates cycle counters based on current status and disables the event.
    pub fn stop(&mut self) -> UdsStatusByte {
        self.disabled = true;
        if !self.nv_config.uds_status.tftoc() {
            if !self.nv_config.uds_status.tnctoc() {
                self.nv_config.uds_status.set_pdtc(false);
                if self.nv_config.uds_status.cdtc() {
                    if self.nv_config.aging_cycles == self.cal_config.aging_threshold {
                        self.nv_config.uds_status.set_cdtc(false);
                        self.nv_config.confirmation_cycles = 0u8;
                    } else {
                        self.nv_config.aging_cycles =
                            self.nv_config.aging_cycles.saturating_add(1u8);
                    }
                }
            }
        } else {
            if !self.nv_config.uds_status.cdtc() {
                if self.nv_config.confirmation_cycles < self.cal_config.confirmation_threshold {
                    self.nv_config.confirmation_cycles =
                        self.nv_config.confirmation_cycles.saturating_add(1u8);
                }
            }
        }
        self.nv_config.uds_status
    }

    /// Clears the event state: resets counters, flags, and status.
    ///
    /// Resets `debounce_counter` to zero, sets `tf` to `false`, `tnctoc` and `tncslc` to `true`,
    /// clears `tfslc`, and resets occurrence counters. Preserves `tftoc`.
    pub fn clear(&mut self) {
        self.init();
        self.nv_config.occurence_cntr = 0u8;
        self.nv_config.confirmation_cycles = 0u8;
        self.nv_config.aging_cycles = 0u8;
    }

    /// Advances the event by one step based on the input condition.
    ///
    /// The behavior depends on the current `Status` signal and the configured debounce mode.
    ///
    /// - If `active == false` or the event is `disabled`, the event is not processed.
    ///   - If `debounce_behavior == DebounceBehavior::Reset`, the counter is reset to zero.
    /// - `Status::PreFailed` and `Status::PrePassed` use incremental debounce logic.
    ///   - `step_up == 1` or `step_down == 1` trigger immediate confirmation via `snap_failed()` / `snap_passed()`.
    ///   - non-zero `step_up` / `step_down` accumulate toward `i16::MAX` / `i16::MIN`.
    ///   - `CounterBased` mode divides the full-scale `i16` range by the configured step value.
    ///   - `TimeBased` mode interprets `step_up` / `step_down` as confirmation time in seconds,
    ///     converts `sampling` from seconds to milliseconds, and evenly distributes the full-scale
    ///     counter across the expected number of samples.
    ///   - If the counter carries the opposite sign from the requested direction, it is reset
    ///     before beginning accumulation.
    /// - `Status::Failed` and `Status::Passed` immediately snap the event state.
    ///
    /// # Arguments
    ///
    /// * `condition` - The status signal driving the debouncing.
    /// * `active` - If `false`, skips debouncing and returns previous status.
    /// * `sampling` - Sampling period in seconds (used only for time-based debouncing).
    ///
    /// # Returns
    ///
    /// * `Err(EventError::InvalidSampling)` if `sampling` is non-positive for time-based debouncing.
    /// * `Ok(UdsStatusByte)` with the updated status after processing the step.
    pub fn step(
        &mut self,
        condition: Status,
        active: bool,
        sampling: f32,
    ) -> Result<UdsStatusByte, EventError> {
        if self.cal_config.debounce_type == DebounceType::TimeBased {
            if sampling <= 0.0 {
                return Err(EventError::InvalidSampling);
            }
        }

        self.uds_status_old = self.nv_config.uds_status;

        if !active || self.disabled {
            if self.cal_config.debounce_behavior == DebounceBehavior::Reset {
                self.reset_counter();
            }
        } else {
            match condition {
                Status::PreFailed => {
                    if self.cal_config.step_up == 1i16 {
                        self.snap_failed();
                    } else if self.cal_config.step_up != 0i16 {
                        if self.debounce_counter < 0i16 {
                            self.reset_counter();
                        }
                        let mut increment: i16 = self.cal_config.step_up;
                        if self.cal_config.debounce_type == DebounceType::TimeBased {
                            // TimeBased step_up is the duration in seconds to reach confirmed failed.
                            // Sampling is provided in seconds, so convert it to milliseconds.
                            // We then compute how many samples are required to span the configured
                            // confirmation duration, and distribute the full-scale i16::MAX counter
                            // evenly across that sample count.
                            let sampling_ms = seconds_to_millis(sampling);
                            let total_samples = div_round_i32(
                                i32::from(self.cal_config.step_up) * 1000,
                                sampling_ms,
                            )
                            .max(1);
                            increment = div_round_i32(i32::from(i16::MAX), total_samples) as i16;
                        } else {
                            increment = div_round(i16::MAX, increment);
                        }
                        self.debounce_counter = self.debounce_counter.saturating_add(increment);
                        if self.debounce_counter == i16::MAX {
                            self.snap_failed();
                        }
                    }
                }
                Status::PrePassed => {
                    if self.cal_config.step_down == 1i16 {
                        self.snap_passed();
                    } else if self.cal_config.step_down != 0i16 {
                        if self.debounce_counter > 0i16 {
                            self.reset_counter();
                        }
                        let mut decrement = self.cal_config.step_down;
                        if self.cal_config.debounce_type == DebounceType::TimeBased {
                            // TimeBased step_down is the duration in seconds to reach confirmed passed.
                            // Sampling is provided in seconds, so convert it to milliseconds.
                            // We then compute how many samples are needed for the configured time,
                            // and map that to a per-step decrement from the full-scale i16::MAX range.
                            let sampling_ms = seconds_to_millis(sampling);
                            let total_samples = div_round_i32(
                                i32::from(self.cal_config.step_down) * 1000,
                                sampling_ms,
                            )
                            .max(1);
                            decrement = div_round_i32(i32::from(i16::MAX), total_samples) as i16;
                        } else {
                            decrement = div_round(i16::MAX, decrement);
                        }
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

        Ok(self.nv_config.uds_status)
    }

    /// Current accumulated debounce_counter.
    pub fn debounce_counter(&self) -> i16 {
        self.debounce_counter
    }

    /// Returns the current status byte.
    pub fn status(&self) -> UdsStatusByte {
        self.nv_config.uds_status
    }

    /// Returns the priority of this event from the calibration config.
    pub fn priority(&self) -> u8 {
        self.cal_config.priority
    }

    fn reset_counter(&mut self) {
        self.debounce_counter = 0i16;
    }

    fn snap_failed(&mut self) {
        self.debounce_counter = i16::MAX;
        self.nv_config.uds_status.set_tf(true);
        if self.nv_config.confirmation_cycles == self.cal_config.confirmation_threshold {
            self.nv_config.uds_status.set_cdtc(true);
            self.nv_config.aging_cycles = 0u8;
        }
        if !self.uds_status_old.tf() {
            self.nv_config.occurence_cntr = self.nv_config.occurence_cntr.saturating_add(1u8);
        }
    }

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

fn seconds_to_millis(seconds: f32) -> i32 {
    (seconds * 1000.0).max(1.0).round() as i32
}

fn div_round_i32(a: i32, b: i32) -> i32 {
    (a + b / 2) / b
}

// ─────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const CALIB_COUNTER_FREEZE_1_0: CalibConfig = CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: DebounceBehavior::Freeze,
        debounce_type: DebounceType::CounterBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 0,
        save_trigger: SaveTrigger::OnCdtc,
    };

    const CALIB_COUNTER_FREEZE_1_0_P5: CalibConfig = CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: DebounceBehavior::Freeze,
        debounce_type: DebounceType::CounterBased,
        confirmation_threshold: 0,
        aging_threshold: 1,
        priority: 5,
        save_trigger: SaveTrigger::OnCdtc,
    };

    const CALIB_COUNTER_RESET_1_0: CalibConfig = CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: DebounceBehavior::Reset,
        debounce_type: DebounceType::CounterBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 0,
        save_trigger: SaveTrigger::OnCdtc,
    };

    const CALIB_TIME_FREEZE_1_0: CalibConfig = CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: DebounceBehavior::Freeze,
        debounce_type: DebounceType::TimeBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 0,
        save_trigger: SaveTrigger::OnCdtc,
    };

    const CALIB_TIME_FREEZE_0_1: CalibConfig = CalibConfig {
        step_up: 0,
        step_down: 1,
        debounce_behavior: DebounceBehavior::Freeze,
        debounce_type: DebounceType::TimeBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 0,
        save_trigger: SaveTrigger::OnCdtc,
    };

    const CALIB_TIME_FREEZE_0_0: CalibConfig = CalibConfig {
        step_up: 0,
        step_down: 0,
        debounce_behavior: DebounceBehavior::Freeze,
        debounce_type: DebounceType::TimeBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 0,
        save_trigger: SaveTrigger::OnCdtc,
    };

    const CALIB_TIME_FREEZE_2_0: CalibConfig = CalibConfig {
        step_up: 2,
        step_down: 0,
        debounce_behavior: DebounceBehavior::Freeze,
        debounce_type: DebounceType::TimeBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 0,
        save_trigger: SaveTrigger::OnCdtc,
    };

    const CALIB_TIME_FREEZE_0_2: CalibConfig = CalibConfig {
        step_up: 0,
        step_down: 2,
        debounce_behavior: DebounceBehavior::Freeze,
        debounce_type: DebounceType::TimeBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 0,
        save_trigger: SaveTrigger::OnCdtc,
    };

    fn create_cal_config(
        step_up: i16,
        step_down: i16,
        debounce_type: DebounceType,
        debounce_behavior: DebounceBehavior,
    ) -> &'static CalibConfig {
        match (step_up, step_down, debounce_type, debounce_behavior) {
            (1, 0, DebounceType::CounterBased, DebounceBehavior::Freeze) => {
                &CALIB_COUNTER_FREEZE_1_0
            }
            (1, 0, DebounceType::CounterBased, DebounceBehavior::Reset) => &CALIB_COUNTER_RESET_1_0,
            (1, 0, DebounceType::TimeBased, DebounceBehavior::Freeze) => &CALIB_TIME_FREEZE_1_0,
            (0, 1, DebounceType::TimeBased, DebounceBehavior::Freeze) => &CALIB_TIME_FREEZE_0_1,
            (0, 0, DebounceType::TimeBased, DebounceBehavior::Freeze) => &CALIB_TIME_FREEZE_0_0,
            (2, 0, DebounceType::TimeBased, DebounceBehavior::Freeze) => &CALIB_TIME_FREEZE_2_0,
            (0, 2, DebounceType::TimeBased, DebounceBehavior::Freeze) => &CALIB_TIME_FREEZE_0_2,
            _ => panic!(
                "unsupported calibration: step_up={}, step_down={}, type={:?}, behavior={:?}",
                step_up, step_down, debounce_type, debounce_behavior
            ),
        }
    }

    fn create_nvm_config() -> &'static mut NvmConfig {
        let mut uds = UdsStatusByte::new(0);
        uds.set_tnctoc(true);

        Box::leak(Box::new(NvmConfig {
            uds_status: uds,
            occurence_cntr: 0,
            aging_cycles: 0,
            confirmation_cycles: 0,
        }))
    }

    fn create_event(
        step_up: i16,
        step_down: i16,
        debounce_type: DebounceType,
        debounce_behavior: DebounceBehavior,
    ) -> Event {
        let cal = create_cal_config(step_up, step_down, debounce_type, debounce_behavior);
        let nvm = create_nvm_config();

        Event {
            debounce_counter: 0,
            uds_status_old: nvm.uds_status,
            disabled: false,
            nv_config: nvm,
            cal_config: cal,
        }
    }

    #[test]
    fn fn_disable_sets_disabled_field() {
        let mut event = create_event(1, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);

        assert!(!event.disabled);

        event.disable(true);
        assert!(event.disabled);

        event.disable(false);
        assert!(!event.disabled);
    }

    #[test]
    fn fn_priority_returns_calib_config_priority() {
        let event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::new(0),
            disabled: false,
            nv_config: create_nvm_config(),
            cal_config: &CALIB_COUNTER_FREEZE_1_0_P5,
        };

        assert_eq!(event.priority(), 5);
    }

    #[test]
    fn fn_clear_resets_all_state() {
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::new(0),
            disabled: false,
            nv_config: create_nvm_config(),
            cal_config: &CALIB_COUNTER_FREEZE_1_0_P5,
        };

        event.step(Status::Failed, true, 0.0).unwrap();
        event.nv_config.uds_status.set_pdtc(true);
        assert!(event.status().tf());
        assert!(event.status().tftoc());
        assert!(event.status().pdtc());
        assert!(event.status().cdtc());
        assert!(event.status().tfslc());
        assert!(!event.status().tnctoc());
        assert!(!event.status().tncslc());

        event.nv_config.occurence_cntr = 5;
        event.nv_config.confirmation_cycles = 3;
        event.nv_config.aging_cycles = 2;

        event.clear();

        assert_eq!(event.debounce_counter(), 0);
        assert!(!event.status().tf());
        assert!(!event.status().tftoc());
        assert!(!event.status().pdtc());
        assert!(!event.status().cdtc());
        assert!(!event.status().tfslc());
        assert!(event.status().tnctoc());
        assert!(event.status().tncslc());
        assert_eq!(event.nv_config.occurence_cntr, 0);
        assert_eq!(event.nv_config.confirmation_cycles, 0);
        assert_eq!(event.nv_config.aging_cycles, 0);
    }

    #[test]
    fn fn_step_resets_when_event_behavior_is_reset_disabled() {
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::new(0),
            disabled: false,
            nv_config: create_nvm_config(),
            cal_config: &CALIB_COUNTER_RESET_1_0,
        };

        event.debounce_counter = 5000;
        event.disable(true);

        event.step(Status::PreFailed, true, 0.0).unwrap();

        assert_eq!(event.debounce_counter(), 0);
    }
}
