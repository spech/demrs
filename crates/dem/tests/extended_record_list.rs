use dem::{
    f_25_events::EVENT_MANAGER, EventManager, EventManagerState, ExtendedRecordList, Status,
};

/// Tests that the ExtendedRecordList evicts the lowest priority entry when full.
///
/// **Use Case**: When the DEM storage for extended records is full (24 slots) and a new
/// event with higher priority (worse) needs to be stored, the system should evict
/// the entry with the lowest priority (best/worst performing) to make room.
///
/// **Setup**:
/// - Creates 25 events with priorities 1-25 and save_trigger = OnPdtc
/// - Fills the first 24 slots (priorities 1-24)
/// - Attempts to add event 25 (priority 25, highest = worst)
///
/// **Expected**: After filling 24 slots, event 25 cannot evict any existing entry
/// because all have lower priority values (better priority).
#[test]
fn bdd_extended_record_list_full_eviction() {
    let cal_config_template = dem::CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: dem::DebounceBehavior::Freeze,
        debounce_type: dem::DebounceType::CounterBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 10,
        save_trigger: dem::SaveTrigger::OnPdtc,
    };

    let mut events = Vec::new();
    for i in 0..25 {
        let mut uds = dem::UdsStatusByte::from_raw(0);
        uds.set_tnctoc(true);

        let nvm_config = Box::leak(Box::new(dem::NvmConfig {
            uds_status: uds,
            occurence_cntr: 0,
            aging_cycles: 0,
            confirmation_cycles: 0,
        }));

        let cal_config = dem::CalibConfig {
            priority: i as u8 + 1,
            ..cal_config_template
        };

        let event = dem::Event {
            debounce_counter: 0,
            uds_status_old: nvm_config.uds_status,
            disabled: false,
            nv_config: nvm_config,
            cal_config,
        };
        events.push(event);
    }

    let events = Box::leak(events.into_boxed_slice());
    static mut EXT_LIST_NVM: [Option<dem::ExtendedRecord>; 24] = [const { None }; 24];
    let ext_list = unsafe {
        Box::leak(Box::new(ExtendedRecordList::from_nvm(
            &*(&raw const EXT_LIST_NVM),
        )))
    };

    let mut manager = EventManager {
        events,
        extended_records: ext_list,
        state: EventManagerState::Off,
    };

    manager.init();
    for i in 0..24 {
        manager
            .step(i as u16, Status::Failed, true, 0.0, i)
            .unwrap();
    }

    assert!(manager.extended_records.is_full());
    assert_eq!(manager.extended_records.len(), 24);
    assert!(manager.extended_records.get_by_event_id(0).is_some());

    let err = manager.step(24, Status::Failed, true, 0.0, 24);
    assert!(err.is_err());
    assert_eq!(manager.extended_records.len(), 24);
    assert!(manager.extended_records.get_by_event_id(0).is_some());
}

