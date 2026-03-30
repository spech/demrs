// ─────────────────────────────────────────────
// Event Management
// ─────────────────────────────────────────────

use crate::extended_record::{EventId, EventManagerError, ExtendedRecord, ExtendedRecordList};
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

/// Error type for [`Event`] operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventError {
    /// Sampling period cannot be negative for time-based debouncing.
    NegativeSampling,
    /// Sampling period cannot be zero for time-based debouncing.
    ZeroSampling,
}

/// Trigger condition for saving extended records.
///
/// Determines when an event's extended record should be created or updated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveTrigger {
    /// Save extended record when `cdtc` (confirmed DTC) is set.
    OnCdtc,
    /// Save extended record when `pdtc` (pending DTC) is set.
    OnPdtc,
}

// ─────────────────────────────────────────────
// CalibConfig
// ─────────────────────────────────────────────

/// Calibration configuration for [`Event`].
///
/// This struct holds step counts, debounce behavior, and type,
/// allowing a single configuration to be shared across multiple events.
#[derive(Clone)]
pub struct CalibConfig {
    /// Number of `PreFailed` ticks to reach `i16::MAX`.
    /// - `0`: Counter unchanged (no debouncing).
    /// - `1`: Immediate snap to confirmed.
    /// - `n > 1`: Gradual accumulation.
    pub step_up: i16,
    /// Number of `PrePassed` ticks to reach `i16::MIN`.
    /// - `0`: Counter unchanged.
    /// - `1`: Immediate snap.
    /// - `n > 1`: Gradual accumulation.
    pub step_down: i16,
    /// Behavior when the event is disabled.
    pub debounce_behavior: DebounceBehavior,
    /// How the counter is changed (tick-based or time-based).
    pub debounce_type: DebounceType,
    /// Number of cycles where failure must be confirmed before setting `cdtc`.
    pub confirmation_threshold: u8,
    /// Number of aging cycles before clearing `cdtc`.
    pub aging_threshold: u8,
    /// Priority of this event in the ordered list.
    pub priority: u8,
    /// Trigger condition for saving extended records.
    pub save_trigger: SaveTrigger,
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
// EventManager
// ─────────────────────────────────────────────

/// Runtime state of the EventManager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventManagerState {
    On,
    Off,
}

/// Manages a collection of [`Event`]s and their associated [`ExtendedRecord`] data.
///
/// The `EventManager` coordinates debouncing logic and persistent storage
/// for diagnostic event handling. It holds references to static event data
/// and extended records that persist across power cycles.
///
/// ## Storage
///
/// - `events` - Slice of [`Event`] instances with fixed static addresses
/// - `extended_records` - Reference to [`ExtendedRecordList`] for persistent metadata
///
/// ## Usage
///
/// Create an `EventManager` by passing static references to event data
/// and extended records storage.
pub struct EventManager {
    /// Slice of [`Event`] instances at fixed static addresses.
    pub events: &'static mut [Event],
    /// Reference to [`ExtendedRecordList`] for persistent metadata.
    pub extended_records: &'static mut ExtendedRecordList,
    /// Runtime state of the EventManager.
    pub state: EventManagerState,
}

impl EventManager {
    /// Clears all extended records by resetting to an empty list.
    pub fn clear_extended_records(&mut self) {
        *self.extended_records = ExtendedRecordList::new();
    }

    /// Initializes all managed events at the start of a new operating cycle.
    ///
    /// Calls [`Event::init()`] on each event to reset `tf` and set `tnctoc`.
    /// Sets the state to [`State::On`].
    pub fn init(&mut self) {
        for event in self.events.iter_mut() {
            event.init();
        }
        self.state = EventManagerState::On;
    }

