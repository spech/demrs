mod event;
mod event_config;
mod event_manager;
mod extended_record;
mod uds_status_byte;

pub use event::{Event, NvmConfig, Status};
pub use event_config::{CalibConfig, DebounceBehavior, DebounceType, SaveTrigger};
pub use event_manager::{EventManager, EventManagerError, EventManagerState};
pub use extended_record::{EventId, ExtendedRecord, ExtendedRecordList, ExtendedRecordListError};
pub use uds_status_byte::UdsStatusByte;

include!(concat!(env!("OUT_DIR"), "/generated_config.rs"));