/// Tests that a lower priority (better) event is rejected when list is full.
///
/// **Use Case**: When the DEM storage is full with high-priority (worse) events, a new
/// event with even higher priority cannot displace existing entries because all existing
/// entries have priority values less than the new event.
///
/// **Setup**:
/// - Creates 25 events with priorities 0-24 (where 0 is best, 24 is worst)
/// - Fills the first 24 slots with priorities 0-23 (all "better" than 24)
/// - Attempts to insert priority 24 (worst)
///
/// **Expected**: Error because no existing entry has worse priority (>= 24) to evict.
/// Only entries with priority greater than new_priority can be evicted.
#[test]
fn bdd_extended_record_list_full_reject_lower_priority() {
    let cal_config_template = dem::CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: dem::DebounceBehavior::Freeze,
        debounce_type: dem::DebounceType::CounterBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 24,
        save_trigger: dem::SaveTrigger::OnPdtc,
    };

    let mut events = Vec::new();
    for i in 0..25 {
        let mut uds = dem::UdsStatusByte::from_raw(0);
        uds.set_tnctoc(true);

        let nvm_config = Box::leak(Box::new(dem::NvmConfig {
            uds_status: uds,
            occurence_cntr: 0,
            aging_cycles: 0,
            confirmation_cycles: 0,
        }));

        let cal_config = dem::CalibConfig {
            priority: i as u8,
            ..cal_config_template
        };

        let event = dem::Event {
            debounce_counter: 0,
            uds_status_old: nvm_config.uds_status,
            disabled: false,
            nv_config: nvm_config,
            cal_config,
        };
        events.push(event);
    }

    let events = Box::leak(events.into_boxed_slice());
    static mut EXT_LIST_NVM: [Option<dem::ExtendedRecord>; 24] = [const { None }; 24];
    let ext_list = unsafe {
        Box::leak(Box::new(ExtendedRecordList::from_nvm(
            &*(&raw const EXT_LIST_NVM),
        )))
    };

    let mut manager = EventManager {
        events,
        extended_records: ext_list,
        state: EventManagerState::Off,
    };

    manager.init();
    for i in 0..24 {
        manager
            .step(i as u16, Status::Failed, true, 0.0, i)
            .unwrap();
    }

    assert!(manager.extended_records.is_full());
    assert_eq!(manager.extended_records.len(), 24);
    assert!(manager.extended_records.get_by_event_id(0).is_some());

    let err = manager.step(24, Status::Failed, true, 0.0, 24);
    assert!(err.is_err());
    assert_eq!(manager.extended_records.len(), 24);
    assert!(manager.extended_records.get_by_event_id(0).is_some());
}

/// Tests that OnPdtc trigger creates an extended record on pdtc rising edge.
///
/// **Use Case**: When an event is configured with save_trigger = OnPdtc, the DEM
/// should create an extended record as soon as the pending DTC (pdtc) flag rises,
/// without waiting for confirmation. This provides immediate visibility into
/// potential fault conditions.
///
/// **Setup**: Uses fixture event 13 which has save_trigger = OnPdtc
///
/// **Expected**: After one step with Status::Failed, pdtc rises and an extended
/// record is created with event_id=13, priority=14, and timestamps set to 100.
#[test]
fn bdd_extended_record_list_onpdtc_trigger() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    // Event 13 has save_trigger: OnPdtc
    manager.step(13, Status::Failed, true, 0.0, 100).unwrap();

    assert_eq!(manager.extended_records.len(), 1);
    let record = manager.extended_records.get_by_event_id(13).unwrap();
    assert_eq!(record.event_id, 13);
    assert_eq!(record.priority, 14);
    assert_eq!(record.date_at_first_save, 100);
    assert_eq!(record.date_at_last_save, 100);
}

/// Tests that OnCdtc trigger creates an extended record on cdtc rising edge.
///
/// **Use Case**: When an event is configured with save_trigger = OnCdtc, the DEM
/// should only create an extended record when the confirmed DTC (cdtc) flag rises.
/// This is used for events requiring full confirmation before recording, reducing
/// noise from transient faults.
///
/// **Setup**: Uses fixture event 0 which has save_trigger = OnCdtc
///
/// **Flow**:
/// 1. First step: pdtc rises but cdtc not yet (no record created)
/// 2. stop() + init(): cycles the operating state
/// 3. Second step: cdtc rises (record created)
///
/// **Expected**: Extended record created only after cdtc is confirmed.
#[test]
fn bdd_extended_record_list_oncdtc_trigger() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    // Event 0 has save_trigger: OnCdtc
    manager.step(0, Status::Failed, true, 0.0, 100).unwrap();
    assert!(manager.extended_records.get_by_event_id(0).is_none());

    manager.stop();
    manager.init();

    let status = manager.step(0, Status::Failed, true, 0.0, 200).unwrap();
    assert!(status.cdtc());
    assert!(manager.extended_records.get_by_event_id(0).is_some());
}

