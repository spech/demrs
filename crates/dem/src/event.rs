// ─────────────────────────────────────────────
// Event Management
// ─────────────────────────────────────────────

use std::f32::consts::E;

use crate::UdsStatusByte;
use chrono::{DateTime, Utc};

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
// ExtendedRecord
// ─────────────────────────────────────────────

pub type EventId = u16;

/// Persistent data associated with an [`Event`].
///
/// Stores event metadata that survives across power cycles, including:
/// - Event identifier
/// - Priority for ordering in [`ExtendedRecordList`]
/// - Timestamps for first and last save operations
///
/// ## Usage
///
/// [`ExtendedRecord`] instances are stored in [`ExtendedRecordList`]
/// and managed by [`EventManager`].
#[derive(Clone)]
pub struct ExtendedRecord {
    /// Unique identifier for this event.
    pub event_id: EventId,
    /// Priority value used for ordering in [`ExtendedRecordList`].
    pub priority: u8,
    /// Timestamp of the first save operation.
    pub date_at_first_save: Option<DateTime<Utc>>,
    /// Timestamp of the last save operation.
    pub date_at_last_save: Option<DateTime<Utc>>,
}

// ─────────────────────────────────────────────
// EventManagerError
// ─────────────────────────────────────────────

/// Error type for [`EventManager`] operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventManagerError {
    /// The extended records list has reached its maximum capacity.
    ListFullError,
    /// The provided event ID is out of bounds.
    InvalidEventIdError,
    /// The event step operation failed.
    EventStepError,
}

// ─────────────────────────────────────────────
// ExtendedRecordList
// ─────────────────────────────────────────────

/// A fixed-capacity list of [`ExtendedRecord`] entries.
///
/// Maintains [`ExtendedRecord`] entries sorted by priority in ascending order.
/// Used for storing persistent event metadata that survives across power cycles.
///
/// ## Capacity
///
/// The list has a fixed capacity of 24 entries. Attempting to insert
/// when full returns [`EventManagerError::ListFullError`].
///
/// ## Usage
///
/// Create an instance with [`ExtendedRecordList::new()`], insert records
/// with [`ExtendedRecordList::insert()`], and access them via
/// [`ExtendedRecordList::get()`] or iterators.
pub struct ExtendedRecordList {
    data: [Option<ExtendedRecord>; 24],
    len: usize,
}

/// Result of finding the lowest priority entry in an [`ExtendedRecordList`].
pub struct LowestPriorityResult {
    /// Index of the lowest priority entry.
    pub index: usize,
    /// The lowest priority value.
    pub priority: u8,
}

impl ExtendedRecordList {
    const CAPACITY: usize = 24;

    /// Creates a new, empty `ExtendedRecordList`.
    ///
    /// # Returns
    ///
    /// A new empty list with capacity for 24 entries.
    pub const fn new() -> Self {
        Self {
            data: [const { None }; 24],
            len: 0,
        }
    }

    /// Returns the number of entries in the list.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the list contains no entries.
    ///
    /// # Returns
    ///
    /// `true` if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` if the list has reached its maximum capacity.
    ///
    /// # Returns
    ///
    /// `true` if the list is full (24 entries).
    pub fn is_full(&self) -> bool {
        self.len == Self::CAPACITY
    }

    /// Returns a reference to the entry with the given event ID.
    ///
    /// # Arguments
    ///
    /// * `event_id` - The event ID to search for.
    ///
    /// # Returns
    ///
    /// `Some(&ExtendedRecord)` if an entry with the given event ID exists, `None` otherwise.
    pub fn get_by_event_id(&self, event_id: EventId) -> Option<&ExtendedRecord> {
        self.iter().find(|ext_rec| ext_rec.event_id == event_id)
    }

    /// Returns a mutable reference to the entry with the given event ID.
    ///
    /// # Arguments
    ///
    /// * `event_id` - The event ID to search for.
    ///
    /// # Returns
    ///
    /// `Some(&mut ExtendedRecord)` if an entry with the given event ID exists, `None` otherwise.
    pub fn get_by_event_id_mut(&mut self, event_id: EventId) -> Option<&mut ExtendedRecord> {
        self.iter_mut().find(|ext_rec| ext_rec.event_id == event_id)
    }

    /// Returns an iterator over all entries in insertion order.
    ///
    /// # Returns
    ///
    /// An iterator yielding references to entries.
    pub fn iter(&self) -> impl Iterator<Item = &ExtendedRecord> {
        self.data
            .iter()
            .take(self.len)
            .filter_map(|opt| opt.as_ref())
    }

