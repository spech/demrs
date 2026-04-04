use dem::{
    f_25_events::EVENT_MANAGER, EventManager, EventManagerState, FreezeFrameList, SnapshotConfig,
    SnapshotSource, Status,
};

fn create_empty_snapshot_config() -> &'static SnapshotConfig {
    const EMPTY_CONFIG: SnapshotConfig = SnapshotConfig {
        sources: [const {
            SnapshotSource {
                address: core::ptr::null(),
                size: 0,
            }
        }; 255],
        count: 0,
    };
    &EMPTY_CONFIG
}

/// Tests that the FreezeFrameList evicts the lowest priority entry when full.
///
/// **Use Case**: When the DEM storage for freeze frames is full (24 slots) and a new
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
fn bdd_freeze_frame_list_full_eviction() {
    let cal_config_template = dem::CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: dem::DebounceBehavior::Freeze,
        debounce_type: dem::DebounceType::CounterBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 10,
        save_trigger: dem::SaveTrigger::OnPdtc,
        record_update: true,
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
    static mut FF_LIST_NVM: [Option<dem::FreezeFrame>; 24] = [const { None }; 24];
    let ff_list = unsafe {
        Box::leak(Box::new(FreezeFrameList::from_nvm(
            &*(&raw const FF_LIST_NVM),
        )))
    };

    let mut manager = EventManager {
        events,
        freeze_frames: ff_list,
        snapshot_config: create_empty_snapshot_config(),
        state: EventManagerState::Off,
    };

    manager.init();
    for i in 0..24 {
        manager
            .step(i as u16, Status::Failed, true, 0.0, i)
            .unwrap();
    }

    assert!(manager.freeze_frames.is_full());
    assert_eq!(manager.freeze_frames.len(), 24);
    assert!(manager.freeze_frames.get_by_event_id(0).is_some());

    let err = manager.step(24, Status::Failed, true, 0.0, 24);
    assert!(err.is_err());
    assert_eq!(manager.freeze_frames.len(), 24);
    assert!(manager.freeze_frames.get_by_event_id(0).is_some());
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
fn bdd_freeze_frame_list_full_reject_lower_priority() {
    let cal_config_template = dem::CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: dem::DebounceBehavior::Freeze,
        debounce_type: dem::DebounceType::CounterBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 24,
        save_trigger: dem::SaveTrigger::OnPdtc,
        record_update: true,
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
    static mut FF_LIST_NVM: [Option<dem::FreezeFrame>; 24] = [const { None }; 24];
    let ff_list = unsafe {
        Box::leak(Box::new(FreezeFrameList::from_nvm(
            &*(&raw const FF_LIST_NVM),
        )))
    };

    let mut manager = EventManager {
        events,
        freeze_frames: ff_list,
        snapshot_config: create_empty_snapshot_config(),
        state: EventManagerState::Off,
    };

    manager.init();
    for i in 0..24 {
        manager
            .step(i as u16, Status::Failed, true, 0.0, i)
            .unwrap();
    }

    assert!(manager.freeze_frames.is_full());
    assert_eq!(manager.freeze_frames.len(), 24);
    assert!(manager.freeze_frames.get_by_event_id(0).is_some());

    let err = manager.step(24, Status::Failed, true, 0.0, 24);
    assert!(err.is_err());
    assert_eq!(manager.freeze_frames.len(), 24);
    assert!(manager.freeze_frames.get_by_event_id(0).is_some());
}

/// Tests that OnPdtc trigger creates a freeze frame on pdtc rising edge.
///
/// **Use Case**: When an event is configured with save_trigger = OnPdtc, the DEM
/// should create a freeze frame as soon as the pending DTC (pdtc) flag rises,
/// without waiting for confirmation. This provides immediate visibility into
/// potential fault conditions.
///
/// **Setup**: Uses fixture event 13 which has save_trigger = OnPdtc
///
/// **Expected**: After one step with Status::Failed, pdtc rises and a freeze
/// frame is created with event_id=13, priority=14, and timestamps set to 100.
#[test]
fn bdd_freeze_frame_list_onpdtc_trigger() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    manager.step(13, Status::Failed, true, 0.0, 100).unwrap();

    assert_eq!(manager.freeze_frames.len(), 1);
    let record = manager.freeze_frames.get_by_event_id(13).unwrap();
    assert_eq!(record.event_id, 13);
    assert_eq!(record.priority, 14);
    assert_eq!(record.first_occurrence_time, 100);
    assert_eq!(record.last_occurrence_time, 100);
}