    /// Stops all managed events at shutdown.
    ///
    /// Sets the state to [`State::Off`].
    /// Calls [`Event::stop()`] on each event to update cycle counters and disable them.
    /// If a falling edge is detected on the save_trigger status bit, the corresponding
    /// extended record is freed.
    pub fn stop(&mut self) {
        self.state = EventManagerState::Off;
        let len = self.events.len();
        for index in 0..len {
            let save_trigger = self.events[index].cal_config.save_trigger;
            let old_status = self.events[index].uds_status_old;
            let new_status = self.events[index].stop();

            let falling_edge = match save_trigger {
                SaveTrigger::OnCdtc => !new_status.cdtc() && old_status.cdtc(),
                SaveTrigger::OnPdtc => !new_status.pdtc() && old_status.pdtc(),
            };

            if falling_edge {
                let event_id = index as EventId;
                self.free_from_extended_records(event_id);
            }
        }
    }

    /// Advances the specified event by one step.
    ///
    /// Returns [`EventManagerError::NotInitializedError`] if the state is [`State::Off`].
    pub fn step(
        &mut self,
        event_id: EventId,
        condition: Status,
        active: bool,
        sampling: f32,
        timestamp: u32,
    ) -> Result<UdsStatusByte, EventManagerError> {
        if self.state != EventManagerState::On {
            return Err(EventManagerError::NotInitializedError);
        }

        let index = event_id as usize;
        if index >= self.events.len() {
            return Err(EventManagerError::InvalidEventIdError);
        }

        let event = &mut self.events[index];
        event
            .step(condition, active, sampling)
            .map_err(|_| EventManagerError::EventStepError)?;
        let new_status = event.nv_config.uds_status;

        self.store_in_extended_records(index, timestamp)?;

        Ok(new_status)
    }

    fn store_in_extended_records(
        &mut self,
        index: usize,
        timestamp: u32,
    ) -> Result<(), EventManagerError> {
        let event_id = index as EventId;
        let event = &self.events[index];
        let priority = event.cal_config.priority;
        let save_trigger = event.cal_config.save_trigger;
        let old_status = event.uds_status_old;
        let new_status = event.nv_config.uds_status;

        let rising_edge = match save_trigger {
            SaveTrigger::OnCdtc => new_status.cdtc() && !old_status.cdtc(),
            SaveTrigger::OnPdtc => new_status.pdtc() && !old_status.pdtc(),
        };

        if rising_edge {
            if let Some(existing) = self.extended_records.get_by_event_id_mut(event_id) {
                existing.date_at_last_save = timestamp;
            } else {
                let ext_rec = ExtendedRecord {
                    event_id,
                    priority,
                    date_at_first_save: timestamp,
                    date_at_last_save: timestamp,
                };
                self.extended_records.insert(ext_rec)?;
            }
        }

        Ok(())
    }