    /// Returns a mutable iterator over all entries in insertion order.
    ///
    /// # Returns
    ///
    /// An iterator yielding mutable references to entries.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ExtendedRecord> {
        self.data
            .iter_mut()
            .take(self.len)
            .filter_map(|opt| opt.as_mut())
    }

    /// Inserts a new entry into the list, maintaining priority order.
    ///
    /// If the list is full, replaces the lowest priority entry if the new entry has higher priority.
    ///
    /// # Arguments
    ///
    /// * `ext_rec` - The entry to insert.
    ///
    /// # Returns
    ///
    /// `Ok(())` if insertion succeeded.
    /// `Err(EventManagerError::ListFullError)` if the list is full and no entry has lower priority.
    pub fn insert(&mut self, ext_rec: ExtendedRecord) -> Result<(), EventManagerError> {
        if self.is_full() {
            let new_priority = ext_rec.priority;
            let lowest = self.find_lowest_priority();
            if new_priority < lowest.priority {
                self.remove(lowest.index);
            } else {
                return Err(EventManagerError::ListFullError);
            }
        }

        let priority = ext_rec.priority;

        let pos = self.data[..self.len]
            .iter()
            .position(|opt| opt.as_ref().map(|e| e.priority > priority).unwrap_or(false))
            .unwrap_or(self.len);

        for i in (pos..self.len).rev() {
            self.data[i + 1] = self.data[i].take();
        }

        self.data[pos] = Some(ext_rec);
        self.len += 1;
        Ok(())
    }

    /// Finds the entry with the lowest priority.
    ///
    /// # Returns
    ///
    /// A struct containing the index and priority of the lowest priority entry.
    pub fn find_lowest_priority(&self) -> LowestPriorityResult {
        let mut lowest_idx = 0;
        let mut lowest_priority = u8::MAX;

        for i in 0..self.len {
            if let Some(ext_rec) = &self.data[i] {
                if ext_rec.priority < lowest_priority {
                    lowest_priority = ext_rec.priority;
                    lowest_idx = i;
                }
            }
        }

        LowestPriorityResult {
            index: lowest_idx,
            priority: lowest_priority,
        }
    }

    /// Removes and returns the entry at the given index.
    ///
    /// # Arguments
    ///
    /// * `index` - The index of the entry to remove.
    ///
    /// # Returns
    ///
    /// `Some(ExtendedRecord)` if the index is valid, `None` otherwise.
    pub fn remove(&mut self, index: usize) -> Option<ExtendedRecord> {
        if index >= self.len {
            return None;
        }

        let removed = self.data[index].take();

        for i in index..self.len - 1 {
            self.data[i] = self.data[i + 1].take();
        }

        self.len -= 1;
        removed
    }

    /// Removes and returns the entry with the given priority.
    ///
    /// # Arguments
    ///
    /// * `priority` - The priority of the entry to remove.
    ///
    /// # Returns
    ///
    /// `Some(ExtendedRecord)` if an entry with the given priority exists, `None` otherwise.
    pub fn remove_by_priority(&mut self, priority: u8) -> Option<ExtendedRecord> {
        let index = self.data[..self.len].iter().position(|opt| {
            opt.as_ref()
                .map(|e| e.priority == priority)
                .unwrap_or(false)
        });

        if let Some(idx) = index {
            self.remove(idx)
        } else {
            None
        }
    }
}

// ─────────────────────────────────────────────
// EventManager
// ─────────────────────────────────────────────

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
    /// All managed [`Event`] instances.
    events: &'static mut [Event],
    /// Persistent extended record storage.
    extended_records: &'static mut ExtendedRecordList,
    /// Bit mask for triggering extended record updates on rising edges.
    extended_record_mask: u8,
}

impl EventManager {
    /// Creates a new `EventManager` with the given static references.
    ///
    /// # Arguments
    ///
    /// * `events` - Static slice of [`Event`] instances.
    /// * `extended_records` - Static reference to [`ExtendedRecordList`].
    /// * `extended_record_mask` - Bit mask for triggering extended record updates on rising edges.
    ///
    /// # Returns
    ///
    /// A new `EventManager` instance managing the provided events and records.
    pub fn new(
        events: &'static mut [Event],
        extended_records: &'static mut ExtendedRecordList,
        extended_record_mask: u8,
    ) -> Self {
        Self {
            events,
            extended_records,
            extended_record_mask,
        }
    }

