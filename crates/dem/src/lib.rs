mod event;
mod event_config;
mod event_manager;
mod freeze_frame;
mod indicator;
mod uds_status_byte;

pub use event::{Event, NvmConfig, Status};
pub use event_config::{
    CalibConfig, DebounceBehavior, DebounceType, SaveTrigger, SnapshotConfig, SnapshotSource,
    SNAPSHOT_DATA_SIZE,
};
pub use event_manager::{EventManager, EventManagerError, EventManagerState};
pub use freeze_frame::{EventId, FreezeFrame, FreezeFrameList, FreezeFrameListError};
pub use indicator::{IndicatorLamp, IndicatorLamps, LampBehavior, LampId};
pub use uds_status_byte::UdsStatusByte;

include!(concat!(env!("OUT_DIR"), "/generated_config.rs"));