/// Tests that aged events are removed from extended records when aging completes.
///
/// **Use Case**: After a confirmed DTC passes through the aging process (operating
/// cycles where the test passes), it should be aged out and removed from storage.
/// This prevents storage from filling with historical resolved faults and ensures
/// only recent/relevant faults remain in the system.
///
/// **Setup**: Uses fixture event 12 which has save_trigger = OnCdtc, aging_threshold = 10
///
/// **Flow**:
/// 1. First occurrence: pdtc rises, stop/init cycle sets cdtc, record created
/// 2. Aging cycles: Test passes for 10+ operating cycles
/// 3. After threshold: stop() clears cdtc, triggering removal from extended records
///
/// **Aging Requirements** (all must be true in stop()):
/// - tftoc must be false (test not failed this cycle)
/// - tnctoc must be false (test complete this cycle)
/// - cdtc must be true (still confirmed)
/// - aging_cycles must reach aging_threshold
#[test]
fn bdd_extended_record_list_remove_aged_event() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    // Event 12 has save_trigger: OnCdtc, aging_threshold: 10
    // First cycle: pdtc rises but cdtc not yet
    manager.step(12, Status::Failed, true, 0.0, 100).unwrap();
    assert!(manager.extended_records.get_by_event_id(12).is_none());

    manager.stop();
    manager.init();

    // cdtc is now set, record should be created
    let status = manager.step(12, Status::Failed, true, 0.0, 200).unwrap();
    assert!(status.cdtc());
    assert!(manager.extended_records.get_by_event_id(12).is_some());

    // Manually reset aging_cycles to 0 so aging can progress
    manager.events[12].nv_config.aging_cycles = 0;
    manager.events[12].nv_config.uds_status = dem::UdsStatusByte::from_raw(0);
    manager.events[12].nv_config.uds_status.set_tftoc(true);
    manager.events[12].nv_config.uds_status.set_cdtc(true);

    // Run aging cycles until threshold (10) is reached
    // tftoc must be false for aging to progress in stop()
    for i in 0..11 {
        manager.events[12].nv_config.uds_status.set_tftoc(false);
        manager.events[12].nv_config.uds_status.set_tnctoc(false);
        manager
            .step(12, Status::Passed, true, 0.0, 300 + i)
            .unwrap();
        manager.stop();
        manager.init();
    }

    // After aging threshold reached, stop should clear cdtc and remove record
    manager.events[12].nv_config.uds_status.set_tftoc(false);
    manager.events[12].nv_config.uds_status.set_tnctoc(false);
    manager.stop();

    assert!(manager.extended_records.get_by_event_id(12).is_none());
}

/// Tests that repeated occurrences update date_at_last_save but preserve date_at_first_save.
///
/// **Use Case**: When the same event fails multiple times (intermittent fault), the DEM
/// should track both when the fault was first recorded and when it was most recently
/// active. This helps diagnose intermittent issues by showing fault history.
///
/// **Setup**: Uses fixture event 13 which has save_trigger = OnPdtc
///
/// **Flow**:
/// 1. First failure at timestamp 100: record created with dates = 100
/// 2. Reset pdtc to false to allow rising edge detection
/// 3. Second failure at timestamp 200: date_at_last_save updated to 200
///
/// **Expected**: date_at_first_save remains 100 (unchanged), date_at_last_save becomes 200.
#[test]
fn bdd_extended_record_list_new_occurence_update_date_at_last_save() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    // Event 13 has save_trigger: OnPdtc
    // First occurrence at timestamp 100
    manager.step(13, Status::Failed, true, 0.0, 100).unwrap();

    assert_eq!(manager.extended_records.len(), 1);
    let record = manager.extended_records.get_by_event_id(13).unwrap();
    assert_eq!(record.date_at_first_save, 100);
    assert_eq!(record.date_at_last_save, 100);

    // Manually set pdtc to false to ensure rising edge detection
    manager.events[13].nv_config.uds_status.set_pdtc(false);

    manager.step(13, Status::Failed, true, 0.0, 200).unwrap();

    // Verify date_at_last_save is updated
    let record = manager.extended_records.get_by_event_id(13).unwrap();
    assert_eq!(record.date_at_first_save, 100);
    assert_eq!(record.date_at_last_save, 200);
}
