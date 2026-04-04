mod event;
mod event_config;
mod event_manager;
mod freeze_frame;
mod uds_status_byte;

pub use event::{Event, NvmConfig, Status};
pub use event_config::{CalibConfig, DebounceBehavior, DebounceType, SaveTrigger};
pub use event_manager::{EventManager, EventManagerError, EventManagerState};
pub use freeze_frame::{EventId, FreezeFrame, FreezeFrameList, FreezeFrameListError};
pub use uds_status_byte::UdsStatusByte;

include!(concat!(env!("OUT_DIR"), "/generated_config.rs"));
