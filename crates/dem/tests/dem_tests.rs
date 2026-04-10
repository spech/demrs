use dem::{
    f_25_events::EVENT_MANAGER, AgingMode, CalibConfig, DebounceBehavior, DebounceType,
    EventManager, EventManagerState, FreezeFrameList, LampBehavior, LampId, NvmConfig, SaveTrigger,
    SnapshotConfig, SnapshotSource, Status,
};
use serial_test::serial;

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
#[serial]
fn bdd_freeze_frame_list_full_eviction() {
    let cal_config_template = dem::CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: dem::DebounceBehavior::Freeze,
        debounce_type: dem::DebounceType::CounterBased,
        confirmation_threshold: 1,
        healing_threshold: 1,
        aging_threshold: 4,
        aging_mode: AgingMode::OperCycle,
        priority: 10,
        save_trigger: dem::SaveTrigger::OnPdtc,
        record_update: true,
        lamp_behaviors: [
            dem::LampBehavior::Off,
            dem::LampBehavior::Off,
            dem::LampBehavior::Off,
            dem::LampBehavior::Off,
        ],
    };

    let mut events = Vec::new();
    for i in 0..25 {
        let mut uds = dem::UdsStatusByte::from_raw(0);
        uds.set_tnctoc(true);

        let nvm_config = Box::leak(Box::new(dem::NvmConfig {
            uds_status: uds,
            occurence_cntr: 0,
            healing_cycles: 0,
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

    static mut TEST_TIMESTAMP: u32 = 0;
    static mut INDICATOR_LAMPS_NVM: dem::IndicatorLamps = dem::IndicatorLamps::new();

    let mut manager = EventManager {
        events,
        freeze_frames: ff_list,
        snapshot_config: create_empty_snapshot_config(),
        state: EventManagerState::Off,
        freeze_frames_lock: spin::Mutex::new(()),
        timestamp: unsafe { &mut *(&raw mut TEST_TIMESTAMP) },
        indicator_lamps: unsafe { &mut *(&raw mut INDICATOR_LAMPS_NVM) },
    };

    manager.init();
    for i in 0..24 {
        *manager.timestamp = i;
        manager.step(i as u16, Status::Failed, true, 0.0).unwrap();
    }

    //assert!(manager.freeze_frames.is_full());
    assert_eq!(manager.freeze_frames.len(), 24);
    assert!(manager.freeze_frames.get_by_event_id(0).is_some());

    *manager.timestamp = 24;
    let err = manager.step(24, Status::Failed, true, 0.0);
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
#[serial]
fn bdd_freeze_frame_list_full_reject_lower_priority() {
    let cal_config_template = CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: DebounceBehavior::Freeze,
        debounce_type: DebounceType::CounterBased,
        confirmation_threshold: 1,
        healing_threshold: 1,
        aging_threshold: 4,
        aging_mode: AgingMode::OperCycle,
        priority: 24,
        save_trigger: SaveTrigger::OnPdtc,
        record_update: true,
        lamp_behaviors: [
            LampBehavior::Off,
            LampBehavior::Off,
            LampBehavior::Off,
            LampBehavior::Off,
        ],
    };

    let mut events = Vec::new();
    for i in 0..25 {
        let mut uds = dem::UdsStatusByte::from_raw(0);
        uds.set_tnctoc(true);

        let nvm_config = Box::leak(Box::new(NvmConfig {
            uds_status: uds,
            occurence_cntr: 0,
            healing_cycles: 0,
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

    static mut TEST_TIMESTAMP: u32 = 0;
    static mut INDICATOR_LAMPS_NVM: dem::IndicatorLamps = dem::IndicatorLamps::new();

    let mut manager = EventManager {
        events,
        freeze_frames: ff_list,
        snapshot_config: create_empty_snapshot_config(),
        state: EventManagerState::Off,
        freeze_frames_lock: spin::Mutex::new(()),
        timestamp: unsafe { &mut *(&raw mut TEST_TIMESTAMP) },
        indicator_lamps: unsafe { &mut *(&raw mut INDICATOR_LAMPS_NVM) },
    };

    manager.init();
    for i in 0..24 {
        *manager.timestamp = i;
        manager.step(i as u16, Status::Failed, true, 0.0).unwrap();
    }

    assert!(manager.freeze_frames.is_full());
    assert_eq!(manager.freeze_frames.len(), 24);
    assert!(manager.freeze_frames.get_by_event_id(0).is_some());

    *manager.timestamp = 24;
    let err = manager.step(24, Status::Failed, true, 0.0);
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
/// frame is created with event_id=13, priority=14, and timestamps set to 0.
#[test]
#[serial]
fn bdd_freeze_frame_list_onpdtc_trigger() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    *manager.timestamp = 0;
    manager.step(13, Status::Failed, true, 0.0).unwrap();

    assert_eq!(manager.freeze_frames.len(), 1);
    let record = manager.freeze_frames.get_by_event_id(13).unwrap();
    assert_eq!(record.event_id, 13);
    assert_eq!(record.priority, 14);
    assert_eq!(record.first_occurrence_time, 0);
    assert_eq!(record.last_occurrence_time, 0);
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
#[serial]
fn bdd_freeze_frame_list_oncdtc_trigger() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    *manager.timestamp = 0;
    manager.step(0, Status::Failed, true, 0.0).unwrap();
    assert!(manager.freeze_frames.get_by_event_id(0).is_some());
    manager.handler_10ms();
    for lamp_id in [LampId::Mil, LampId::Rsl, LampId::Awl, LampId::Pl] {
        assert!(manager.is_lamp_on(lamp_id));
    }
    manager.stop();
    manager.init();

    *manager.timestamp = 1;
    let status = manager.step(0, Status::Failed, true, 0.0).unwrap();
    assert!(status.cdtc());
    assert!(manager.freeze_frames.get_by_event_id(0).is_some());
}

/// Tests that healed events are removed from freeze frames when healing completes.
///
/// **Use Case**: After a confirmed DTC passes through the healing process (operating
/// cycles where the test passes), it should be healed out and removed from storage.
/// This prevents storage from filling with historical resolved faults and ensures
/// only recent/relevant faults remain in the system.
///
/// **Setup**: Uses fixture event 12 which has save_trigger = OnCdtc, healing_threshold = 10
///
/// **Flow**:
/// 1. First occurrence: cdtc set, record created
/// 2. Healing cycles: Test passes for 10+ operating cycles
/// 3. After threshold: stop() clears cdtc, triggering removal from freeze frames
///
/// **Healing Requirements** (all must be true in stop()):
/// - tftoc must be false (test not failed this cycle)
/// - tnctoc must be false (test complete this cycle)
/// - cdtc must be true (still confirmed)
/// - healing_cycles must reach healing_threshold
#[test]
#[serial]
fn bdd_freeze_frame_list_remove_aged_event() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    *manager.timestamp = 0;
    manager.step(12, Status::Failed, true, 0.0).unwrap();
    assert!(manager.freeze_frames.get_by_event_id(12).is_some());

    manager.events[12].nv_config.healing_cycles = 0;
    manager.events[12].nv_config.uds_status = dem::UdsStatusByte::from_raw(0);
    manager.events[12].nv_config.uds_status.set_cdtc(true);

    for _ in 0..=manager.events[12].cal_config.healing_threshold {
        manager.step(12, Status::Passed, true, 0.0).unwrap();
        manager.stop();
        manager.init();
    }

    assert_eq!(manager.events[12].nv_config.uds_status.wir(), false);
    assert_eq!(manager.events[12].nv_config.uds_status.cdtc(), true);
    assert!(manager.freeze_frames.get_by_event_id(12).is_some());

    for _ in 0..=manager.events[12].cal_config.aging_threshold + 2 {
        manager.step(12, Status::Passed, true, 0.0).unwrap();
        manager.stop();
        manager.init();
    }

    assert!(manager.freeze_frames.get_by_event_id(12).is_none());
}

/// Tests that repeated occurrences update last_occurrence_time but preserve first_occurrence_time.
///
/// **Use Case**: When the same event fails multiple times (intermittent fault), the DEM
/// should track both when the fault was first recorded and when it was most recently
/// active. This helps diagnose intermittent issues by showing fault history.
///
/// **Setup**: Uses fixture event 23 which has save_trigger = OnTf, record_update = true
///
/// **Flow**:
/// 1. clear() + init()
/// 2. First failure at timestamp 0: record created, occurrence_counter = 1
/// 3. Passed to reset tf
/// 4. Second failure at timestamp 2: last_occurrence_time updated to 2
///
/// **Expected**: first_occurrence_time remains 0 (unchanged), last_occurrence_time becomes 2.
#[test]
#[serial]
fn bdd_freeze_frame_list_new_occurence_update_last_occurrence_time() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    *manager.timestamp = 0;
    manager.step(23, Status::Failed, true, 0.0).unwrap();

    assert_eq!(manager.events[23].nv_config.occurence_cntr, 1);
    assert_eq!(manager.freeze_frames.len(), 1);
    let record = manager.freeze_frames.get_by_event_id(23).unwrap();
    assert_eq!(record.first_occurrence_time, 0);
    assert_eq!(record.last_occurrence_time, 0);

    manager.step(23, Status::Passed, true, 0.0).unwrap();

    *manager.timestamp = 2;
    manager.step(23, Status::Failed, true, 0.0).unwrap();

    let record = manager.freeze_frames.get_by_event_id(23).unwrap();
    assert_eq!(record.first_occurrence_time, 0);
    assert_eq!(record.last_occurrence_time, 2);
}

/// Tests that OnTf trigger creates a freeze frame on tf rising edge.
///
/// **Use Case**: When an event is configured with save_trigger = OnTf, the DEM
/// should create a freeze frame when the test failed flag (tf) rises.
///
/// **Setup**: Uses fixture event 23 which has save_trigger = OnTf
///
/// **Flow**:
/// 1. clear() + init()
/// 2. step(Failed) - tf rises, cdtc set (confirmation reached)
/// 3. Freeze frame created on tf rising edge
///
/// **Expected**: Freeze frame created after tf rises.
#[test]
#[serial]
fn bdd_freeze_frame_list_ontf_trigger() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    // Event 23 has save_trigger: OnTf
    // After one Failed step, tf rises and cdtc is set (confirmation_threshold=1)
    *manager.timestamp = 0;
    manager.step(23, Status::Failed, true, 0.0).unwrap();

    assert_eq!(manager.freeze_frames.len(), 1);
    let record = manager.freeze_frames.get_by_event_id(23).unwrap();
    assert_eq!(record.event_id, 23);
    assert_eq!(record.priority, 24);
    assert_eq!(record.first_occurrence_time, 0);
    assert_eq!(record.last_occurrence_time, 0);
}

/// Tests that OnTftoc trigger creates a freeze frame on tftoc rising edge.
///
/// **Use Case**: When an event is configured with save_trigger = OnTftoc, the DEM
/// should create a freeze frame when the test failed this operation cycle flag (tftoc) rises.
///
/// **Setup**: Uses fixture event 24 which has save_trigger = OnTftoc
///
/// **Flow**:
/// 1. clear() + init()
/// 2. step(Failed) - tf rises, tftoc set
/// 3. Freeze frame created on tftoc rising edge
///
/// **Expected**: Freeze frame created after tftoc rises.
#[test]
#[serial]
fn bdd_freeze_frame_list_ontftoc_trigger() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    // Event 24 has save_trigger: OnTftoc
    // After one Failed step, tf and tftoc rise
    *manager.timestamp = 0;
    manager.step(24, Status::Failed, true, 0.0).unwrap();

    assert_eq!(manager.freeze_frames.len(), 1);
    let record = manager.freeze_frames.get_by_event_id(24).unwrap();
    assert_eq!(record.event_id, 24);
    assert_eq!(record.priority, 25);
    assert_eq!(record.first_occurrence_time, 0);
    assert_eq!(record.last_occurrence_time, 0);
}

/// Tests that record_update: false prevents last_occurrence_time from being updated.
///
/// **Use Case**: When an event has record_update = false, repeated trigger events
/// should not update the last_occurrence_time of an existing freeze frame.
///
/// **Setup**: Uses fixture event 13 which has save_trigger = OnPdtc, record_update = false
///
/// **Flow**:
/// 1. clear() + init()
/// 2. step(Failed) - freeze frame created at t=0
/// 3. stop() + init()
/// 4. step(Failed) - pdtc rises again, freeze frame exists but shouldn't update
/// 5. Verify last_occurrence_time remains 0
///
/// **Expected**: last_occurrence_time stays at 0 despite second trigger.
#[test]
#[serial]
fn bdd_freeze_frame_list_record_update_disabled() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    // Event 13 has save_trigger: OnPdtc, record_update: false
    // First occurrence at t=0
    *manager.timestamp = 0;
    manager.step(13, Status::Failed, true, 0.0).unwrap();

    assert_eq!(manager.freeze_frames.len(), 1);
    let record = manager.freeze_frames.get_by_event_id(13).unwrap();
    assert_eq!(record.first_occurrence_time, 0);
    assert_eq!(record.last_occurrence_time, 0);

    manager.stop();
    manager.init();

    // Second occurrence - pdtc rises again
    *manager.timestamp = 1;
    manager.step(13, Status::Failed, true, 0.0).unwrap();

    // record_update is false, so last_occurrence_time should NOT be updated
    let record = manager.freeze_frames.get_by_event_id(13).unwrap();
    assert_eq!(record.first_occurrence_time, 0);
    assert_eq!(record.last_occurrence_time, 0);
}

/// Tests that freeze frames are removed for events with aging_mode = WarmUpCycle.
///
/// **Use Case**: When a confirmed DTC with WarmUpCycle aging mode completes its warm-up
/// cycles, the aging counter increments and eventually clears CDTC, removing the freeze frame.
///
/// **Setup**: Uses fixture event 14 which has aging_mode = WarmUpCycle, aging_threshold = 4
///
/// **Flow**:
/// 1. Trigger event to Failed → CDTC set, freeze frame created
/// 2. Healing: Run operating cycles until wir clears
/// 3. Warm-up aging: Run warm-up cycles until aging completes
/// 4. CDTC clears and freeze frame is removed
#[test]
#[serial]
fn bdd_freeze_frame_for_warmup_cycle_event() {
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.clear();
    manager.init();

    *manager.timestamp = 0;
    manager.step(14, Status::Failed, true, 0.0).unwrap();
    manager.stop();
    manager.init();

    assert!(manager.freeze_frames.get_by_event_id(14).is_some());
    assert_eq!(
        manager.events[14].cal_config.aging_mode,
        AgingMode::WarmUpCycle
    );

    manager.events[14].nv_config.healing_cycles = 0;
    manager.events[14].nv_config.uds_status = dem::UdsStatusByte::from_raw(0);
    manager.events[14].nv_config.uds_status.set_cdtc(true);

    for _ in 0..=manager.events[14].cal_config.healing_threshold {
        manager.step(14, Status::Passed, true, 0.0).unwrap();
        manager.stop();
        manager.init();
    }

    assert!(!manager.events[14].nv_config.uds_status.wir());
    assert!(manager.events[14].nv_config.uds_status.cdtc());

    for _ in 0..=manager.events[14].cal_config.aging_threshold {
        manager.step(14, Status::Passed, true, 0.0).unwrap();
        manager.handle_warmup_cycle();
        manager.stop();
        manager.init();
    }

    assert!(manager.freeze_frames.get_by_event_id(14).is_none());
}