    /// Advances the specified event by one step.
    ///
    /// # Arguments
    ///
    /// * `event_id` - The ID of the event to step.
    /// * `condition` - The status signal driving the debouncing.
    /// * `active` - If `false`, skips debouncing and returns previous status.
    /// * `sampling` - Sampling period (used for time-based debouncing).
    ///
    /// # Returns
    ///
    /// * `Ok(UdsStatusByte)` with the updated status after processing the step.
    /// * `Err(EventManagerError::InvalidEventIdError)` if the event ID is out of bounds.
    /// * `Err(EventManagerError::EventStepError)` if the event step operation failed.
    pub fn step(
        &mut self,
        event_id: EventId,
        condition: Status,
        active: bool,
        sampling: f32,
    ) -> Result<UdsStatusByte, EventManagerError> {
        let index = event_id as usize;
        if index >= self.events.len() {
            return Err(EventManagerError::InvalidEventIdError);
        }

        let event = &mut self.events[index];
        let event_id = event.event_id;
        let priority = event.cal_config.priority;
        let old_status = event.uds_status_old;
        let new_status = event
            .step(condition, active, sampling)
            .map_err(|_| EventManagerError::EventStepError)?;

        self.store_in_extended_records(event_id, priority, old_status, new_status)?;

        Ok(new_status)
    }

