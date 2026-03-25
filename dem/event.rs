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
    /// Cycles since the last failure.
    pub cycles_since_last_failed: &'a mut u8,
    /// Cycles since the first failure in a series.
    pub cycles_since_first_failed: &'a mut u8,
    /// Total cycles where failure was confirmed.
    pub cycles_failed: &'a mut u8,
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
        }
        if self.nv_config.uds_status.tfslc() && !self.nv_config.uds_status.tftoc() {
            *self.nv_config.cycles_since_last_failed += 1u8;
        }
        if self.nv_config.uds_status.tfslc() {
            *self.nv_config.cycles_since_first_failed += 1u8;
        }
        if self.nv_config.uds_status.tftoc() {
            *self.nv_config.cycles_failed += 1u8;
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
        *self.nv_config.cycles_since_first_failed = 0u8;
        *self.nv_config.cycles_failed = 0u8;
        self.nv_config.uds_status.clear();
        self.uds_status_old = *self.nv_config.uds_status;
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
        *self.nv_config.cycles_since_last_failed = 0u8;
        if self.uds_status_old.raw() & UdsStatusByte::TF_BIT != UdsStatusByte::TF_BIT  {
            let x = *self.nv_config.occurence_cntr;
            *self.nv_config.occurence_cntr = x.saturating_add(1u8);
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

       // ── Debouncer ─────────────────────

    #[test]
    fn event_initial_output_respected() {
        let c_cfg = CalibConfig { step_up: &2, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // step_up=Some(2) → increment=i16::MAX; one tick → counter=i16::MAX, not at MAX
        assert!(!c.step(Status::PreFailed, true, 0.0).tf());
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,2));
    }

    #[test]
    fn event_count_2_confirms_in_2_ticks() {
        // i16::MAX = 127, count=2 → increment = i16::MAX per tick
        // tick1: 0 + i16::MAX = i16::MAX  (not confirmed)
        // tick2: i16::MAX + i16::MAX = 126 (not confirmed, 126 < 127)
        // need a third tick to saturate at 127
        let c_cfg = CalibConfig { step_up: &2, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::PreFailed, true, 0.0);
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,2));
        assert!(!c.status().tf());
        assert!(c.step(Status::PreFailed, true, 0.0).tf());
        assert_eq!(c.debounce_counter(), i16::MAX);
    }

    #[test]
    fn event_count_1_confirms_in_1_tick() {
        // count=1 → increment = 127/1 = 127 → confirms immediately
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::PreFailed, true, 0.0);
        assert_eq!(c.debounce_counter(), i16::MAX);
        assert!(c.status().tf());
    }

    #[test]
    fn event_count_0_leaves_counter_unchanged() {
        // First advance with count=2 to get counter to i16::MAX
        let c_cfg = CalibConfig { step_up: &2, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::PreFailed, true, 0.0); // counter = i16::MAX
        // Now switch to count=0 → no change
        let c2_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased };
        let mut nv2_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c2: Event<'_, '_> = Event::new(&mut nv2_config, &c2_cfg);
        c2.step(Status::PreFailed, true, 0.0);
        assert_eq!(c2.debounce_counter(), 0); // unchanged from 0
    }

    #[test]
    fn event_prepassed_count_2_heals_in_ticks() {
        // decrement = i16::MIN / 2 = i16::MIN, abs() = 64 per tick toward T::MIN (-128)
        // tick1: 0   - 64 = i16::MIN  (not confirmed)
        // tick2: i16::MIN - 64 = -128 == T::MIN → tf = false
        let c_cfg = CalibConfig { step_up: &0, step_down: &2, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        //c_status.set_tf(true);
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::Failed,true, 0.0);
        c.step(Status::PrePassed, true, 0.0);
        assert_eq!(c.debounce_counter(), -div_round(i16::MAX,2));
        assert!(c.status().tf());
        c.step(Status::PrePassed, true, 0.0);
        assert_eq!(c.debounce_counter(), i16::MIN);
        assert!(!c.status().tf());
    }

    #[test]
    fn event_holds_true_in_middle() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &2, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // reach T::MAX with count=1 (snap)
        c.step(Status::PreFailed, true, 0.0);
        assert!(c.status().tf());
        assert_eq!(c.debounce_counter(), i16::MAX);
        // heal tick count=2 → counter positive so reset to 0 first, then - 64 = i16::MIN, middle zone
        c.step(Status::PrePassed, true, 0.0);
        assert!(c.status().tf()); // holds true
        assert_eq!(c.debounce_counter(), -div_round(i16::MAX,2));
    }

    #[test]
    fn event_holds_false_in_middle() {
        let c_cfg = CalibConfig { step_up: &2, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        nv_config.uds_status.set_tf(true);
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // use Passed to snap instantly to T::MIN → tf = false
        c.step(Status::Passed, true, 0.0);
        assert!(!c.status().tf());
        assert_eq!(c.debounce_counter(), i16::MIN);
        // PreFailed count=2 → counter negative so reset to 0 first, then +i16::MAX → i16::MAX, middle zone
        c.step(Status::PreFailed, true, 0.0);
        assert!(!c.status().tf()); // holds false
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,2));
    }

    #[test]
    fn event_clear_resets_state() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::PreFailed, true, 0.0);
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
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        assert!(!c.status().tfslc()); // starts false
        c.step(Status::PreFailed, true, 0.0); // tf → true, tfslc latches
        assert!(c.status().tfslc());
        c.step(Status::Passed, true, 0.0); // tf → false, tfslc stays
        assert!(!c.status().tf());
        assert!(c.status().tfslc()); // still set
        c.clear(); // tfslc reset
        assert!(!c.status().tfslc());
    }

    #[test]
    fn tncslc_drops_with_tnctoc() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.clear(); // sets tnctoc and tncslc to true
        assert!(c.status().tncslc());
        assert!(c.status().tnctoc());
        // reaching threshold drops tnctoc → tncslc drops too
        c.step(Status::PreFailed, true, 0.0);
        assert!(!c.status().tnctoc());
        assert!(!c.status().tncslc());
    }

    #[test]
    fn event_not_complete_starts_true() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        assert!(c.status().tnctoc());
    }

    #[test]
    fn event_not_complete_false_on_positive_threshold() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        assert!(c.status().tnctoc());
        c.step(Status::PreFailed, true, 0.0);
        assert!(!c.status().tnctoc());
    }

    #[test]
    fn event_not_complete_false_on_negative_threshold() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        nv_config.uds_status.set_tf(true);
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::Passed, true, 0.0);
        assert!(!c.status().tnctoc());
    }

    #[test]
    fn event_not_complete_stays_false_after_negative_threshold() {
        let c_cfg = CalibConfig { step_up: &2, step_down: &1, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        nv_config.uds_status.set_tf(true);
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::PrePassed, true, 0.0); // snaps to T::MIN
        assert!(!c.status().tnctoc());
        // PreFailed count=2 → resets to 0 (was negative), then +i16::MAX → i16::MAX, middle zone
        c.step(Status::PreFailed, true, 0.0);
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,2));
        assert!(!c.status().tnctoc()); // stays false
    }

    #[test]
    fn event_not_complete_stays_false_after_threshold() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &2, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::PreFailed, true, 0.0);
        assert!(!c.status().tnctoc());
        // count=2 → counter positive so reset to 0 first, then - 64 = i16::MIN, middle zone
        c.step(Status::PrePassed, true, 0.0);
        assert_eq!(c.debounce_counter(), -div_round(i16::MAX,2));
        assert!(!c.status().tnctoc()); // stays false
    }

    #[test]
    fn event_freeze_holds_counter_and_tf() {
        let c_cfg = CalibConfig { step_up: &3, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // count=2 → increment=i16::MAX; two ticks → counter=126
        c.step(Status::PreFailed, true, 0.0);
        c.step(Status::PreFailed, true, 0.0);
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,3)*2);
        c.step(Status::PreFailed, false, 0.0); // disabled: Freeze
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,3)*2); // unchanged
        assert!(!c.status().tf());
    }

    #[test]
    fn event_reset_zeroes_counter_when_disabled() {
        let c_cfg = CalibConfig { step_up: &2, step_down: &0, debounce_behavior: &DebounceBehavior::Reset , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::PreFailed, true, 0.0);
        assert_eq!(c.debounce_counter(), div_round(i16::MAX,2));
        c.step(Status::PreFailed, false, 0.0); // disabled: Reset
        assert_eq!(c.debounce_counter(), 0);
        assert!(!c.status().tf());
    }

    #[test]
    fn event_disabled_returns_previous_tf() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &2, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::PreFailed, true, 0.0); // tf = true
        assert!(c.step(Status::PrePassed, false, 0.0).tf());
        assert_eq!(c.debounce_counter(), i16::MAX); // holds true
        assert!(c.step(Status::PrePassed, false, 0.0).tf()); // holds true
    }

    #[test]
    fn event_saturates_at_max() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::PreFailed, true, 0.0);
        c.step(Status::PreFailed, true, 0.0); // still saturated
        assert_eq!(c.debounce_counter(), i16::MAX);
    }

    #[test]
    fn event_saturates_at_min() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        nv_config.uds_status.set_tf(true);
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::Passed, true, 0.0);
        c.step(Status::Passed, true, 0.0); // still at T::MIN
        assert_eq!(c.debounce_counter(), i16::MIN);
    }

    #[test]
    fn event_failed_sets_counter_to_max_and_tf_true() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::Failed, true, 0.0);
        assert_eq!(c.debounce_counter(), i16::MAX);
        assert!(c.status().tf());
        assert!(!c.status().tnctoc());
    }

    #[test]
    fn event_passed_sets_counter_to_min_and_tf_false() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        nv_config.uds_status.set_tf(true);
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::Passed, true, 0.0);
        assert_eq!(c.debounce_counter(), i16::MIN);
        assert!(!c.status().tf());
        assert!(!c.status().tnctoc());
    }

    #[test]
    fn event_failed_after_prefailed_stays_true() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze , debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        c.step(Status::PreFailed, true, 0.0);
        assert!(c.status().tf());
        c.step(Status::Failed, true, 0.0);
        assert_eq!(c.debounce_counter(), i16::MAX);
        assert!(c.status().tf());
    }

    #[test]
    fn event_passed_after_prepassed_stays_false() {
        let c_cfg = CalibConfig { step_up: &0, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        nv_config.uds_status.set_tf(true);
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // use Passed to reach T::MIN immediately
        c.step(Status::Passed, true, 0.0);
        assert!(!c.status().tf());
        // Passed again: counter stays at T::MIN, tf stays false
        c.step(Status::Passed, true, 0.0);
        assert_eq!(c.debounce_counter(), i16::MIN);
        assert!(!c.status().tf());
    }

    #[test]
    fn cycles_since_last_failed_resets_on_failure() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 5u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // Initially 5
        assert_eq!(*c.nv_config.cycles_since_last_failed, 5u8);
        // Fail: should reset to 0
        c.step(Status::PreFailed, true, 0.0);
        assert_eq!(*c.nv_config.cycles_since_last_failed, 0u8);
    }

    #[test]
    fn cycles_since_last_failed_increments_on_stop_when_tfslc_but_not_tftoc() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // Fail to set tfslc
        c.step(Status::PreFailed, true, 0.0);
        assert!(c.status().tfslc());
        assert!(c.status().tftoc());
        // Stop: since tftoc is true, cycles_since_last_failed should not increment
        c.stop();
        assert_eq!(*c.nv_config.cycles_since_last_failed, 0u8);
    }

    #[test]
    fn cycles_since_last_failed_increments_on_stop_when_tfslc_true_and_tftoc_false() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // Manually set tfslc without setting tftoc
        c.nv_config.uds_status.set_tfslc(true);
        assert!(c.status().tfslc());
        assert!(!c.status().tftoc()); // tftoc not set
        // Stop: tfslc true, tftoc false, so increment
        c.stop();
        assert_eq!(*c.nv_config.cycles_since_last_failed, 1u8);
    }

    #[test]
    fn cycles_since_last_failed_not_incremented_when_tftoc_true_on_stop() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // Fail to set both
        c.step(Status::PreFailed, true, 0.0);
        assert!(c.status().tfslc());
        assert!(c.status().tftoc());
        // Stop: tftoc true, so cycles_since_last_failed not incremented
        c.stop();
        assert_eq!(*c.nv_config.cycles_since_last_failed, 0u8);
        // But cycles_failed should increment
        assert_eq!(*c.nv_config.cycles_failed, 1u8);
    }

    #[test]
    fn cycles_since_first_failed_increments_on_stop_when_tfslc_true() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // Fail to set tfslc
        c.step(Status::PreFailed, true, 0.0);
        assert!(c.status().tfslc());
        // Stop: tfslc true, so increment cycles_since_first_failed
        c.stop();
        assert_eq!(*c.nv_config.cycles_since_first_failed, 1u8);
        // Stop again: increment again
        c.stop();
        assert_eq!(*c.nv_config.cycles_since_first_failed, 2u8);
    }

    #[test]
    fn cycles_since_first_failed_resets_on_clear() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 5u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // Initially 5
        assert_eq!(*c.nv_config.cycles_since_first_failed, 5u8);
        // Clear: should reset to 0
        c.clear();
        assert_eq!(*c.nv_config.cycles_since_first_failed, 0u8);
    }

    #[test]
    fn cycles_failed_increments_on_stop_when_tftoc_true() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 0u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // Fail to set tftoc
        c.step(Status::PreFailed, true, 0.0);
        assert!(c.status().tftoc());
        // Stop: tftoc true, so increment cycles_failed
        c.stop();
        assert_eq!(*c.nv_config.cycles_failed, 1u8);
        // Stop again: increment again
        c.stop();
        assert_eq!(*c.nv_config.cycles_failed, 2u8);
    }

    #[test]
    fn cycles_failed_resets_on_clear() {
        let c_cfg = CalibConfig { step_up: &1, step_down: &0, debounce_behavior: &DebounceBehavior::Freeze, debounce_type: &DebounceType::CounterBased};
        let mut nv_config = NvmConfig { uds_status: &mut UdsStatusByte::new(0), occurence_cntr: &mut 0u8 , cycles_since_last_failed: &mut 0u8, cycles_since_first_failed: &mut 0u8, cycles_failed: &mut 5u8};
        let mut c: Event<'_, '_> = Event::new(&mut nv_config, &c_cfg);
        // Initially 5
        assert_eq!(*c.nv_config.cycles_failed, 5u8);
        // Clear: should reset to 0
        c.clear();
        assert_eq!(*c.nv_config.cycles_failed, 0u8);
    }

}
