use dem::{
    f_25_events::{reset_event_manager, EVENT_MANAGER},
    EventManager, Status,
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