    /// Frees (removes) an entry from the extended records list by event ID.
    ///
    /// If an entry with the given event ID exists, it is removed.
    /// If no entry exists with that event ID, this function does nothing.
    ///
    /// # Arguments
    ///
    /// * `event_id` - The event ID of the entry to free.
    pub fn free_from_extended_records(&mut self, event_id: EventId) {
        let index_to_remove = self
            .extended_records
            .iter()
            .position(|rec| rec.event_id == event_id);

        if let Some(index) = index_to_remove {
            self.extended_records.remove(index);
        }
    }
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
    pub(crate) debounce_counter: i16,
    /// Previous UDS status byte for detecting rising edges.
    pub(crate) uds_status_old: UdsStatusByte,
    /// Whether debouncing is disabled.
    pub(crate) disabled: bool,
    /// Reference to non-volatile configuration.
    pub(crate) nv_config: &'static mut NvmConfig,
    /// Reference to calibration configuration.
    pub(crate) cal_config: &'static CalibConfig,
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
    /// # Arguments
    ///
    /// * `condition` - The status signal driving the debouncing.
    /// * `active` - If `false`, skips debouncing and returns previous status.
    /// * `sampling` - Sampling period (used for time-based debouncing).
    ///
    /// # Returns
    ///
    /// * `Err(EventError::NegativeSampling)` if `sampling` is negative for time-based debouncing.
    /// * `Err(EventError::ZeroSampling)` if `sampling` is zero for time-based debouncing.
    /// * `Ok(UdsStatusByte)` with the updated status after processing the step.
    pub fn step(
        &mut self,
        condition: Status,
        active: bool,
        sampling: f32,
    ) -> Result<UdsStatusByte, EventError> {
        if self.cal_config.debounce_type == DebounceType::TimeBased {
            if sampling < 0.0 {
                return Err(EventError::NegativeSampling);
            }
            if sampling == 0.0 {
                return Err(EventError::ZeroSampling);
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
                            increment = div_round(self.cal_config.step_up, sampling as i16);
                        }
                        increment = div_round(i16::MAX, increment);
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
                            decrement = div_round(self.cal_config.step_down, sampling as i16);
                        }
                        decrement = div_round(i16::MAX, decrement);
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

// ─────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn create_cal_config(
        step_up: i16,
        step_down: i16,
        confirmation_thr: u8,
        aging_thr: u8,
        debounce_type: DebounceType,
        debounce_behavior: DebounceBehavior,
        priority: u8,
        save_trigger: SaveTrigger,
    ) -> &'static CalibConfig {
        Box::leak(Box::new(CalibConfig {
            step_up,
            step_down,
            debounce_behavior,
            debounce_type,
            confirmation_threshold: confirmation_thr,
            aging_threshold: aging_thr,
            priority,
            save_trigger,
        }))
    }

    fn create_nvm_config(
        occ: u8,
        aging: u8,
        confirm: u8,
        tftoc: bool,
        tnctoc: bool,
        cdtc: bool,
    ) -> &'static mut NvmConfig {
        let mut uds = UdsStatusByte::new(0);
        uds.set_tftoc(tftoc);
        uds.set_tnctoc(tnctoc);
        uds.set_cdtc(cdtc);

        Box::leak(Box::new(NvmConfig {
            uds_status: uds,
            occurence_cntr: occ,
            aging_cycles: aging,
            confirmation_cycles: confirm,
        }))
    }

    fn create_event(
        _event_id: EventId,
        nv_config: &'static mut NvmConfig,
        cal_config: &'static CalibConfig,
    ) -> Event {
        Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::new(0),
            disabled: false,
            nv_config,
            cal_config,
        }
    }

    // ────────────────────────────────────────────
    // Configuration Tests
    // ────────────────────────────────────────────

    #[test]
    fn calib_config_fields_accessible() {
        let cfg = create_cal_config(
            2,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        assert_eq!(cfg.confirmation_threshold, 3);
        assert_eq!(cfg.aging_threshold, 5);
    }

    #[test]
    fn nvm_config_fields_accessible() {
        let n_cfg = create_nvm_config(5, 2, 3, false, true, false);

        assert_eq!(n_cfg.occurence_cntr, 5u8);
        assert_eq!(n_cfg.aging_cycles, 2u8);
        assert_eq!(n_cfg.confirmation_cycles, 3u8);
    }

    #[test]
    fn event_cal_config_accessible() {
        let c_cfg = create_cal_config(
            1,
            0,
            4,
            6,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        assert_eq!(evt.cal_config.confirmation_threshold, 4u8);
        assert_eq!(evt.cal_config.aging_threshold, 6u8);
    }

    #[test]
    fn event_init_resets_tf_and_sets_tnctoc() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, false, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::new(0),
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };

        evt.nv_config.uds_status.set_tf(true);

        assert!(evt.nv_config.uds_status.tf());
        assert!(!evt.nv_config.uds_status.tnctoc());

        evt.init();

        assert!(!evt.nv_config.uds_status.tf());
        assert!(evt.nv_config.uds_status.tnctoc());
    }

    #[test]
    fn event_priority_returns_config_value() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            42,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        assert_eq!(evt.priority(), 42);
    }

    // ────────────────────────────────────────────
    // Debouncing Logic Tests
    // ────────────────────────────────────────────

    #[test]
    fn prefailed_increments_counter() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::PreFailed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MAX);
        assert!(evt.status().tf());
    }

    #[test]
    fn prepassed_decrements_counter() {
        let c_cfg = create_cal_config(
            0,
            1,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::Failed, true, 0.0).unwrap();
        evt.step(Status::PrePassed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MIN);
        assert!(!evt.status().tf());
    }

    #[test]
    fn failed_status_immediate_confirmation() {
        let c_cfg = create_cal_config(
            2,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::Failed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MAX);
        assert!(evt.status().tf());
    }

    #[test]
    fn snap_failed_cdtc_threshold_not_reached_occurrence_not_incremented() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 2, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.nv_config.uds_status.set_tf(true);

        evt.step(Status::Failed, true, 0.0).unwrap();

        assert!(!evt.status().cdtc());
        assert_eq!(evt.nv_config.aging_cycles, 0);
        assert_eq!(evt.nv_config.occurence_cntr, 0);
    }

    #[test]
    fn prefailed_no_debounce_when_step_up_zero() {
        let c_cfg = create_cal_config(
            0,
            1,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::PreFailed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), 0);
        assert!(!evt.status().tf());
    }

    #[test]
    fn prefailed_accumulates_and_snaps() {
        let c_cfg = create_cal_config(
            2,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::Failed, true, 0.0).unwrap();
        evt.step(Status::Passed, true, 0.0).unwrap();
        assert!(!evt.status().tf());
        assert!(evt.debounce_counter() < 0);

        evt.step(Status::PreFailed, true, 0.0).unwrap();
        assert!(evt.debounce_counter() > 0);
        assert!(!evt.status().tf());

        evt.step(Status::PreFailed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MAX);
        assert!(evt.status().tf());
    }

    #[test]
    fn prepassed_no_debounce_when_step_down_zero() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::Failed, true, 0.0).unwrap();
        assert!(evt.status().tf());

        evt.step(Status::PrePassed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MAX);
        assert!(evt.status().tf());
    }

    #[test]
    fn passed_status_immediate_clear() {
        let c_cfg = create_cal_config(
            0,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::Failed, true, 0.0).unwrap();
        assert!(evt.status().tf());
        evt.step(Status::Passed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MIN);
        assert!(!evt.status().tf());
    }

    #[test]
    fn counter_saturates_at_max() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::PreFailed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MAX);
        evt.step(Status::PreFailed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MAX);
    }

    #[test]
    fn counter_saturates_at_min() {
        let c_cfg = create_cal_config(
            0,
            1,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::Failed, true, 0.0).unwrap();
        evt.step(Status::PrePassed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MIN);
        evt.step(Status::PrePassed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MIN);
    }

    #[test]
    fn disabled_event_freezes_counter() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::PreFailed, true, 0.0).unwrap();
        let counter_val = evt.debounce_counter();
        evt.disable(true);
        evt.step(Status::PrePassed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), counter_val);
    }

    #[test]
    fn disabled_event_resets_counter() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Reset,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::PreFailed, true, 0.0).unwrap();
        assert!(evt.debounce_counter() > 0);
        evt.disable(true);
        evt.step(Status::PrePassed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), 0);
    }

    #[test]
    fn clear_resets_debounce_state() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::PreFailed, true, 0.0).unwrap();
        assert!(evt.status().tf());
        evt.clear();
        assert_eq!(evt.debounce_counter(), 0);
        assert!(!evt.status().tf());
    }

    #[test]
    fn prepassed_accumulates_and_snaps() {
        let c_cfg = create_cal_config(
            0,
            2,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::Failed, true, 0.0).unwrap();
        assert!(evt.status().tf());

        evt.step(Status::PrePassed, true, 0.0).unwrap();
        assert!(evt.debounce_counter() < 0);
        assert!(evt.status().tf());

        evt.step(Status::PrePassed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MIN);
        assert!(!evt.status().tf());
    }

    // ────────────────────────────────────────────
    // Threshold Boundary Tests
    // ────────────────────────────────────────────

    #[test]
    fn calib_config_thresholds_set_correctly() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        // This test verifies that thresholds are set correctly
        assert_eq!(c_cfg.confirmation_threshold, 3u8);
        assert_eq!(c_cfg.aging_threshold, 5u8);
    }

    // ────────────────────────────────────────────
    // TimeBased Debounce Type Tests
    // ────────────────────────────────────────────

    #[test]
    fn timebased_prefailed_with_sampling_period() {
        let c_cfg = create_cal_config(
            100,
            0,
            3,
            5,
            DebounceType::TimeBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        // With sampling_period=10ms, should reduce effective step_up
        evt.step(Status::PreFailed, true, 10.0).unwrap();
        assert!(evt.debounce_counter() > 0);
        assert!(!evt.status().tf()); // Not yet confirmed
    }

    #[test]
    fn timebased_prefailed_accumulates_with_multiple_steps() {
        let c_cfg = create_cal_config(
            100,
            0,
            3,
            5,
            DebounceType::TimeBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        let counter_after_first = {
            evt.step(Status::PreFailed, true, 10.0).unwrap();
            evt.debounce_counter()
        };

        evt.step(Status::PreFailed, true, 10.0).unwrap();
        let counter_after_second = evt.debounce_counter();

        // Counter should increase with each step
        assert!(counter_after_second >= counter_after_first);
    }

    #[test]
    fn timebased_prepassed_with_sampling_period() {
        let c_cfg = create_cal_config(
            0,
            100,
            3,
            5,
            DebounceType::TimeBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.step(Status::Failed, true, 10.0).unwrap();
        assert!(evt.status().tf());

        // With sampling_period=10ms, should reduce effective step_down
        evt.step(Status::PrePassed, true, 10.0).unwrap();
        assert!(evt.debounce_counter() < 0);
        assert!(evt.status().tf()); // Still true, not enough decrement
    }

    #[test]
    fn timebased_sampling_period_affects_counter() {
        let c_cfg = create_cal_config(
            1000,
            0,
            3,
            5,
            DebounceType::TimeBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg1 = create_nvm_config(0, 0, 0, false, true, false);
        let n_cfg2 = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt1 = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg1.uds_status,
            disabled: false,
            nv_config: n_cfg1,
            cal_config: c_cfg,
        };
        evt1.step(Status::PreFailed, true, 10.0).unwrap();
        let counter_10ms = evt1.debounce_counter();

        let mut evt2 = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg2.uds_status,
            disabled: false,
            nv_config: n_cfg2,
            cal_config: c_cfg,
        };
        evt2.step(Status::PreFailed, true, 100.0).unwrap();
        let counter_100ms = evt2.debounce_counter();

        assert_ne!(counter_10ms, counter_100ms);
        assert!(counter_10ms > 0);
        assert!(counter_100ms > 0);
    }

    #[test]
    fn timebased_negative_sampling_error() {
        let c_cfg = create_cal_config(
            100,
            0,
            3,
            5,
            DebounceType::TimeBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        assert_eq!(
            evt.step(Status::PreFailed, true, -1.0),
            Err(EventError::NegativeSampling)
        );
    }

    #[test]
    fn timebased_zero_sampling_error() {
        let c_cfg = create_cal_config(
            100,
            0,
            3,
            5,
            DebounceType::TimeBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        assert_eq!(
            evt.step(Status::PreFailed, true, 0.0),
            Err(EventError::ZeroSampling)
        );
    }

    // ────────────────────────────────────────────
    // Stop Method Tests
    // ────────────────────────────────────────────

    #[test]
    fn stop_not_failed_aging_sets_pdtc_false() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, false, true);

        let status = {
            let mut evt = Event {
                debounce_counter: 0,
                uds_status_old: n_cfg.uds_status,
                disabled: false,
                nv_config: n_cfg,
                cal_config: c_cfg,
            };
            assert!(!evt.status().tftoc());
            assert!(!evt.status().tnctoc());
            assert!(evt.status().cdtc());
            evt.stop();
            evt.status()
        };

        assert!(!status.pdtc());
        assert!(status.cdtc());
    }

    #[test]
    fn stop_tftoc_true_increments_confirmation_cycles() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 2, true, false, false);

        let conf_cycles_after = {
            let mut evt = Event {
                debounce_counter: 0,
                uds_status_old: n_cfg.uds_status,
                disabled: false,
                nv_config: n_cfg,
                cal_config: c_cfg,
            };
            evt.stop();
            evt.nv_config.confirmation_cycles
        };

        assert_eq!(conf_cycles_after, 3);
    }

    #[test]
    fn stop_tftoc_true_cdtc_already_set() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let conf_cycles_after = {
            let mut evt = Event {
                debounce_counter: 0,
                uds_status_old: n_cfg.uds_status,
                disabled: false,
                nv_config: n_cfg,
                cal_config: c_cfg,
            };
            evt.stop();
            evt.nv_config.confirmation_cycles
        };

        assert_eq!(conf_cycles_after, 3);
    }

    #[test]
    fn stop_tnctoc_true_does_nothing() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let conf_cycles_after = {
            let mut evt = Event {
                debounce_counter: 0,
                uds_status_old: n_cfg.uds_status,
                disabled: false,
                nv_config: n_cfg,
                cal_config: c_cfg,
            };
            assert!(!evt.status().tftoc());
            assert!(evt.status().tnctoc());
            evt.stop();
            evt.nv_config.confirmation_cycles
        };

        assert_eq!(conf_cycles_after, 0);
    }

    #[test]
    fn stop_tftoc_false_cdtc_false_sets_pdtc() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, false, false);

        let status = {
            let mut evt = Event {
                debounce_counter: 0,
                uds_status_old: n_cfg.uds_status,
                disabled: false,
                nv_config: n_cfg,
                cal_config: c_cfg,
            };
            assert!(!evt.status().tftoc());
            assert!(!evt.status().tnctoc());
            assert!(!evt.status().cdtc());
            evt.stop();
            evt.status()
        };

        assert!(!status.pdtc());
    }

    #[test]
    fn stop_aging_not_complete_increments_aging() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 2, 0, false, false, true);

        let aging_after = {
            let mut evt = Event {
                debounce_counter: 0,
                uds_status_old: n_cfg.uds_status,
                disabled: false,
                nv_config: n_cfg,
                cal_config: c_cfg,
            };
            assert!(!evt.status().tftoc());
            assert!(!evt.status().tnctoc());
            assert!(evt.status().cdtc());
            assert_eq!(evt.nv_config.aging_cycles, 2);
            evt.stop();
            evt.nv_config.aging_cycles
        };

        assert_eq!(aging_after, 3);
    }

    #[test]
    fn stop_not_failed_aging_completes_clears_cdtc() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 5, 0, false, false, true);

        let status = {
            let mut evt = Event {
                debounce_counter: 0,
                uds_status_old: n_cfg.uds_status,
                disabled: false,
                nv_config: n_cfg,
                cal_config: c_cfg,
            };
            assert!(!evt.status().tftoc());
            assert!(!evt.status().tnctoc());
            assert!(evt.status().cdtc());
            evt.stop();
            evt.status()
        };

        assert!(!status.pdtc());
        assert!(!status.cdtc());
    }

    #[test]
    fn stop_not_failed_no_cdtc() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, false, false);

        let status = {
            let mut evt = Event {
                debounce_counter: 0,
                uds_status_old: n_cfg.uds_status,
                disabled: false,
                nv_config: n_cfg,
                cal_config: c_cfg,
            };
            assert!(!evt.status().tftoc());
            assert!(!evt.status().tnctoc());
            assert!(!evt.status().cdtc());
            evt.stop();
            evt.status()
        };

        assert!(!status.pdtc());
        assert!(!status.cdtc());
    }

    #[test]
    fn stop_failed_increments_confirmation_cycles() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 2, true, false, false);

        let conf_cycles_after = {
            let mut evt = Event {
                debounce_counter: 0,
                uds_status_old: n_cfg.uds_status,
                disabled: false,
                nv_config: n_cfg,
                cal_config: c_cfg,
            };
            assert!(evt.status().tftoc());
            assert!(!evt.status().tnctoc());
            assert!(!evt.status().cdtc());
            evt.stop();
            evt.nv_config.confirmation_cycles
        };

        assert_eq!(conf_cycles_after, 3);
    }

    #[test]
    fn stop_disables_event() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, false, true);

        let mut evt = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        evt.stop();
        assert!(evt.status().cdtc());
        evt.step(Status::PreFailed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), 0);
    }

    // ────────────────────────────────────────────
    // EventManager Tests
    // ────────────────────────────────────────────

    #[test]
    fn event_manager_step_success() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let event = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        manager.init();
        let result = manager.step(0, Status::PreFailed, true, 0.0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn event_manager_init_calls_all_events() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg1 = create_nvm_config(0, 0, 0, false, false, false);
        let n_cfg2 = create_nvm_config(0, 0, 0, false, false, false);

        n_cfg1.uds_status.set_tf(true);
        n_cfg2.uds_status.set_tf(true);

        let event1 = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::new(0),
            disabled: false,
            nv_config: n_cfg1,
            cal_config: c_cfg,
        };
        let event2 = Event {
            debounce_counter: 0,
            uds_status_old: UdsStatusByte::new(0),
            disabled: false,
            nv_config: n_cfg2,
            cal_config: c_cfg,
        };
        let events = Box::leak(Box::new([event1, event2]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        assert!(manager.events[0].nv_config.uds_status.tf());
        assert!(!manager.events[0].nv_config.uds_status.tnctoc());
        assert!(manager.events[1].nv_config.uds_status.tf());
        assert!(!manager.events[1].nv_config.uds_status.tnctoc());

        manager.init();

        assert!(!manager.events[0].nv_config.uds_status.tf());
        assert!(manager.events[0].nv_config.uds_status.tnctoc());
        assert!(!manager.events[1].nv_config.uds_status.tf());
        assert!(manager.events[1].nv_config.uds_status.tnctoc());
    }

    #[test]
    fn event_manager_stop_calls_all_events() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg1 = create_nvm_config(0, 0, 0, true, false, true);
        let n_cfg2 = create_nvm_config(0, 0, 0, true, false, true);

        let event1 = create_event(0, n_cfg1, c_cfg);
        let event2 = create_event(1, n_cfg2, c_cfg);
        let events = Box::leak(Box::new([event1, event2]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        assert!(!manager.events[0].disabled);
        assert!(!manager.events[1].disabled);

        manager.stop();

        assert!(manager.events[0].disabled);
        assert!(manager.events[1].disabled);
    }

    #[test]
    fn event_manager_free_from_extended_records() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg1 = create_nvm_config(0, 0, 3, false, true, false);
        let n_cfg2 = create_nvm_config(0, 0, 3, false, true, false);

        let event1 = create_event(0, n_cfg1, c_cfg);
        let event2 = create_event(1, n_cfg2, c_cfg);
        let events = Box::leak(Box::new([event1, event2]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        manager.init();
        manager.step(0, Status::Failed, true, 0.0, 100).unwrap();
        manager.step(1, Status::Failed, true, 0.0, 100).unwrap();

        assert_eq!(manager.extended_records.len(), 2);
        assert!(manager.extended_records.get_by_event_id(0).is_some());
        assert!(manager.extended_records.get_by_event_id(1).is_some());

        manager.free_from_extended_records(0);

        assert_eq!(manager.extended_records.len(), 1);
        assert!(manager.extended_records.get_by_event_id(0).is_none());
        assert!(manager.extended_records.get_by_event_id(1).is_some());

        manager.free_from_extended_records(99);

        assert_eq!(manager.extended_records.len(), 1);
    }

    #[test]
    fn event_manager_step_invalid_id() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let event = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        manager.init();
        let result = manager.step(99, Status::PreFailed, true, 0.0, 0);
        assert_eq!(result.unwrap_err(), EventManagerError::InvalidEventIdError);
    }

    #[test]
    fn event_manager_step_event_error() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::TimeBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let event = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        manager.init();
        let result = manager.step(0, Status::PreFailed, true, -1.0, 0);
        assert_eq!(result.unwrap_err(), EventManagerError::EventStepError);
    }

    #[test]
    fn event_manager_step_returns_error_when_state_off() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let event = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        let result = manager.step(0, Status::Failed, true, 0.0, 100);
        assert_eq!(result.unwrap_err(), EventManagerError::NotInitializedError);
    }

    #[test]
    fn event_manager_step_succeeds_after_init() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let event = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        manager.init();
        let result = manager.step(0, Status::Failed, true, 0.0, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn event_manager_rising_edge_creates_extended_record() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let event = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        manager.init();
        manager.step(0, Status::Failed, true, 0.0, 0).unwrap();

        let ext_rec = manager.extended_records.get_by_event_id(0);
        assert!(ext_rec.is_some());
        assert_eq!(ext_rec.unwrap().event_id, 0);
    }

    #[test]
    fn event_manager_no_rising_edge_no_extended_record() {
        let c_cfg = create_cal_config(
            0,
            0,
            3,
            5,
            DebounceType::TimeBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let event = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        manager.init();
        manager.step(0, Status::Passed, true, 1.0, 0).unwrap();

        assert!(manager.extended_records.is_empty());
    }

    #[test]
    fn event_manager_extended_record_no_update_without_rising_edge() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let event = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        manager.init();

        manager.step(0, Status::Failed, true, 0.0, 100).unwrap();
        let first_rec = manager.extended_records.get_by_event_id(0).unwrap();
        let first_date = first_rec.date_at_last_save;

        manager.step(0, Status::Failed, true, 0.0, 200).unwrap();
        let second_rec = manager.extended_records.get_by_event_id(0).unwrap();

        assert_eq!(second_rec.date_at_last_save, first_date);
        assert_eq!(second_rec.event_id, 0);
    }

    #[test]
    fn event_manager_extended_record_reinsert_on_rising_edge_after_stop() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
            SaveTrigger::OnCdtc,
        );
        let n_cfg = create_nvm_config(0, 0, 3, false, true, false);

        let event = Event {
            debounce_counter: 0,
            uds_status_old: n_cfg.uds_status,
            disabled: false,
            nv_config: n_cfg,
            cal_config: c_cfg,
        };
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        manager.init();
        manager.step(0, Status::Failed, true, 0.0, 100).unwrap();
        let first_date = manager
            .extended_records
            .get_by_event_id(0)
            .unwrap()
            .date_at_last_save;

        manager.stop();
        manager.init();

        manager.step(0, Status::Failed, true, 0.0, 200).unwrap();
        let second_date = manager
            .extended_records
            .get_by_event_id(0)
            .unwrap()
            .date_at_last_save;

        assert_eq!(second_date, 200);
        assert_ne!(first_date, second_date);
    }

    #[test]
    fn event_manager_extended_records_list_full_replaces_lowest_priority() {
        let mut events_vec: Vec<Event> = Vec::with_capacity(25);

        for i in 0..25u8 {
            let priority = if i < 24 { 10 + i } else { 0 };
            let c_cfg = create_cal_config(
                1,
                0,
                3,
                5,
                DebounceType::CounterBased,
                DebounceBehavior::Freeze,
                priority,
                SaveTrigger::OnCdtc,
            );
            let n_cfg = create_nvm_config(0, 0, 3, false, true, false);
            events_vec.push(Event {
                debounce_counter: 0,
                uds_status_old: n_cfg.uds_status,
                disabled: false,
                nv_config: n_cfg,
                cal_config: c_cfg,
            });
        }

        let events = Box::leak(events_vec.into_boxed_slice());
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));
        let mut manager = EventManager {
            events,
            extended_records: ext_list,
            state: EventManagerState::Off,
        };

        manager.init();
        for i in 0..=24 {
            let result = manager.step(i as EventId, Status::Failed, true, 0.0, i as u32);
            if result.is_err() {
                panic!("step({}) failed: {:?}", i, result.unwrap_err());
            }
        }

        assert!(manager.extended_records.is_full());
        assert_eq!(manager.extended_records.len(), 24);

        assert!(manager.extended_records.get_by_event_id(0).is_none());
        assert!(manager.extended_records.get_by_event_id(24).is_some());
    }
}
