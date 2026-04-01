use dem::{
    f_25_events::{reset_event_manager, EVENT_MANAGER},
    CalibConfig, DebounceBehavior, DebounceType, Event, EventManager, EventManagerState,
    ExtendedRecordList, NvmConfig, SaveTrigger, Status, UdsStatusByte,
};

#[test]
fn extended_record_list_full_evicts_lowest_priority() {
    reset_event_manager();
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
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

#[test]
fn save_trigger_onpdtc_creates_extended_record_on_pdtc_rising() {
    let cal_config = CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: DebounceBehavior::Freeze,
        debounce_type: DebounceType::CounterBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 10,
        save_trigger: SaveTrigger::OnPdtc,
    };

    let mut uds = UdsStatusByte::from_raw(0);
    uds.set_tnctoc(true);

    let nvm_config = Box::leak(Box::new(NvmConfig {
        uds_status: uds,
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    }));

    let event = Event {
        debounce_counter: 0,
        uds_status_old: nvm_config.uds_status,
        disabled: false,
        nv_config: nvm_config,
        cal_config,
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

    assert_eq!(manager.extended_records.len(), 1);
    let record = manager.extended_records.get_by_event_id(0).unwrap();
    assert_eq!(record.event_id, 0);
    assert_eq!(record.priority, 10);
    assert_eq!(record.date_at_first_save, 100);
    assert_eq!(record.date_at_last_save, 100);
}

#[test]
fn save_trigger_oncdtc_creates_extended_record_on_cdtc_rising() {
    let cal_config = CalibConfig {
        step_up: 1,
        step_down: 0,
        debounce_behavior: DebounceBehavior::Freeze,
        debounce_type: DebounceType::CounterBased,
        confirmation_threshold: 1,
        aging_threshold: 1,
        priority: 10,
        save_trigger: SaveTrigger::OnCdtc,
    };

    let mut uds = UdsStatusByte::from_raw(0);
    uds.set_tnctoc(true);

    let nvm_config = Box::leak(Box::new(NvmConfig {
        uds_status: uds,
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    }));

    let event = Event {
        debounce_counter: 0,
        uds_status_old: nvm_config.uds_status,
        disabled: false,
        nv_config: nvm_config,
        cal_config,
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
    assert!(manager.extended_records.get_by_event_id(0).is_none());

    manager.stop();
    manager.init();

    let status = manager.step(0, Status::Failed, true, 0.0, 200).unwrap();
    assert!(status.cdtc());
    assert!(manager.extended_records.get_by_event_id(0).is_some());
}
