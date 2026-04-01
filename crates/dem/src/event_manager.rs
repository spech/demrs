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
    ) -> CalibConfig {
        CalibConfig {
            step_up,
            step_down,
            debounce_behavior,
            debounce_type,
            confirmation_threshold: confirmation_thr,
            aging_threshold: aging_thr,
            priority,
            save_trigger,
        }
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
        cal_config: CalibConfig,
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
    // EventManager Tests
    // ────────────────────────────────────────────

    #[test]
    fn fn_step_success() {
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
        let result = manager.step(0, crate::Status::PreFailed, true, 0.0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn fn_init_calls_all_events() {
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
    fn fn_stop_calls_all_events() {
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
    fn fn_free_from_extended_records() {
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
        manager
            .step(0, crate::Status::Failed, true, 0.0, 100)
            .unwrap();
        manager
            .step(1, crate::Status::Failed, true, 0.0, 100)
            .unwrap();

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
    fn fn_step_invalid_id_returns_error() {
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
        let result = manager.step(99, crate::Status::PreFailed, true, 0.0, 0);
        assert_eq!(result.unwrap_err(), EventManagerError::InvalidEventIdError);
    }

    #[test]
    fn fn_step_event_error_propagates() {
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
        let result = manager.step(0, crate::Status::PreFailed, true, -1.0, 0);
        assert_eq!(result.unwrap_err(), EventManagerError::EventStepError);
    }

    #[test]
    fn fn_step_returns_error_when_state_off() {
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

        let result = manager.step(0, crate::Status::Failed, true, 0.0, 100);
        assert_eq!(result.unwrap_err(), EventManagerError::NotInitializedError);
    }

    #[test]
    fn fn_step_succeeds_after_init() {
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
        let result = manager.step(0, crate::Status::Failed, true, 0.0, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn fn_rising_edge_creates_extended_record() {
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
        manager
            .step(0, crate::Status::Failed, true, 0.0, 0)
            .unwrap();

        let ext_rec = manager.extended_records.get_by_event_id(0);
        assert!(ext_rec.is_some());
        assert_eq!(ext_rec.unwrap().event_id, 0);
    }

    #[test]
    fn fn_no_rising_edge_no_extended_record() {
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
        manager
            .step(0, crate::Status::Passed, true, 1.0, 0)
            .unwrap();

        assert!(manager.extended_records.is_empty());
    }

    #[test]
    fn fn_extended_record_no_update_without_rising_edge() {
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

        manager
            .step(0, crate::Status::Failed, true, 0.0, 100)
            .unwrap();
        let first_rec = manager.extended_records.get_by_event_id(0).unwrap();
        let first_date = first_rec.date_at_last_save;

        manager
            .step(0, crate::Status::Failed, true, 0.0, 200)
            .unwrap();
        let second_rec = manager.extended_records.get_by_event_id(0).unwrap();

        assert_eq!(second_rec.date_at_last_save, first_date);
        assert_eq!(second_rec.event_id, 0);
    }

    #[test]
    fn fn_extended_record_reinsert_on_rising_edge_after_stop() {
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
        manager
            .step(0, crate::Status::Failed, true, 0.0, 100)
            .unwrap();
        let first_date = manager
            .extended_records
            .get_by_event_id(0)
            .unwrap()
            .date_at_last_save;

        manager.stop();
        manager.init();

        manager
            .step(0, crate::Status::Failed, true, 0.0, 200)
            .unwrap();
        let second_date = manager
            .extended_records
            .get_by_event_id(0)
            .unwrap()
            .date_at_last_save;

        assert_eq!(second_date, 200);
        assert_ne!(first_date, second_date);
    }

    #[test]
    fn fn_extended_records_list_full_replaces_lowest_priority() {
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
            let result = manager.step(i as EventId, crate::Status::Failed, true, 0.0, i as u32);
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
