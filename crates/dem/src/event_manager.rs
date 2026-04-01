// ─────────────────────────────────────────────
// EventManager
// ─────────────────────────────────────────────

#[allow(unused_imports)]
use crate::event::{Event, NvmConfig};
#[allow(unused_imports)]
use crate::event_config::{CalibConfig, DebounceBehavior, DebounceType, SaveTrigger};
use crate::extended_record::{
    EventId, ExtendedRecord, ExtendedRecordList, ExtendedRecordListError,
};
use crate::UdsStatusByte;

/// Runtime state of the EventManager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventManagerState {
    On,
    Off,
}

/// Error type for [`EventManager`] operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventManagerError {
    /// The provided event ID is out of bounds.
    InvalidEventIdError,
    /// The event step operation failed.
    EventStepError,
    /// The EventManager is not initialized (state is Off).
    NotInitializedError,
    /// An error occurred while updating the extended records storage.
    ExtendedRecordError(ExtendedRecordListError),
}

impl From<ExtendedRecordListError> for EventManagerError {
    fn from(err: ExtendedRecordListError) -> Self {
        EventManagerError::ExtendedRecordError(err)
    }
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
    /// Sets the state to [`EventManagerState::On`].
    pub fn init(&mut self) {
        for event in self.events.iter_mut() {
            event.init();
        }
        self.state = EventManagerState::On;
    }

    /// Stops all managed events at shutdown.
    ///
    /// Sets the state to [`EventManagerState::Off`].
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
    /// Returns [`EventManagerError::NotInitializedError`] if the state is [`EventManagerState::Off`].
    pub fn step(
        &mut self,
        event_id: EventId,
        condition: crate::Status,
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


