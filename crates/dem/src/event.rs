// ─────────────────────────────────────────────
// Event Management
// ─────────────────────────────────────────────

#[allow(unused_imports)]
use crate::event_config::{AgingMode, CalibConfig, DebounceBehavior, DebounceType, SaveTrigger};
#[cfg(test)]
use crate::indicator::LampBehavior;
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
    /// Healing cycles represents the number of consecutive cycles where a confirmed event is not failed
    pub healing_cycles: u8,
    /// Aging cycles represents the number of consecutive cycles after healing completes.
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
    /// Calibration configuration.
    pub cal_config: CalibConfig,
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
        self.nv_config.uds_status.init();
        self.uds_status_old.init();
        self.reset_counter();
        self.disabled = false;
    }

    /// Stops the event, typically at shutdown.
    ///
    /// Updates cycle counters based on current status and disables the event.
    pub fn stop(&mut self) -> UdsStatusByte {
        self.disabled = true;
        self.handle_aging_cycles(AgingMode::OperCycle);
        self.handle_healing_cycles();
        self.handle_confirmation_cycles();
        // Clear PDTC
        if !self.nv_config.uds_status.tftoc() {
            if !self.nv_config.uds_status.tnctoc() {
                self.nv_config.uds_status.set_pdtc(false);
            }
        }
        self.nv_config.uds_status
    }

    /// Clears the event state: resets counters, flags, and status.
    ///
    /// Resets `debounce_counter` to zero, sets `tf` to `false`, `tnctoc` and `tncslc` to `true`,
    /// clears `tfslc`, and resets occurrence counters. Preserves `tftoc`.
    pub fn clear(&mut self) {
        self.nv_config.uds_status.clear();
        self.reset_counter();
        self.uds_status_old = self.nv_config.uds_status;
        self.nv_config.occurence_cntr = 0u8;
        self.nv_config.confirmation_cycles = 0u8;
        self.nv_config.healing_cycles = 0u8;
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
                            increment = div_ceil(i16::MAX, increment);
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
                            decrement = div_ceil(i16::MAX, decrement);
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

    /// Returns `true` if the specified bit transitioned from `0` to `1`.
    ///
    /// Compares the current status byte with the previous status byte.
    pub fn has_risen(&self, bit: u8) -> bool {
        (self.uds_status_old.raw() & bit) == 0u8 && (self.nv_config.uds_status.raw() & bit) == bit
    }

    /// Returns `true` if the specified bit transitioned from `1` to `0`.
    ///
    /// Compares the current status byte with the previous status byte.
    pub fn has_fallen(&self, bit: u8) -> bool {
        (self.uds_status_old.raw() & bit) == bit && (self.nv_config.uds_status.raw() & bit) == 0u8
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

    /// Immediately confirms the event as failed.
    ///
    /// Sets `debounce_counter` to `i16::MAX`, `tf` to `true`, and increments
    /// `occurrence_counter` on rising edge of `tf`. Resets `aging_cycles` and
    /// conditionally resets `healing_cycles`. Sets `cdtc` if `confirmation_threshold` is reached.
    fn snap_failed(&mut self) {
        self.debounce_counter = i16::MAX;
        self.nv_config.uds_status.set_tf(true);
        self.nv_config.aging_cycles = 0u8;
        self.nv_config.healing_cycles = 0u8;
        if self.nv_config.confirmation_cycles
            >= self.cal_config.confirmation_threshold.saturating_sub(1)
        {
            self.nv_config.uds_status.set_cdtc(true);
        }
        if self.has_risen(UdsStatusByte::TF_BIT) {
            self.nv_config.occurence_cntr = self.nv_config.occurence_cntr.saturating_add(1u8);
        }
    }

    /// Immediately confirms the event as passed.
    ///
    /// Sets `debounce_counter` to `i16::MIN`, `tf` to `false`, and clears
    /// `tnctoc` and `tncslc` flags.
    fn snap_passed(&mut self) {
        self.debounce_counter = i16::MIN;
        self.nv_config.uds_status.set_tf(false);
        self.nv_config.uds_status.set_tnctoc(false);
        self.nv_config.uds_status.set_tncslc(false);
    }

    /// Handles aging cycles at the end of an operating cycle.
    ///
    /// Increments `aging_cycles` when `cdtc` is set and the event is not failed
    /// this cycle (`!tftoc && !tnctoc`) and `wir` is not active. Clears `cdtc`
    /// when `aging_threshold` is reached.
    ///
    /// The `mode` parameter specifies the current cycle type. Aging only proceeds
    /// if `mode` matches `cal_config.aging_mode`.
    fn handle_aging_cycles(&mut self, mode: AgingMode) {
        if mode != self.cal_config.aging_mode {
            return;
        }
        if !self.nv_config.uds_status.wir()
            && self.nv_config.uds_status.cdtc()
            && !self.nv_config.uds_status.tftoc()
            && !self.nv_config.uds_status.tnctoc()
        {
            if self.nv_config.aging_cycles >= self.cal_config.aging_threshold.saturating_sub(1) {
                self.nv_config.uds_status.set_cdtc(false);
            } else {
                self.nv_config.aging_cycles = self.nv_config.aging_cycles.saturating_add(1u8);
            }
        }
    }

    /// Handles healing cycles at the end of an operating cycle.
    ///
    /// Increments `healing_cycles` when `wir` is active and the event is not
    /// failed this cycle (`!tftoc && !tnctoc`). Clears `wir` and resets
    /// `confirmation_cycles` when `healing_threshold` is reached.
    fn handle_healing_cycles(&mut self) {
        if self.nv_config.uds_status.wir()
            && !self.nv_config.uds_status.tftoc()
            && !self.nv_config.uds_status.tnctoc()
        {
            if self.nv_config.healing_cycles >= self.cal_config.healing_threshold.saturating_sub(1)
            {
                self.nv_config.uds_status.set_wir(false);
                self.nv_config.confirmation_cycles = 0u8;
            } else {
                self.nv_config.healing_cycles = self.nv_config.healing_cycles.saturating_add(1u8);
            }
        }
    }

    /// Handles aging cycles during a warm-up cycle.
    ///
    /// Calls [`handle_aging_cycles`] with [`AgingMode::WarmUpCycle`].
    pub fn handle_warmup_cycle(&mut self) {
        self.handle_aging_cycles(AgingMode::WarmUpCycle);
    }

    /// Handles confirmation cycles at the end of an operating cycle.
    ///
    /// Increments `confirmation_cycles` when `tftoc` is set and `cdtc` is not yet
    /// confirmed, up to `confirmation_threshold`. Resets `healing_cycles` and
    /// `aging_cycles` when incrementing.
    fn handle_confirmation_cycles(&mut self) {
        if self.nv_config.uds_status.tftoc() {
            if !self.nv_config.uds_status.cdtc() {
                if self.nv_config.confirmation_cycles
                    >= self.cal_config.confirmation_threshold.saturating_sub(1)
                {
                    // Already at or past last increment - don't add more
                } else {
                    self.nv_config.confirmation_cycles =
                        self.nv_config.confirmation_cycles.saturating_add(1u8);
                    self.nv_config.healing_cycles = 0u8;
                    self.nv_config.aging_cycles = 0u8;
                }
            }
        }
    }
}

/// Rounds the division of `a` by `b` with ceiling.
///
/// Used for calculating increments in counter-based debouncing.
fn div_ceil(a: i16, b: i16) -> i16 {
    ((a as i32 + b as i32 - 1) / b as i32) as i16
}

/// Converts seconds to milliseconds, with a minimum of 1ms.
fn seconds_to_millis(seconds: f32) -> i32 {
    (seconds * 1000.0).max(1.0).round() as i32
}

/// Rounds division of two i32 values to nearest.
fn div_round_i32(a: i32, b: i32) -> i32 {
    (a + b / 2) / b
}

// ─────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn create_cal_config(
        step_up: i16,
        step_down: i16,
        debounce_type: DebounceType,
        debounce_behavior: DebounceBehavior,
    ) -> CalibConfig {
        CalibConfig {
            step_up,
            step_down,
            debounce_type,
            debounce_behavior,
            confirmation_threshold: 1,
            healing_threshold: 1,
            aging_threshold: 4,
            aging_mode: AgingMode::OperCycle,
            priority: 0,
            save_trigger: SaveTrigger::OnCdtc,
            record_update: true,
            lamp_behaviors: [
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
            ],
        }
    }

    fn create_nvm_config() -> &'static mut NvmConfig {
        let uds = UdsStatusByte::from_raw(0b0100_0000);

        Box::leak(Box::new(NvmConfig {
            uds_status: uds,
            occurence_cntr: 0,
            healing_cycles: 0,
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
        let cal_config = CalibConfig {
            step_up: 1,
            step_down: 0,
            debounce_behavior: DebounceBehavior::Freeze,
            debounce_type: DebounceType::CounterBased,
            confirmation_threshold: 1,
            healing_threshold: 1,
            aging_threshold: 4,
            aging_mode: AgingMode::OperCycle,
            priority: 5,
            save_trigger: SaveTrigger::OnCdtc,
            record_update: true,
            lamp_behaviors: [
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
            ],
        };
        let event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: create_nvm_config(),
            cal_config,
        };

        assert_eq!(event.priority(), 5);
    }

    #[test]
    fn fn_clear_resets_all_state() {
        let cal_config = CalibConfig {
            step_up: 1,
            step_down: 0,
            debounce_behavior: DebounceBehavior::Freeze,
            debounce_type: DebounceType::CounterBased,
            confirmation_threshold: 0,
            healing_threshold: 1,
            aging_threshold: 4,
            aging_mode: AgingMode::OperCycle,
            priority: 0,
            save_trigger: SaveTrigger::OnCdtc,
            record_update: true,
            lamp_behaviors: [
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
            ],
        };
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: create_nvm_config(),
            cal_config,
        };
        event.init();
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
        event.nv_config.healing_cycles = 2;

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
        assert_eq!(event.nv_config.healing_cycles, 0);
    }

    #[test]
    fn fn_stop_with_pdtc_increment_confirmation_counter() {
        let cal_config = CalibConfig {
            step_up: 1,
            step_down: 0,
            debounce_behavior: DebounceBehavior::Freeze,
            debounce_type: DebounceType::CounterBased,
            confirmation_threshold: 2,
            healing_threshold: 1,
            aging_threshold: 4,
            aging_mode: AgingMode::OperCycle,
            priority: 0,
            save_trigger: SaveTrigger::OnCdtc,
            record_update: true,
            lamp_behaviors: [
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
            ],
        };
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: create_nvm_config(),
            cal_config,
        };
        event.init();
        event.step(Status::Failed, true, 0.0).unwrap();

        assert!(event.status().pdtc());
        assert!(!event.status().cdtc());

        event.stop();

        assert_eq!(event.nv_config.confirmation_cycles, 1);
    }

    #[test]
    fn fn_stop_with_cdtc_and_notftoc_increment_healing_cycles() {
        let mut event = create_event(1, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);
        event.nv_config.confirmation_cycles = 1;
        event.nv_config.uds_status.set_cdtc(true);
        event.init();
        event.step(Status::Passed, true, 0.0).unwrap();
        event.uds_status_old = event.nv_config.uds_status;

        assert!(event.status().cdtc());
        assert!(!event.status().tftoc());
        assert!(!event.status().tnctoc());
        assert!(!event.status().pdtc());

        event.stop();

        assert!(!event.status().pdtc());
        assert_eq!(event.nv_config.healing_cycles, 0);
    }

    #[test]
    fn fn_stop_with_cdtc_and_nottftoc_clear_wir_when_threshold_reached() {
        let mut event = create_event(1, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);
        event.nv_config.confirmation_cycles = 1;
        event.nv_config.healing_cycles = 1;
        event.nv_config.uds_status.set_cdtc(true);
        event.init();
        event.step(Status::Passed, true, 0.0).unwrap();

        assert!(event.status().cdtc());
        assert!(!event.status().tftoc());

        event.stop();

        assert!(event.status().cdtc());
        assert!(!event.status().wir());
        assert_eq!(event.nv_config.confirmation_cycles, 0);
    }

    #[test]
    fn fn_stop_with_warmup_mode_does_not_increment_aging_cycles() {
        let cal = CalibConfig {
            step_up: 1,
            step_down: 0,
            debounce_behavior: DebounceBehavior::Freeze,
            debounce_type: DebounceType::CounterBased,
            confirmation_threshold: 1,
            healing_threshold: 1,
            aging_threshold: 4,
            aging_mode: AgingMode::WarmUpCycle,
            priority: 0,
            save_trigger: SaveTrigger::OnCdtc,
            record_update: true,
            lamp_behaviors: [
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
            ],
        };
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: create_nvm_config(),
            cal_config: cal,
        };
        event.nv_config.uds_status.set_cdtc(true);
        event.init();
        event.step(Status::Passed, true, 0.0).unwrap();

        assert!(event.status().cdtc());
        assert!(!event.status().tftoc());

        event.stop();

        assert!(event.status().cdtc());
        assert_eq!(event.nv_config.aging_cycles, 0);
    }

    #[test]
    fn fn_handle_warmup_cycle_increments_aging_when_mode_matches() {
        let cal = CalibConfig {
            step_up: 0,
            step_down: 1,
            debounce_behavior: DebounceBehavior::Freeze,
            debounce_type: DebounceType::CounterBased,
            confirmation_threshold: 0,
            healing_threshold: 0,
            aging_threshold: 4,
            aging_mode: AgingMode::WarmUpCycle,
            priority: 0,
            save_trigger: SaveTrigger::OnCdtc,
            record_update: true,
            lamp_behaviors: [
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
                LampBehavior::Off,
            ],
        };
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: create_nvm_config(),
            cal_config: cal,
        };
        event.init();
        event.step(Status::Failed, true, 0.0).unwrap();
        event.stop();

        event.init();
        event.step(Status::Passed, true, 0.0).unwrap();
        event.stop();

        assert!(!event.nv_config.uds_status.wir());

        event.handle_warmup_cycle();

        assert!(event.status().cdtc());
        assert_eq!(event.nv_config.aging_cycles, 1);
    }

    #[test]
    fn fn_stop_with_cdtc_and_tftoc_do_not_update() {
        let mut event = create_event(1, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);
        event.nv_config.healing_cycles = 1;
        event.nv_config.uds_status.set_cdtc(true);
        event.init();
        event.step(Status::Failed, true, 0.0).unwrap();

        assert!(event.status().cdtc());
        assert!(event.status().tftoc());

        event.stop();

        assert_eq!(event.nv_config.confirmation_cycles, 0);
        assert_eq!(event.nv_config.healing_cycles, 0);
    }

    #[test]
    fn fn_stop_with_nottftoc_and_nottnctoc_do_not_update() {
        let mut event = create_event(1, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);
        event.init();
        event.stop();

        assert!(!event.status().pdtc());
    }

    #[test]
    fn fn_stop_with_nottftoc_and_nottnctoc_and_notcdtc_do_not_update() {
        let mut event = create_event(1, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);
        event.nv_config.healing_cycles = 5;
        event.init();
        event.step(Status::Passed, true, 0.0).unwrap();

        assert!(!event.status().cdtc());
        assert!(!event.status().tftoc());

        event.stop();

        assert_eq!(event.nv_config.healing_cycles, 5);
    }

    #[test]
    fn fn_step_resets_when_event_behavior_is_reset_disabled() {
        let cal = create_cal_config(1, 0, DebounceType::CounterBased, DebounceBehavior::Reset);
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: create_nvm_config(),
            cal_config: cal,
        };

        event.debounce_counter = 5000;
        event.disable(true);

        event.step(Status::PreFailed, true, 0.0).unwrap();

        assert_eq!(event.debounce_counter(), 0);
    }

    #[test]
    fn fn_step_immediateness_when_failed() {
        let mut event = create_event(1, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);

        event.step(Status::Failed, true, 0.0).unwrap();

        assert_eq!(event.debounce_counter(), i16::MAX);
        assert!(event.status().tf());
    }

    #[test]
    fn fn_step_immediateness_when_passed() {
        let mut event = create_event(1, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);

        event.step(Status::Passed, true, 0.0).unwrap();

        assert_eq!(event.debounce_counter(), i16::MIN);
        assert!(!event.status().tf());
    }

    #[test]
    fn fn_step_immediateness_when_step_up_is_1() {
        let mut event = create_event(1, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);

        event.step(Status::PreFailed, true, 0.0).unwrap();

        assert_eq!(event.debounce_counter(), i16::MAX);
        assert!(event.status().tf());
    }

    #[test]
    fn fn_step_immediateness_when_step_down_is_1() {
        let mut event = create_event(0, 1, DebounceType::CounterBased, DebounceBehavior::Freeze);

        event.step(Status::PrePassed, true, 0.0).unwrap();

        assert_eq!(event.debounce_counter(), i16::MIN);
        assert!(!event.status().tf());
    }

    #[test]
    fn fn_step_hold_when_step_up_is_0() {
        let mut event = create_event(0, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);

        event.step(Status::PreFailed, true, 0.0).unwrap();

        assert_eq!(event.debounce_counter(), 0);
        assert!(!event.status().tf());
    }

    #[test]
    fn fn_step_hold_when_step_down_is_0() {
        let mut event = create_event(0, 0, DebounceType::CounterBased, DebounceBehavior::Freeze);

        event.step(Status::PrePassed, true, 0.0).unwrap();

        assert_eq!(event.debounce_counter(), 0);
        assert!(!event.status().tf());
    }

    #[test]
    fn fn_step_conterbased_confirm_3_steps_when_step_up_is_3() {
        let cal = create_cal_config(3, 3, DebounceType::CounterBased, DebounceBehavior::Freeze);
        let nvm = create_nvm_config();
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: nvm,
            cal_config: cal,
        };

        for _ in 0..3 {
            event.step(Status::PreFailed, true, 0.0).unwrap();
        }

        assert_eq!(event.debounce_counter(), i16::MAX);
        assert!(event.status().tf());
    }

    #[test]
    fn fn_step_conterbased_heal_3_steps_when_step_down_is_3() {
        let cal = create_cal_config(3, 3, DebounceType::CounterBased, DebounceBehavior::Freeze);
        let nvm = create_nvm_config();
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: nvm,
            cal_config: cal,
        };

        for _ in 0..3 {
            event.step(Status::PrePassed, true, 0.0).unwrap();
        }

        assert_eq!(event.debounce_counter(), i16::MIN);
        assert!(!event.status().tf());
    }

    #[test]
    fn fn_step_timebased_error_with_sampling_is_0() {
        let mut event = create_event(1, 0, DebounceType::TimeBased, DebounceBehavior::Freeze);

        let result = event.step(Status::PreFailed, true, 0.0);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), EventError::InvalidSampling);
    }

    #[test]
    fn fn_step_prefailed_to_prepassed_starts_from_0() {
        let cal = create_cal_config(3, 3, DebounceType::CounterBased, DebounceBehavior::Freeze);
        let nvm = create_nvm_config();
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: nvm,
            cal_config: cal,
        };

        event.step(Status::PreFailed, true, 0.0).unwrap();
        assert!(event.debounce_counter() > 0);

        event.step(Status::PrePassed, true, 0.0).unwrap();
        assert_eq!(event.debounce_counter(), -10923);
    }

    #[test]
    fn fn_step_prepassed_to_prefailed_starts_from_0() {
        let cal = create_cal_config(3, 3, DebounceType::CounterBased, DebounceBehavior::Freeze);
        let nvm = create_nvm_config();
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: nvm,
            cal_config: cal,
        };

        event.step(Status::PrePassed, true, 0.0).unwrap();
        assert!(event.debounce_counter() < 0);

        event.step(Status::PreFailed, true, 0.0).unwrap();
        assert_eq!(event.debounce_counter(), 10923);
    }

    #[test]
    fn fn_step_timebased_at_10ms_confirm_in_3_steps_when_step_is_25() {
        let cal = create_cal_config(25, 0, DebounceType::TimeBased, DebounceBehavior::Freeze);
        let nvm = create_nvm_config();
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: nvm,
            cal_config: cal,
        };

        for _ in 0..3 {
            event.step(Status::PreFailed, true, 0.01).unwrap();
        }

        assert_eq!(event.debounce_counter(), 39);
        assert!(!event.status().tf());
    }

    #[test]
    fn fn_step_timebased_at_10ms_heal_in_3_steps_when_step_is_25() {
        let cal = create_cal_config(25, 25, DebounceType::TimeBased, DebounceBehavior::Freeze);
        let nvm = create_nvm_config();
        nvm.uds_status.set_tf(true);
        let mut event = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::from_raw(0),
            disabled: false,
            nv_config: nvm,
            cal_config: cal,
        };

        for _ in 0..3 {
            event.step(Status::PrePassed, true, 0.01).unwrap();
        }

        assert_eq!(event.debounce_counter(), -39);
        assert!(event.status().tf());
    }
}
