use dem::{
    f_25_events::EVENT_MANAGER, EventManager, EventManagerState, ExtendedRecordList, Status,
};

#[test]
fn extended_record_list_full_evicts_lowest_priority() {
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

#[test]
fn save_trigger_onpdtc_creates_extended_record_on_pdtc_rising() {
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

#[test]
fn save_trigger_oncdtc_creates_extended_record_on_cdtc_rising() {
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