    /// Stores or updates an extended record when a rising edge is detected on the status bits
    /// that match `extended_record_mask`.
    ///
    /// A rising edge occurs when a status bit transitions from 0 to 1. If the event already
    /// exists in the extended records list, its `date_at_last_save` is updated. Otherwise,
    /// a new entry is created with both timestamps set to the current time.
    fn store_in_extended_records(
        &mut self,
        event_id: EventId,
        priority: u8,
        old_status: UdsStatusByte,
        new_status: UdsStatusByte,
    ) -> Result<(), EventManagerError> {
        let rising_edge =
            ((new_status.raw() ^ old_status.raw()) & new_status.raw()) & self.extended_record_mask;
        let status_triggered = rising_edge != 0;

        if status_triggered {
            if let Some(existing) = self.extended_records.get_by_event_id_mut(event_id) {
                existing.date_at_last_save = Some(Utc::now());
            } else {
                let ext_rec = ExtendedRecord {
                    event_id,
                    priority,
                    date_at_first_save: Some(Utc::now()),
                    date_at_last_save: Some(Utc::now()),
                };
                self.extended_records.insert(ext_rec)?;
            }
        }

        Ok(())
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
    event_id: EventId,
    debounce_counter: i16,
    pub uds_status_old: UdsStatusByte,
    disabled: bool,
    nv_config: &'static mut NvmConfig,
    cal_config: &'static CalibConfig,
}

impl Event {
    /// Creates a new `Event` with the given configurations.
    ///
    /// # Arguments
    ///
    /// * `event_id` - Unique identifier for this event.
    /// * `nv_config` - Non-volatile config holding persistent state.
    /// * `cal_config` - Calibration config with step counts and behavior.
    ///
    /// # Returns
    ///
    /// A new `Event` instance, initialized with counter at 0 and status copied.
    pub fn new(
        event_id: EventId,
        nv_config: &'static mut NvmConfig,
        cal_config: &'static CalibConfig,
    ) -> Self {
        Self {
            event_id,
            debounce_counter: 0i16,
            uds_status_old: nv_config.uds_status,
            disabled: false,
            nv_config,
            cal_config,
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
                self.nv_config.aging_cycles = self.nv_config.aging_cycles.saturating_add(1u8);
                if self.nv_config.aging_cycles == self.cal_config.aging_threshold {
                    self.nv_config.uds_status.set_cdtc(false);
                    self.nv_config.confirmation_cycles = 0u8;
                }
            }
        }
        if self.nv_config.uds_status.tftoc() {
            self.nv_config.confirmation_cycles =
                self.nv_config.confirmation_cycles.saturating_add(1u8);
            if self.nv_config.confirmation_cycles == self.cal_config.confirmation_threshold {
                self.nv_config.uds_status.set_cdtc(true);
                self.nv_config.aging_cycles = 0u8;
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
        self.nv_config.occurence_cntr = 0u8;
        self.nv_config.confirmation_cycles = 0u8;
        self.nv_config.aging_cycles = 0u8;
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
        if self.uds_status_old.raw() & UdsStatusByte::TF_BIT != UdsStatusByte::TF_BIT {
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
    ) -> &'static CalibConfig {
        Box::leak(Box::new(CalibConfig {
            step_up,
            step_down,
            debounce_behavior,
            debounce_type,
            confirmation_threshold: confirmation_thr,
            aging_threshold: aging_thr,
            priority,
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let evt = Event::new(0, n_cfg, c_cfg);
        assert_eq!(evt.cal_config.confirmation_threshold, 4u8);
        assert_eq!(evt.cal_config.aging_threshold, 6u8);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
        evt.step(Status::Failed, true, 0.0).unwrap();
        assert_eq!(evt.debounce_counter(), i16::MAX);
        assert!(evt.status().tf());
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg1 = create_nvm_config(0, 0, 0, false, true, false);
        let n_cfg2 = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt1 = Event::new(0, n_cfg1, c_cfg);
        evt1.step(Status::PreFailed, true, 10.0).unwrap();
        let counter_10ms = evt1.debounce_counter();

        let mut evt2 = Event::new(0, n_cfg2, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, false, true);

        let status = {
            let mut evt = Event::new(0, n_cfg, c_cfg);
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
    fn stop_not_failed_aging_completes_clears_cdtc() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
        );
        let n_cfg = create_nvm_config(0, 4, 0, false, false, true);

        let status = {
            let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, false, false);

        let status = {
            let mut evt = Event::new(0, n_cfg, c_cfg);
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
    fn stop_failed_sets_cdtc() {
        let c_cfg = create_cal_config(
            1,
            0,
            3,
            5,
            DebounceType::CounterBased,
            DebounceBehavior::Freeze,
            0,
        );
        let n_cfg = create_nvm_config(0, 0, 2, true, false, false);

        let status = {
            let mut evt = Event::new(0, n_cfg, c_cfg);
            assert!(evt.status().tftoc());
            assert!(!evt.status().tnctoc());
            assert!(!evt.status().cdtc());
            evt.stop();
            evt.status()
        };

        assert!(status.cdtc());
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, false, true);

        let mut evt = Event::new(0, n_cfg, c_cfg);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let event = Event::new(0, n_cfg, c_cfg);
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager::new(events, ext_list, UdsStatusByte::TF_BIT);

        let result = manager.step(0, Status::PreFailed, true, 0.0);
        assert!(result.is_ok());
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let event = Event::new(0, n_cfg, c_cfg);
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager::new(events, ext_list, UdsStatusByte::TF_BIT);

        let result = manager.step(99, Status::PreFailed, true, 0.0);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let event = Event::new(0, n_cfg, c_cfg);
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager::new(events, ext_list, UdsStatusByte::TF_BIT);

        let result = manager.step(0, Status::PreFailed, true, -1.0);
        assert_eq!(result.unwrap_err(), EventManagerError::EventStepError);
    }

    #[test]
    fn event_manager_rising_edge_creates_extended_record() {
        let c_cfg = create_cal_config(
            0,
            0,
            3,
            5,
            DebounceType::TimeBased,
            DebounceBehavior::Freeze,
            0,
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let event = Event::new(42, n_cfg, c_cfg);
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager::new(events, ext_list, UdsStatusByte::TF_BIT);

        manager.step(0, Status::Failed, true, 1.0).unwrap();

        let ext_rec = manager.extended_records.get_by_event_id(42);
        assert!(ext_rec.is_some());
        assert_eq!(ext_rec.unwrap().event_id, 42);
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
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let event = Event::new(42, n_cfg, c_cfg);
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager::new(events, ext_list, UdsStatusByte::TF_BIT);

        manager.step(0, Status::Passed, true, 1.0).unwrap();

        assert!(manager.extended_records.is_empty());
    }

    #[test]
    fn event_manager_extended_record_updated_on_reinsert() {
        let c_cfg = create_cal_config(
            0,
            0,
            3,
            5,
            DebounceType::TimeBased,
            DebounceBehavior::Freeze,
            0,
        );
        let n_cfg = create_nvm_config(0, 0, 0, false, true, false);

        let event = Event::new(42, n_cfg, c_cfg);
        let events = Box::leak(Box::new([event]));
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));

        let mut manager = EventManager::new(events, ext_list, UdsStatusByte::TF_BIT);

        manager.step(0, Status::Failed, true, 1.0).unwrap();
        let first_rec = manager.extended_records.get_by_event_id(42).unwrap();
        let first_date = first_rec.date_at_last_save;

        std::thread::sleep(std::time::Duration::from_millis(10));

        manager.step(0, Status::Failed, true, 1.0).unwrap();
        let second_rec = manager.extended_records.get_by_event_id(42).unwrap();

        assert!(second_rec.date_at_last_save > first_date);
        assert_eq!(second_rec.event_id, 42);
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
                DebounceType::TimeBased,
                DebounceBehavior::Freeze,
                priority,
            );
            let n_cfg = create_nvm_config(0, 0, 0, false, true, false);
            events_vec.push(Event::new(i as EventId, n_cfg, c_cfg));
        }

        let events = Box::leak(events_vec.into_boxed_slice());
        let ext_list = Box::leak(Box::new(ExtendedRecordList::new()));
        let mut manager = EventManager::new(events, ext_list, UdsStatusByte::TF_BIT);

        for i in 0..=24 {
            let result = manager.step(i as EventId, Status::Failed, true, 1.0);
            if result.is_err() {
                panic!("step({}) failed: {:?}", i, result.unwrap_err());
            }
        }

        assert!(manager.extended_records.is_full());
        assert_eq!(manager.extended_records.len(), 24);

        assert!(manager.extended_records.get_by_event_id(0).is_none());
        assert!(manager.extended_records.get_by_event_id(24).is_some());
    }

    #[test]
    fn extended_record_list_insert_full_returns_error() {
        let mut list = ExtendedRecordList::new();

        for i in 0..24 {
            let ext_rec = ExtendedRecord {
                event_id: i,
                priority: 10 + i as u8,
                date_at_first_save: None,
                date_at_last_save: None,
            };
            list.insert(ext_rec).unwrap();
        }

        assert!(list.is_full());
        assert_eq!(list.len(), 24);

        let ext_rec = ExtendedRecord {
            event_id: 99,
            priority: 100,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        let result = list.insert(ext_rec);
        assert_eq!(result.unwrap_err(), EventManagerError::ListFullError);

        assert_eq!(list.len(), 24);
        assert!(list.get_by_event_id(0).is_some());
        assert!(list.get_by_event_id(23).is_some());
        assert!(list.get_by_event_id(99).is_none());
    }

    #[test]
    fn extended_record_list_remove_out_of_bounds_returns_none() {
        let mut list = ExtendedRecordList::new();

        let result = list.remove(0);
        assert!(result.is_none());

        let ext_rec = ExtendedRecord {
            event_id: 1,
            priority: 5,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        list.insert(ext_rec).unwrap();

        let result = list.remove(5);
        assert!(result.is_none());

        let result = list.remove(100);
        assert!(result.is_none());
    }

    #[test]
    fn extended_record_list_remove_by_priority() {
        let mut list = ExtendedRecordList::new();

        let ext_rec1 = ExtendedRecord {
            event_id: 1,
            priority: 5,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        let ext_rec2 = ExtendedRecord {
            event_id: 2,
            priority: 10,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        let ext_rec3 = ExtendedRecord {
            event_id: 3,
            priority: 15,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        list.insert(ext_rec1).unwrap();
        list.insert(ext_rec2).unwrap();
        list.insert(ext_rec3).unwrap();

        assert_eq!(list.len(), 3);

        let removed = list.remove_by_priority(10).unwrap();
        assert_eq!(removed.event_id, 2);
        assert_eq!(list.len(), 2);
        assert!(list.get_by_event_id(2).is_none());
        assert!(list.get_by_event_id(1).is_some());
        assert!(list.get_by_event_id(3).is_some());

        let result = list.remove_by_priority(200);
        assert!(result.is_none());
    }

    #[test]
    fn extended_record_list_find_lowest_priority() {
        let mut list = ExtendedRecordList::new();

        let ext_rec1 = ExtendedRecord {
            event_id: 1,
            priority: 10,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        let ext_rec2 = ExtendedRecord {
            event_id: 2,
            priority: 5,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        let ext_rec3 = ExtendedRecord {
            event_id: 3,
            priority: 15,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        list.insert(ext_rec1).unwrap();
        list.insert(ext_rec2).unwrap();
        list.insert(ext_rec3).unwrap();

        let result = list.find_lowest_priority();
        assert_eq!(result.priority, 5);
        assert_eq!(result.index, 0);
        assert_eq!(list.iter().nth(result.index).unwrap().event_id, 2);
    }
}