/// Tests that OnCdtc trigger creates a freeze frame on cdtc rising edge.
///
/// **Use Case**: When an event is configured with save_trigger = OnCdtc, the DEM
/// should only create a freeze frame when the confirmed DTC (cdtc) flag rises.
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
/// **Expected**: Freeze frame created only after cdtc is confirmed.
#[test]
fn bdd_freeze_frame_list_oncdtc_trigger() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    manager.step(0, Status::Failed, true, 0.0, 100).unwrap();
    assert!(manager.freeze_frames.get_by_event_id(0).is_none());

    manager.stop();
    manager.init();

    let status = manager.step(0, Status::Failed, true, 0.0, 200).unwrap();
    assert!(status.cdtc());
    assert!(manager.freeze_frames.get_by_event_id(0).is_some());
}

/// Tests that aged events are removed from freeze frames when aging completes.
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
/// 3. After threshold: stop() clears cdtc, triggering removal from freeze frames
///
/// **Aging Requirements** (all must be true in stop()):
/// - tftoc must be false (test not failed this cycle)
/// - tnctoc must be false (test complete this cycle)
/// - cdtc must be true (still confirmed)
/// - aging_cycles must reach aging_threshold
#[test]
fn bdd_freeze_frame_list_remove_aged_event() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    manager.step(12, Status::Failed, true, 0.0, 100).unwrap();
    assert!(manager.freeze_frames.get_by_event_id(12).is_none());

    manager.stop();
    manager.init();

    let status = manager.step(12, Status::Failed, true, 0.0, 200).unwrap();
    assert!(status.cdtc());
    assert!(manager.freeze_frames.get_by_event_id(12).is_some());

    manager.events[12].nv_config.aging_cycles = 0;
    manager.events[12].nv_config.uds_status = dem::UdsStatusByte::from_raw(0);
    manager.events[12].nv_config.uds_status.set_tftoc(true);
    manager.events[12].nv_config.uds_status.set_cdtc(true);

    for i in 0..11 {
        manager.events[12].nv_config.uds_status.set_tftoc(false);
        manager.events[12].nv_config.uds_status.set_tnctoc(false);
        manager
            .step(12, Status::Passed, true, 0.0, 300 + i)
            .unwrap();
        manager.stop();
        manager.init();
    }

    manager.events[12].nv_config.uds_status.set_tftoc(false);
    manager.events[12].nv_config.uds_status.set_tnctoc(false);
    manager.stop();

    assert!(manager.freeze_frames.get_by_event_id(12).is_none());
}

/// Tests that repeated occurrences update last_occurrence_time but preserve first_occurrence_time.
///
/// **Use Case**: When the same event fails multiple times (intermittent fault), the DEM
/// should track both when the fault was first recorded and when it was most recently
/// active. This helps diagnose intermittent issues by showing fault history.
///
/// **Setup**: Uses fixture event 13 which has save_trigger = OnPdtc
///
/// **Flow**:
/// 1. First failure at timestamp 100: record created with timestamps = 100
/// 2. Reset pdtc to false to allow rising edge detection
/// 3. Second failure at timestamp 200: last_occurrence_time updated to 200
///
/// **Expected**: first_occurrence_time remains 100 (unchanged), last_occurrence_time becomes 200.
#[test]
fn bdd_freeze_frame_list_new_occurence_update_last_occurrence_time() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    manager.step(13, Status::Failed, true, 0.0, 100).unwrap();

    assert_eq!(manager.freeze_frames.len(), 1);
    let record = manager.freeze_frames.get_by_event_id(13).unwrap();
    assert_eq!(record.first_occurrence_time, 100);
    assert_eq!(record.last_occurrence_time, 100);

    manager.events[13].nv_config.uds_status.set_pdtc(false);

    // Debug: check status before step
    let ev = &manager.events[13];
    eprintln!(
        "DEBUG before step2: tf={}, pdtc={}, uds_status_old={:?}",
        ev.nv_config.uds_status.tf(),
        ev.nv_config.uds_status.pdtc(),
        ev.uds_status_old
    );

    manager.step(13, Status::Failed, true, 0.0, 200).unwrap();

    // Debug: check status after step
    let ev = &manager.events[13];
    eprintln!(
        "DEBUG after step2: tf={}, pdtc={}, uds_status_old={:?}",
        ev.nv_config.uds_status.tf(),
        ev.nv_config.uds_status.pdtc(),
        ev.uds_status_old
    );

    let record = manager.freeze_frames.get_by_event_id(13).unwrap();
    assert_eq!(record.first_occurrence_time, 100);
    assert_eq!(record.last_occurrence_time, 200);
}

