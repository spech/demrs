// ─────────────────────────────────────────────
// EventManager
// ─────────────────────────────────────────────

#[allow(unused_imports)]
use crate::event::{Event, NvmConfig};
#[allow(unused_imports)]
use crate::event_config::{
    CalibConfig, DebounceBehavior, DebounceType, SaveTrigger, SnapshotConfig, SNAPSHOT_DATA_SIZE,
};
use crate::freeze_frame::{EventId, FreezeFrame, FreezeFrameList, FreezeFrameListError};
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
    /// An error occurred while updating the freeze frames storage.
    FreezeFrameError(FreezeFrameListError),
}

impl From<FreezeFrameListError> for EventManagerError {
    fn from(err: FreezeFrameListError) -> Self {
        EventManagerError::FreezeFrameError(err)
    }
}

/// Manages a collection of [`Event`]s and their associated [`FreezeFrame`] data.
///
/// The `EventManager` coordinates debouncing logic and persistent storage
/// for diagnostic event handling. It holds references to static event data
/// and freeze frames that persist across power cycles.
///
/// ## Storage
///
/// - `events` - Slice of [`Event`] instances with fixed static addresses
/// - `freeze_frames` - Reference to [`FreezeFrameList`] for persistent metadata
/// - `snapshot_config` - Reference to [`SnapshotConfig`] for snapshot data sources
///
/// ## Usage
///
/// Create an `EventManager` by passing static references to event data
/// and freeze frames storage.
pub struct EventManager {
    /// Slice of [`Event`] instances at fixed static addresses.
    pub events: &'static mut [Event],
    /// Reference to [`FreezeFrameList`] for persistent metadata.
    pub freeze_frames: &'static mut FreezeFrameList,
    /// Reference to [`SnapshotConfig`] for snapshot data sources.
    pub snapshot_config: &'static SnapshotConfig,
    /// Runtime state of the EventManager.
    pub state: EventManagerState,
}

impl EventManager {
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
    /// If aging threshold is reached, the corresponding freeze frame is freed.
    pub fn stop(&mut self) {
        self.state = EventManagerState::Off;
        let len = self.events.len();
        for index in 0..len {
            let new_status = self.events[index].stop();

            if !new_status.tftoc()
                && !new_status.tnctoc()
                && new_status.cdtc()
                && self.events[index].nv_config.aging_cycles
                    >= self.events[index].cal_config.aging_threshold
            {
                let event_id = index as EventId;
                self.free_from_freeze_frames(event_id);
            }
        }
    }

    /// Clears fault memory by calling `clear()` on all events and removing all freeze frames.
    ///
    /// This is typically called in response to a "Clear DTC" request (e.g., OBD service $04).
    pub fn clear(&mut self) {
        for event in self.events.iter_mut() {
            event.clear();
        }
        while !self.freeze_frames.is_empty() {
            self.freeze_frames.remove(0);
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

        self.store_in_freeze_frames(index, timestamp)?;

        Ok(new_status)
    }

    fn auto_capture_snapshot(&self, buffer: &mut [u8; SNAPSHOT_DATA_SIZE]) {
        let mut offset = 0usize;
        for i in 0..self.snapshot_config.count as usize {
            let src = &self.snapshot_config.sources[i];
            if src.size > 0 {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        src.address,
                        buffer.as_mut_ptr().add(offset),
                        src.size as usize,
                    );
                }
                offset += src.size as usize;
            }
        }
    }

    fn store_in_freeze_frames(
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
            SaveTrigger::OnPdtc => new_status.pdtc() && !old_status.pdtc(),
            SaveTrigger::OnCdtc => new_status.cdtc() && !old_status.cdtc(),
            SaveTrigger::OnTf => new_status.tf() && !old_status.tf(),
            SaveTrigger::OnTftoc => new_status.tftoc() && !old_status.tftoc(),
        };

        if rising_edge {
            let mut snapshot = [0u8; SNAPSHOT_DATA_SIZE];
            self.auto_capture_snapshot(&mut snapshot);

            if let Some(existing) = self.freeze_frames.get_by_event_id_mut(event_id) {
                if event.cal_config.record_update {
                    existing.last_occurrence_time = timestamp;
                    existing.snapshot_data = snapshot;
                }
            } else {
                let freeze_frame = FreezeFrame {
                    event_id,
                    priority,
                    first_occurrence_time: timestamp,
                    last_occurrence_time: timestamp,
                    snapshot_data: snapshot,
                };
                self.freeze_frames.insert(freeze_frame)?;
            }
        }

        Ok(())
    }

    /// Frees (removes) an entry from the freeze frames list by event ID.
    ///
    /// If an entry with the given event ID exists, it is removed.
    /// If no entry exists with that event ID, this function does nothing.
    ///
    /// # Arguments
    ///
    /// * `event_id` - The event ID of the entry to free.
    pub fn free_from_freeze_frames(&mut self, event_id: EventId) {
        let index_to_remove = self
            .freeze_frames
            .iter()
            .position(|rec| rec.event_id == event_id);

        if let Some(index) = index_to_remove {
            self.freeze_frames.remove(index);
        }
    }
}
