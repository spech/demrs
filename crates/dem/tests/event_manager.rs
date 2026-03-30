use dem::{
    f_25_events::{reset_event_manager, EVENT_MANAGER},
    EventManager, Status,
};

#[test]
fn event_manager_rising_tf_creates_extended_record() {
    reset_event_manager();
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.init();
    let result = manager.step(0, Status::Failed, true, 0.0, 100).unwrap();
    eprintln!("After step: cdtc={}", result.cdtc());
    eprintln!("Extended records len: {}", manager.extended_records.len());

    let ext_rec = manager.extended_records.get_by_event_id(0);
    assert!(ext_rec.is_some());
    assert_eq!(ext_rec.unwrap().event_id, 0);
    assert_eq!(ext_rec.unwrap().priority, 1);
    assert!(ext_rec.unwrap().date_at_first_save != 0);
    assert!(ext_rec.unwrap().date_at_last_save != 0);
}

#[test]
fn event_manager_no_extended_record_without_rising_edge() {
    reset_event_manager();
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.init();
    manager.step(0, Status::Passed, true, 0.0, 100).unwrap();

    assert!(manager.extended_records.is_empty());
}

#[test]
fn event_manager_extended_record_no_update_without_rising_edge() {
    reset_event_manager();
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.init();
    manager.step(0, Status::Failed, true, 0.0, 100).unwrap();

    let first_date = manager
        .extended_records
        .get_by_event_id(0)
        .unwrap()
        .date_at_last_save;

    manager.step(0, Status::Failed, true, 0.0, 200).unwrap();

    let second_date = manager
        .extended_records
        .get_by_event_id(0)
        .unwrap()
        .date_at_last_save;

    assert_eq!(second_date, first_date);
}

#[test]
fn event_manager_extended_record_reinsert_on_rising_edge_after_stop() {
    reset_event_manager();
    let manager = unsafe {
        (&raw mut EVENT_MANAGER as *mut EventManager)
            .as_mut()
            .unwrap()
    };

    manager.init();
    manager.step(0, Status::Failed, true, 0.0, 100).unwrap();
    let first_date = manager
        .extended_records
        .get_by_event_id(0)
        .unwrap()
        .date_at_last_save;

    manager.stop();
    manager.init();

    manager.step(0, Status::Failed, true, 0.0, 200).unwrap();
    let second_date = manager
        .extended_records
        .get_by_event_id(0)
        .unwrap()
        .date_at_last_save;

    assert_eq!(second_date, 200);
    assert_ne!(first_date, second_date);
}