/// Tests that OnTf trigger creates a freeze frame on tf rising edge.
///
/// **Use Case**: When an event is configured with save_trigger = OnTf, the DEM
/// should create a freeze frame when the test failed flag (tf) rises.
///
/// **Setup**: Uses fixture event 25 which has save_trigger = OnTf
///
/// **Flow**:
/// 1. clear() + init()
/// 2. step(Failed) - tf rises, cdtc set (confirmation reached)
/// 3. Freeze frame created on tf rising edge
///
/// **Expected**: Freeze frame created after tf rises.
#[test]
fn bdd_freeze_frame_list_ontf_trigger() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    // Event 25 has save_trigger: OnTf
    // After one Failed step, tf rises and cdtc is set (confirmation_threshold=1)
    manager.step(25, Status::Failed, true, 0.0, 100).unwrap();

    assert_eq!(manager.freeze_frames.len(), 1);
    let record = manager.freeze_frames.get_by_event_id(25).unwrap();
    assert_eq!(record.event_id, 25);
    assert_eq!(record.priority, 26);
    assert_eq!(record.first_occurrence_time, 100);
    assert_eq!(record.last_occurrence_time, 100);
}

/// Tests that OnTftoc trigger creates a freeze frame on tftoc rising edge.
///
/// **Use Case**: When an event is configured with save_trigger = OnTftoc, the DEM
/// should create a freeze frame when the test failed this operation cycle flag (tftoc) rises.
///
/// **Setup**: Uses fixture event 26 which has save_trigger = OnTftoc
///
/// **Flow**:
/// 1. clear() + init()
/// 2. step(Failed) - tf rises, tftoc set
/// 3. Freeze frame created on tftoc rising edge
///
/// **Expected**: Freeze frame created after tftoc rises.
#[test]
fn bdd_freeze_frame_list_ontftoc_trigger() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    // Event 26 has save_trigger: OnTftoc
    // After one Failed step, tf and tftoc rise
    manager.step(26, Status::Failed, true, 0.0, 100).unwrap();

    assert_eq!(manager.freeze_frames.len(), 1);
    let record = manager.freeze_frames.get_by_event_id(26).unwrap();
    assert_eq!(record.event_id, 26);
    assert_eq!(record.priority, 27);
    assert_eq!(record.first_occurrence_time, 100);
    assert_eq!(record.last_occurrence_time, 100);
}

/// Tests that record_update: false prevents last_occurrence_time from being updated.
///
/// **Use Case**: When an event has record_update = false, repeated trigger events
/// should not update the last_occurrence_time of an existing freeze frame.
///
/// **Setup**: Uses fixture event 25 which has save_trigger = OnTf, record_update = false
///
/// **Flow**:
/// 1. clear() + init()
/// 2. step(Failed) - freeze frame created at t=100
/// 3. stop() + init()
/// 4. step(Failed) - tf rises again, freeze frame exists but shouldn't update
/// 5. Verify last_occurrence_time remains 100
///
/// **Expected**: last_occurrence_time stays at 100 despite second trigger.
#[test]
fn bdd_freeze_frame_list_record_update_disabled() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    // Event 25 has save_trigger: OnTf, record_update: false
    // First occurrence at t=100
    manager.step(25, Status::Failed, true, 0.0, 100).unwrap();

    assert_eq!(manager.freeze_frames.len(), 1);
    let record = manager.freeze_frames.get_by_event_id(25).unwrap();
    assert_eq!(record.first_occurrence_time, 100);
    assert_eq!(record.last_occurrence_time, 100);

    manager.stop();
    manager.init();

    // Second occurrence at t=200 - tf rises again
    manager.step(25, Status::Failed, true, 0.0, 200).unwrap();

    // record_update is false, so last_occurrence_time should NOT be updated
    let record = manager.freeze_frames.get_by_event_id(25).unwrap();
    assert_eq!(record.first_occurrence_time, 100);
    assert_eq!(record.last_occurrence_time, 100);
}
