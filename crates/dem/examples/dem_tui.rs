use core::ptr::{addr_of, addr_of_mut};
use crossterm::{
    event::{KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table,
    },
    Terminal,
};
use std::io;
use std::time::{Duration, Instant};

static EVENT_NAMES: [&str; 25] = [
    "OverTemp",
    "UnderVolt",
    "OverSpeed",
    "LowFuel",
    "BrakeFail",
    "SensorErr",
    "CommFault",
    "OilPress",
    "Coolant",
    "Catalyst",
    "O2Sensor",
    "EGR",
    "VVT",
    "Turbo",
    "AFRatio",
    "MAFSensor",
    "Ignition",
    "Knock",
    "EVAP",
    "Thermostat",
    "Battery",
    "Alternator",
    "ABS",
    "Airbag",
    "TransGear",
];

mod theme {
    use ratatui::style::Color;

    pub const BG_LIGHT: Color = Color::Rgb(36, 40, 59);
    pub const FG: Color = Color::Rgb(192, 202, 245);
    pub const GREEN: Color = Color::Rgb(158, 206, 106);
    pub const RED: Color = Color::Rgb(247, 118, 142);
    pub const PURPLE: Color = Color::Rgb(187, 154, 247);
    pub const YELLOW: Color = Color::Rgb(224, 175, 104);
    pub const CYAN: Color = Color::Rgb(122, 162, 255);
    pub const BLUE: Color = Color::Rgb(86, 182, 194);
    pub const ORANGE: Color = Color::Rgb(255, 165, 0);
    pub const MUTED: Color = Color::Rgb(86, 95, 137);
    pub const SELECTION: Color = Color::Rgb(36, 40, 59);
}

use dem::{
    CalibConfig, DebounceBehavior, DebounceType, Event, EventId, EventManager, EventManagerState,
    FreezeFrame, FreezeFrameList, NvmConfig, SaveTrigger, SnapshotConfig, SnapshotSource, Status,
    UdsStatusByte,
};
use spin::Mutex;

static mut FF_LIST_NVM: [Option<FreezeFrame>; 24] = [const { None }; 24];

static mut EVENT_NVM: [NvmConfig; 25] = [
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
    NvmConfig {
        uds_status: UdsStatusByte::from_raw(0b0100_0000),
        occurence_cntr: 0,
        aging_cycles: 0,
        confirmation_cycles: 0,
    },
];

static mut SNAPSHOT_DATA: [u8; 255] = [0u8; 255];

static mut SNAPSHOT_CONFIG: SnapshotConfig = SnapshotConfig {
    sources: [SnapshotSource {
        address: std::ptr::null(),
        size: 0,
    }; 255],
    count: 0,
};

static mut TIMESTAMP: u32 = 0;

static mut SIMULATION_TICK: u32 = 0;

#[derive(Copy, Clone)]
#[repr(C)]
struct SystemState {
    battery_voltage_x10: u8,
    engine_rpm: u16,
    vehicle_speed: u16,
    coolant_temp: u8,
    intake_air_temp: u8,
    throttle_position: u8,
    obd_cycle_counter: u16,
    system_mode: u8,
    fuel_level: u8,
    oil_pressure: u16,
    transmission_temp: u8,
    exhaust_temp: u8,
    maf_rate: u16,
    dyno_load: u8,
    fuel_pressure: u8,
}

static mut SYSTEM_STATE: SystemState = SystemState {
    battery_voltage_x10: 120,
    engine_rpm: 0,
    vehicle_speed: 0,
    coolant_temp: 20,
    intake_air_temp: 20,
    throttle_position: 0,
    obd_cycle_counter: 0,
    system_mode: 0x01,
    fuel_level: 75,
    oil_pressure: 45,
    transmission_temp: 70,
    exhaust_temp: 80,
    maf_rate: 15,
    dyno_load: 50,
    fuel_pressure: 50,
};

static mut SYSTEM_STATE_OLD: SystemState = SystemState {
    battery_voltage_x10: 120,
    engine_rpm: 0,
    vehicle_speed: 0,
    coolant_temp: 20,
    intake_air_temp: 20,
    throttle_position: 0,
    obd_cycle_counter: 0,
    system_mode: 0x01,
    fuel_level: 75,
    oil_pressure: 45,
    transmission_temp: 70,
    exhaust_temp: 80,
    maf_rate: 15,
    dyno_load: 50,
    fuel_pressure: 50,
};

fn update_live_system_state(tick: u32) {
    unsafe {
        SYSTEM_STATE_OLD = SYSTEM_STATE;

        SYSTEM_STATE.battery_voltage_x10 = 118_u8.saturating_add(((tick % 10) * 7 % 10) as u8);

        let base_rpm = 800 + (tick % 600);
        let variation = (((tick % 200) * 37 % 200) as i16) - 100;
        SYSTEM_STATE.engine_rpm = (base_rpm as i16 + variation) as u16;

        SYSTEM_STATE.vehicle_speed = ((tick % 180) * 13 % 180) as u16;
        SYSTEM_STATE.coolant_temp = 75_u8.saturating_add(((tick % 35) * 3 % 35) as u8);
        SYSTEM_STATE.intake_air_temp = 20_u8.saturating_add(((tick % 25) * 5 % 25) as u8);
        SYSTEM_STATE.throttle_position = ((tick % 100) * 17 % 100) as u8;
        SYSTEM_STATE.obd_cycle_counter = (tick / 60) as u16;

        SYSTEM_STATE.fuel_level = 75_u8.saturating_sub(((tick % 15) * 2 % 30) as u8);
        SYSTEM_STATE.oil_pressure = 45_u16.saturating_add(((tick % 20) * 7 % 140) as u16);
        SYSTEM_STATE.transmission_temp = 70_u8.saturating_add(((tick % 10) * 5 % 40) as u8);
        SYSTEM_STATE.exhaust_temp = 80_u8.saturating_add(((tick % 20) * 11 % 200) as u8);
        SYSTEM_STATE.maf_rate = 15_u16.saturating_add(((tick % 20) * 3 % 60) as u16);
        SYSTEM_STATE.dyno_load = 50_u8.saturating_add(((tick % 10) * 13 % 130) as u8);
        SYSTEM_STATE.fuel_pressure = 50_u8.saturating_add(((tick % 15) * 17 % 205) as u8);
    }
}

fn get_trend_indicator<T: PartialEq + PartialOrd>(current: T, old: T) -> &'static str {
    if current > old {
        "▲"
    } else if current < old {
        "▼"
    } else {
        "─"
    }
}

struct App {
    manager: EventManager,
    viewed_event: usize,
    trigger_event: Option<usize>,
    show_freeze_frames: bool,
    show_raw_snapshot: bool,
    selected_ff: Option<usize>,
    editing_calib: bool,
    editing_field: usize,
    last_action: String,
    ff_scroll: usize,
    scroll: usize,
    show_help: bool,
    current_cycle: u32,
}

impl App {
    fn new() -> Self {
        let ff_list = unsafe { FreezeFrameList::from_nvm(&*addr_of!(FF_LIST_NVM)) };

        let cal_configs = [
            CalibConfig {
                step_up: 1,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 3,
                priority: 1,
                save_trigger: SaveTrigger::OnCdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 3,
                step_down: 1,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 5,
                priority: 2,
                save_trigger: SaveTrigger::OnPdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 5,
                step_down: 2,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 3,
                priority: 3,
                save_trigger: SaveTrigger::OnTf,
                record_update: false,
            },
            CalibConfig {
                step_up: 10,
                step_down: 5,
                debounce_behavior: DebounceBehavior::Reset,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 10,
                priority: 4,
                save_trigger: SaveTrigger::OnTftoc,
                record_update: true,
            },
            CalibConfig {
                step_up: 1,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 3,
                priority: 5,
                save_trigger: SaveTrigger::OnCdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 25,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::TimeBased,
                confirmation_threshold: 1,
                aging_threshold: 5,
                priority: 6,
                save_trigger: SaveTrigger::OnPdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 1,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 3,
                priority: 7,
                save_trigger: SaveTrigger::OnCdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 3,
                step_down: 3,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 5,
                priority: 8,
                save_trigger: SaveTrigger::OnTf,
                record_update: true,
            },
            CalibConfig {
                step_up: 1,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 3,
                priority: 9,
                save_trigger: SaveTrigger::OnCdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 2,
                step_down: 1,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 4,
                priority: 10,
                save_trigger: SaveTrigger::OnPdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 4,
                step_down: 2,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 6,
                priority: 11,
                save_trigger: SaveTrigger::OnTf,
                record_update: true,
            },
            CalibConfig {
                step_up: 6,
                step_down: 3,
                debounce_behavior: DebounceBehavior::Reset,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 7,
                priority: 12,
                save_trigger: SaveTrigger::OnCdtc,
                record_update: false,
            },
            CalibConfig {
                step_up: 1,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 3,
                priority: 13,
                save_trigger: SaveTrigger::OnTftoc,
                record_update: true,
            },
            CalibConfig {
                step_up: 8,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::TimeBased,
                confirmation_threshold: 1,
                aging_threshold: 8,
                priority: 14,
                save_trigger: SaveTrigger::OnPdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 3,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 4,
                priority: 15,
                save_trigger: SaveTrigger::OnCdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 5,
                step_down: 5,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 6,
                priority: 16,
                save_trigger: SaveTrigger::OnTf,
                record_update: true,
            },
            CalibConfig {
                step_up: 2,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Reset,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 5,
                priority: 17,
                save_trigger: SaveTrigger::OnPdtc,
                record_update: false,
            },
            CalibConfig {
                step_up: 1,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::TimeBased,
                confirmation_threshold: 1,
                aging_threshold: 3,
                priority: 18,
                save_trigger: SaveTrigger::OnCdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 7,
                step_down: 1,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 9,
                priority: 19,
                save_trigger: SaveTrigger::OnTf,
                record_update: true,
            },
            CalibConfig {
                step_up: 4,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 5,
                priority: 20,
                save_trigger: SaveTrigger::OnTftoc,
                record_update: true,
            },
            CalibConfig {
                step_up: 1,
                step_down: 1,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 4,
                priority: 21,
                save_trigger: SaveTrigger::OnCdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 9,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Reset,
                debounce_type: DebounceType::TimeBased,
                confirmation_threshold: 1,
                aging_threshold: 10,
                priority: 22,
                save_trigger: SaveTrigger::OnPdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 2,
                step_down: 2,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 6,
                priority: 23,
                save_trigger: SaveTrigger::OnTf,
                record_update: false,
            },
            CalibConfig {
                step_up: 3,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::CounterBased,
                confirmation_threshold: 1,
                aging_threshold: 4,
                priority: 24,
                save_trigger: SaveTrigger::OnCdtc,
                record_update: true,
            },
            CalibConfig {
                step_up: 1,
                step_down: 0,
                debounce_behavior: DebounceBehavior::Freeze,
                debounce_type: DebounceType::TimeBased,
                confirmation_threshold: 1,
                aging_threshold: 3,
                priority: 25,
                save_trigger: SaveTrigger::OnPdtc,
                record_update: true,
            },
        ];

        let events: Vec<Event> = cal_configs
            .iter()
            .enumerate()
            .map(|(i, cal)| {
                let nvm = unsafe { &mut EVENT_NVM[i] };
                Event {
                    debounce_counter: 0,
                    uds_status_old: UdsStatusByte::from_raw(0),
                    disabled: false,
                    nv_config: nvm,
                    cal_config: *cal,
                }
            })
            .collect();

        let manager = EventManager {
            events: Box::leak(events.into_boxed_slice()),
            freeze_frames: Box::leak(Box::new(ff_list)),
            snapshot_config: unsafe {
                SNAPSHOT_CONFIG.sources[0] = SnapshotSource {
                    address: addr_of!(SNAPSHOT_DATA) as *const u8,
                    size: 255,
                };
                SNAPSHOT_CONFIG.count = 1;
                &*addr_of!(SNAPSHOT_CONFIG)
            },
            state: EventManagerState::Off,
            freeze_frames_lock: Mutex::new(()),
            timestamp: unsafe { &mut *addr_of_mut!(TIMESTAMP) },
        };

        Self {
            manager,
            viewed_event: 0,
            trigger_event: None,
            show_freeze_frames: false,
            show_raw_snapshot: false,
            selected_ff: None,
            editing_calib: false,
            editing_field: 0,
            last_action: "Ready".to_string(),
            ff_scroll: 0,
            scroll: 0,
            show_help: false,
            current_cycle: 0,
        }
    }

    fn trigger_event(&mut self, event_id: usize, status: Status) {
        if self.manager.state == EventManagerState::Off {
            self.last_action = "Error: Not initialized".to_string();
            return;
        }
        if event_id >= self.manager.events.len() {
            return;
        }

        unsafe {
            let state = &raw const SYSTEM_STATE;
            let state = &*state;
            SNAPSHOT_DATA[0] = state.battery_voltage_x10;
            SNAPSHOT_DATA[1] = state.engine_rpm as u8;
            SNAPSHOT_DATA[2] = (state.engine_rpm >> 8) as u8;
            SNAPSHOT_DATA[3] = state.vehicle_speed as u8;
            SNAPSHOT_DATA[4] = (state.vehicle_speed >> 8) as u8;
            SNAPSHOT_DATA[5] = state.coolant_temp;
            SNAPSHOT_DATA[6] = state.intake_air_temp;
            SNAPSHOT_DATA[7] = state.throttle_position;
            SNAPSHOT_DATA[8] = state.obd_cycle_counter as u8;
            SNAPSHOT_DATA[9] = (state.obd_cycle_counter >> 8) as u8;
            SNAPSHOT_DATA[10] = state.fuel_level;
            SNAPSHOT_DATA[11] = state.oil_pressure as u8;
            SNAPSHOT_DATA[12] = (state.oil_pressure >> 8) as u8;
            SNAPSHOT_DATA[13] = state.transmission_temp;
            SNAPSHOT_DATA[14] = state.exhaust_temp;
            SNAPSHOT_DATA[15] = state.maf_rate as u8;
            SNAPSHOT_DATA[16] = (state.maf_rate >> 8) as u8;
            SNAPSHOT_DATA[17] = state.dyno_load;
            SNAPSHOT_DATA[18] = state.fuel_pressure;
            SNAPSHOT_DATA[19] = match status {
                Status::Failed | Status::PreFailed => 0x02,
                _ => 0x01,
            };

            for i in 20..255 {
                SNAPSHOT_DATA[i] = 0;
            }
        }

        let event_name = format!("Event{}", event_id);
        let status_name = match status {
            Status::PreFailed => "PreFailed",
            Status::PrePassed => "PrePassed",
            Status::Failed => "Failed",
            Status::Passed => "Passed",
        };

        match self.manager.step(event_id as EventId, status, true, 0.01) {
            Ok(_) => {
                self.last_action = format!("Stepped {} with {}", event_name, status_name);
            }
            Err(e) => {
                self.last_action = format!("Error: {:?}", e);
            }
        }
    }

    fn init_cycle(&mut self) {
        self.manager.init();
        self.last_action = "Cycle initialized".to_string();
    }

    fn stop_cycle(&mut self) {
        self.manager.stop();
        self.last_action = "Cycle stopped".to_string();
    }

    fn next_cycle(&mut self) {
        self.manager.stop();
        self.manager.init();
        self.current_cycle += 1;
        self.last_action = format!("Advanced to cycle {}", self.current_cycle);
    }

    fn clear_all(&mut self) {
        self.manager.clear();
        self.last_action = "All events cleared".to_string();
    }

    fn get_calib_field_name(field: usize) -> &'static str {
        match field {
            0 => "step_up",
            1 => "step_down",
            2 => "debounce_type",
            3 => "debounce_behavior",
            4 => "confirmation_threshold",
            5 => "aging_threshold",
            6 => "priority",
            7 => "save_trigger",
            8 => "record_update",
            _ => "",
        }
    }

    fn adjust_calib_value(&mut self, increment: bool) {
        let event = &mut self.manager.events[self.viewed_event];
        match self.editing_field {
            0 => {
                let new_val = if increment {
                    event.cal_config.step_up.saturating_add(1)
                } else {
                    event.cal_config.step_up.saturating_sub(1)
                };
                event.cal_config.step_up = new_val;
                self.last_action = format!("step_up: {}", new_val);
            }
            1 => {
                let new_val = if increment {
                    event.cal_config.step_down.saturating_add(1)
                } else {
                    event.cal_config.step_down.saturating_sub(1)
                };
                event.cal_config.step_down = new_val;
                self.last_action = format!("step_down: {}", new_val);
            }
            2 => {
                event.cal_config.debounce_type =
                    if event.cal_config.debounce_type == DebounceType::CounterBased {
                        DebounceType::TimeBased
                    } else {
                        DebounceType::CounterBased
                    };
                self.last_action = format!("debounce_type: {:?}", event.cal_config.debounce_type);
            }
            3 => {
                event.cal_config.debounce_behavior =
                    if event.cal_config.debounce_behavior == DebounceBehavior::Freeze {
                        DebounceBehavior::Reset
                    } else {
                        DebounceBehavior::Freeze
                    };
                self.last_action = format!(
                    "debounce_behavior: {:?}",
                    event.cal_config.debounce_behavior
                );
            }
            4 => {
                let new_val = if increment {
                    event.cal_config.confirmation_threshold.saturating_add(1)
                } else {
                    event.cal_config.confirmation_threshold.saturating_sub(1)
                };
                event.cal_config.confirmation_threshold = new_val;
                self.last_action = format!("confirmation_threshold: {}", new_val);
            }
            5 => {
                let new_val = if increment {
                    event.cal_config.aging_threshold.saturating_add(1)
                } else {
                    event.cal_config.aging_threshold.saturating_sub(1)
                };
                event.cal_config.aging_threshold = new_val;
                self.last_action = format!("aging_threshold: {}", new_val);
            }
            6 => {
                let new_val = if increment {
                    event.cal_config.priority.saturating_add(1).min(255)
                } else {
                    event.cal_config.priority.saturating_sub(1)
                };
                event.cal_config.priority = new_val;
                self.last_action = format!("priority: {}", new_val);
            }
            7 => {
                event.cal_config.save_trigger = match event.cal_config.save_trigger {
                    SaveTrigger::OnPdtc => SaveTrigger::OnCdtc,
                    SaveTrigger::OnCdtc => SaveTrigger::OnTf,
                    SaveTrigger::OnTf => SaveTrigger::OnTftoc,
                    SaveTrigger::OnTftoc => SaveTrigger::OnPdtc,
                };
                self.last_action = format!("save_trigger: {:?}", event.cal_config.save_trigger);
            }
            8 => {
                event.cal_config.record_update = !event.cal_config.record_update;
                self.last_action = format!("record_update: {}", event.cal_config.record_update);
            }
            _ => {}
        }
    }

    fn get_calib_field_value(&self) -> String {
        let event = &self.manager.events[self.viewed_event];
        match self.editing_field {
            0 => format!("{}", event.cal_config.step_up),
            1 => format!("{}", event.cal_config.step_down),
            2 => format!("{:?}", event.cal_config.debounce_type),
            3 => format!("{:?}", event.cal_config.debounce_behavior),
            4 => format!("{}", event.cal_config.confirmation_threshold),
            5 => format!("{}", event.cal_config.aging_threshold),
            6 => format!("{}", event.cal_config.priority),
            7 => format!("{:?}", event.cal_config.save_trigger),
            8 => format!("{}", event.cal_config.record_update),
            _ => String::new(),
        }
    }
}

fn render_ui(f: &mut ratatui::Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_body(f, app, chunks[1]);
    render_footer(f, app, chunks[2]);
}

fn render_header(f: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let title = match app.manager.state {
        EventManagerState::On => "DEM TUI",
        EventManagerState::Off => "DEM TUI",
    };

    let state_text = match app.manager.state {
        EventManagerState::On => "[ON] ",
        EventManagerState::Off => "[OFF]",
    };

    let state_color = match app.manager.state {
        EventManagerState::On => theme::GREEN,
        EventManagerState::Off => theme::RED,
    };

    let cycle_text = format!("Cycle: {:04}", app.current_cycle);

    let title_line = Line::from(vec![
        Span::raw(title).bold().fg(theme::PURPLE),
        Span::raw(" "),
        Span::raw(state_text).bold().fg(state_color),
        Span::raw("  "),
        Span::raw(cycle_text).fg(theme::CYAN),
    ]);

    f.render_widget(
        Paragraph::new(title_line)
            .style(Style::default().bg(theme::BG_LIGHT).fg(theme::FG))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::MUTED))
                    .title_style(Style::default().fg(theme::PURPLE))
                    .title(" DEM Diagnostic Event Manager "),
            ),
        area,
    );
}

fn render_body(f: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    if app.show_freeze_frames {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        render_event_list(f, app, chunks[0]);
        render_freeze_frames_panel(f, app, chunks[1]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(45),
                Constraint::Percentage(35),
            ])
            .split(area);

        render_event_list(f, app, chunks[0]);
        render_event_details_wrapper(f, app, chunks[1]);
        render_live_system_state(f, app, chunks[2]);
    }
}

fn render_live_system_state(f: &mut ratatui::Frame<'_>, _app: &App, area: Rect) {
    let state = unsafe { &*(&raw const SYSTEM_STATE) };
    let old_state = unsafe { &*(&raw const SYSTEM_STATE_OLD) };

    let make_trend_cell = |current: u32, old: u32, color: ratatui::style::Color| -> Cell<'static> {
        let trend = get_trend_indicator(current, old);
        let trend_color = if current > old {
            color
        } else if current < old {
            theme::RED
        } else {
            theme::MUTED
        };
        Cell::from(Span::raw(format!("{:>2}", trend)).fg(trend_color))
    };

    let make_value_trend_cell =
        |current: u32, old: u32, up_color: ratatui::style::Color| -> Cell<'static> {
            let trend = get_trend_indicator(current, old);
            let trend_color = if current > old {
                up_color
            } else if current < old {
                theme::RED
            } else {
                theme::MUTED
            };
            Cell::from(Span::raw(format!("{:>2}", trend)).fg(trend_color))
        };
    let make_value_cell = |text: String| -> Cell<'static> {
        Cell::from(Line::from(Span::raw(format!("{:>10}", text))))
    };

    let rows: Vec<Row> = vec![
        Row::new(vec![
            Cell::from(Span::raw("Battery Voltage")),
            make_value_cell(format!("{:>7.1}V", state.battery_voltage_x10 as f32 / 10.0)),
            make_trend_cell(
                state.battery_voltage_x10 as u32,
                old_state.battery_voltage_x10 as u32,
                theme::GREEN,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Engine RPM")),
            make_value_cell(format!("{:>6} rpm", state.engine_rpm)),
            make_trend_cell(
                state.engine_rpm as u32,
                old_state.engine_rpm as u32,
                theme::GREEN,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Vehicle Speed")),
            make_value_cell(format!("{:>5} km/h", state.vehicle_speed)),
            make_value_trend_cell(
                state.vehicle_speed as u32,
                old_state.vehicle_speed as u32,
                theme::YELLOW,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Coolant Temp")),
            make_value_cell(format!("{:>4}°C", state.coolant_temp)),
            make_trend_cell(
                state.coolant_temp as u32,
                old_state.coolant_temp as u32,
                theme::RED,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Intake Air")),
            make_value_cell(format!("{:>4}°C", state.intake_air_temp)),
            make_trend_cell(
                state.intake_air_temp as u32,
                old_state.intake_air_temp as u32,
                theme::MUTED,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Throttle")),
            make_value_cell(format!("{:>4}%", state.throttle_position)),
            make_value_trend_cell(
                state.throttle_position as u32,
                old_state.throttle_position as u32,
                theme::YELLOW,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Fuel Level")),
            make_value_cell(format!("{:>5}%", state.fuel_level)),
            make_trend_cell(
                state.fuel_level as u32,
                old_state.fuel_level as u32,
                theme::GREEN,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Oil Pressure")),
            make_value_cell(format!("{:>5}kPa", state.oil_pressure)),
            make_trend_cell(
                state.oil_pressure as u32,
                old_state.oil_pressure as u32,
                theme::GREEN,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Trans Temp")),
            make_value_cell(format!("{:>4}°C", state.transmission_temp)),
            make_trend_cell(
                state.transmission_temp as u32,
                old_state.transmission_temp as u32,
                theme::RED,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Exhaust Temp")),
            make_value_cell(format!("{:>4}°C", state.exhaust_temp)),
            make_trend_cell(
                state.exhaust_temp as u32,
                old_state.exhaust_temp as u32,
                theme::RED,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("MAF Rate")),
            make_value_cell(format!("{:>5}g/s", state.maf_rate)),
            make_trend_cell(
                state.maf_rate as u32,
                old_state.maf_rate as u32,
                theme::GREEN,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Dyno Load")),
            make_value_cell(format!("{:>4}%", state.dyno_load)),
            make_value_trend_cell(
                state.dyno_load as u32,
                old_state.dyno_load as u32,
                theme::YELLOW,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Fuel Press")),
            make_value_cell(format!("{:>6}kPa", state.fuel_pressure)),
            make_trend_cell(
                state.fuel_pressure as u32,
                old_state.fuel_pressure as u32,
                theme::GREEN,
            ),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("OBD Cycle")),
            make_value_cell(format!("{:>8}", state.obd_cycle_counter)),
            Cell::from(Span::raw("  ")),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("System Mode")),
            Cell::from(match state.system_mode {
                0x01 => Span::raw("   Normal").fg(theme::GREEN),
                0x02 => Span::raw("Fault Active").fg(theme::RED),
                0x03 => Span::raw("Fault Healed").fg(theme::YELLOW),
                _ => Span::raw("   Unknown").fg(theme::MUTED),
            }),
            Cell::from(Span::raw("  ")),
        ]),
    ];

    let table = Table::new(
        rows,
        [
            Constraint::Length(20),
            Constraint::Length(10),
            Constraint::Length(3),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from(Span::raw("Parameter")),
            Cell::from(Line::from(Span::raw(format!("{:>10}", "Value")))),
            Cell::from(Span::raw("")),
        ])
        .style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme::PURPLE),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MUTED))
            .title_style(Style::default().fg(theme::CYAN))
            .title(" System State (Live) "),
    );

    f.render_widget(table, area);
}

fn render_event_list(f: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let rows: Vec<Row> = app
        .manager
        .events
        .iter()
        .enumerate()
        .map(|(i, event)| {
            let status = event.status();
            let name = EVENT_NAMES.get(i).unwrap_or(&"Unknown");
            let counter = event.debounce_counter();

            let (t_flag, f_flag, p_flag, c_flag) = (
                if status.tf() { "T" } else { "-" },
                if status.tftoc() { "F" } else { "-" },
                if status.pdtc() { "P" } else { "-" },
                if status.cdtc() { "C" } else { "-" },
            );

            let (t_color, f_color, p_color, c_color) = (
                if status.tf() {
                    theme::RED
                } else {
                    theme::MUTED
                },
                if status.tftoc() {
                    theme::RED
                } else {
                    theme::MUTED
                },
                if status.pdtc() {
                    theme::YELLOW
                } else {
                    theme::MUTED
                },
                if status.cdtc() {
                    theme::RED
                } else {
                    theme::MUTED
                },
            );

            let row_style = if app.trigger_event == Some(i) {
                Style::default()
                    .fg(theme::ORANGE)
                    .add_modifier(Modifier::BOLD)
            } else if i == app.viewed_event {
                Style::default()
                    .fg(theme::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else if status.tf() || status.cdtc() {
                Style::default().fg(theme::RED)
            } else {
                Style::default().fg(theme::FG)
            };

            Row::new(vec![
                Cell::from(Span::raw(format!("{:>3}", i))),
                Cell::from(Span::raw(format!("{:<10}", name))),
                Cell::from(Span::raw(format!("{:>6}", counter))),
                Cell::from(Span::raw(t_flag).fg(t_color)),
                Cell::from(Span::raw(f_flag).fg(f_color)),
                Cell::from(Span::raw(p_flag).fg(p_color)),
                Cell::from(Span::raw(c_flag).fg(c_color)),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(11),
            Constraint::Length(7),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from(Span::raw("Idx")),
            Cell::from(Span::raw("Name")),
            Cell::from(Span::raw("Counter")),
            Cell::from(Span::raw("T")),
            Cell::from(Span::raw("F")),
            Cell::from(Span::raw("P")),
            Cell::from(Span::raw("C")),
        ])
        .style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme::PURPLE),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MUTED))
            .title_style(Style::default().fg(theme::CYAN))
            .title(" Events "),
    )
    .highlight_style(Style::default().bg(theme::SELECTION).fg(theme::ORANGE));

    f.render_widget(table, area);
}

fn render_event_details_wrapper(f: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let event = &app.manager.events[app.viewed_event];
    let status = event.status();
    let cal = &event.cal_config;

    let title = format!(" Details - {} ", EVENT_NAMES[app.viewed_event]);
    f.render_widget(Clear, area);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MUTED))
            .title_style(Style::default().fg(theme::CYAN))
            .title(title),
        area,
    );

    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    let inner_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(24),
            Constraint::Length(1),
            Constraint::Percentage(28),
            Constraint::Length(1),
            Constraint::Percentage(48),
        ])
        .split(inner_layout[0]);

    let line_style = Style::default().fg(theme::MUTED).bg(theme::BG_LIGHT);

    f.render_widget(Paragraph::new("│").style(line_style), body_layout[1]);
    f.render_widget(Paragraph::new("│").style(line_style), body_layout[3]);

    render_uds_status_panel(f, status, body_layout[0]);

    let nvm_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(body_layout[2]);

    render_nvm_counters_panel(f, event, nvm_layout[0]);
    render_debounce_panel(f, event, nvm_layout[1]);

    let calib_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(30),
            Constraint::Percentage(25),
        ])
        .split(body_layout[4]);

    render_debounce_config_panel(f, cal, calib_layout[0]);
    render_thresholds_panel(f, cal, calib_layout[1]);
    render_persistence_panel(f, cal, calib_layout[2]);

    if app.editing_calib {
        let edit_line = Line::from(vec![
            Span::raw("Editing: ").fg(theme::CYAN),
            Span::raw(App::get_calib_field_name(app.editing_field)).bold(),
            Span::raw(" | Value: "),
            Span::raw(app.get_calib_field_value()).fg(theme::YELLOW),
            Span::raw(" | [↑/↓] Change  [Tab] Next  [Esc] Cancel"),
        ]);
        f.render_widget(
            Paragraph::new(edit_line)
                .style(Style::default().bg(theme::BG_LIGHT))
                .alignment(ratatui::layout::Alignment::Center),
            inner_layout[1],
        );
    }
}

fn render_uds_status_panel(f: &mut ratatui::Frame<'_>, status: dem::UdsStatusByte, area: Rect) {
    let rows = vec![
        Row::new(vec![
            Cell::from(Span::raw("TF")),
            Cell::from(Span::raw("Test Failed")),
            flag_cell(status.tf()),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("TFTOC")),
            Cell::from(Span::raw("Failed TOC")),
            flag_cell(status.tftoc()),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("PDTC")),
            Cell::from(Span::raw("Pending DTC")),
            flag_cell(status.pdtc()),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("CDTC")),
            Cell::from(Span::raw("Confirmed DTC")),
            flag_cell(status.cdtc()),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("TFSLC")),
            Cell::from(Span::raw("Failed SLC")),
            flag_cell(status.tfslc()),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("TNCSLC")),
            Cell::from(Span::raw("NC SLC")),
            flag_cell(status.tncslc()),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("TNCTOC")),
            Cell::from(Span::raw("NC This OC")),
            flag_cell(status.tnctoc()),
        ]),
    ];

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Length(8),
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MUTED))
            .title_style(Style::default().fg(theme::PURPLE))
            .title(" UDS Status "),
    );

    f.render_widget(table, area);
}

fn render_nvm_counters_panel(f: &mut ratatui::Frame<'_>, event: &Event, area: Rect) {
    let rows = vec![
        Row::new(vec![
            Cell::from(Span::raw("Occurrence").bold().fg(theme::MUTED)),
            Cell::from(Span::raw(format!("{:>8}", event.nv_config.occurence_cntr))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Confirmation").bold().fg(theme::MUTED)),
            Cell::from(Span::raw(format!(
                "{:>8}",
                event.nv_config.confirmation_cycles
            ))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Aging Cycles").bold().fg(theme::MUTED)),
            Cell::from(Span::raw(format!("{:>8}", event.nv_config.aging_cycles))),
        ]),
    ];

    let table = Table::new(rows, [Constraint::Length(14), Constraint::Length(10)]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MUTED))
            .title_style(Style::default().fg(theme::PURPLE))
            .title(" NVM Counters "),
    );

    f.render_widget(table, area);
}

fn render_debounce_panel(f: &mut ratatui::Frame<'_>, event: &Event, area: Rect) {
    let rows = vec![Row::new(vec![
        Cell::from(Span::raw("Debounce").bold().fg(theme::MUTED)),
        Cell::from(Span::raw(format!("{:>8}", event.debounce_counter())).fg(theme::YELLOW)),
    ])];

    let table = Table::new(rows, [Constraint::Length(14), Constraint::Length(10)]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MUTED))
            .title_style(Style::default().fg(theme::PURPLE))
            .title(" Debounce "),
    );

    f.render_widget(table, area);
}

fn render_debounce_config_panel(f: &mut ratatui::Frame<'_>, cal: &dem::CalibConfig, area: Rect) {
    let rows = vec![
        Row::new(vec![
            Cell::from(Span::raw("step_up")),
            Cell::from(Span::raw(format!("{:>14}", cal.step_up))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("step_down")),
            Cell::from(Span::raw(format!("{:>14}", cal.step_down))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("deb_type")),
            Cell::from(Span::raw(format!(
                "{:>14}",
                format!("{:?}", cal.debounce_type)
            ))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("deb_behav")),
            Cell::from(Span::raw(format!(
                "{:>14}",
                format!("{:?}", cal.debounce_behavior)
            ))),
        ]),
    ];

    let table = Table::new(rows, [Constraint::Length(14), Constraint::Length(16)]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MUTED))
            .title_style(Style::default().fg(theme::PURPLE))
            .title(" Debounce Config "),
    );

    f.render_widget(table, area);
}

fn render_thresholds_panel(f: &mut ratatui::Frame<'_>, cal: &dem::CalibConfig, area: Rect) {
    let rows = vec![
        Row::new(vec![
            Cell::from(Span::raw("confirm")),
            Cell::from(Span::raw(format!("{:>14}", cal.confirmation_threshold))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("aging")),
            Cell::from(Span::raw(format!("{:>14}", cal.aging_threshold))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("priority")),
            Cell::from(Span::raw(format!("{:>14}", cal.priority))),
        ]),
    ];

    let table = Table::new(rows, [Constraint::Length(14), Constraint::Length(16)]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MUTED))
            .title_style(Style::default().fg(theme::PURPLE))
            .title(" Thresholds "),
    );

    f.render_widget(table, area);
}

fn render_persistence_panel(f: &mut ratatui::Frame<'_>, cal: &dem::CalibConfig, area: Rect) {
    let rows = vec![
        Row::new(vec![
            Cell::from(Span::raw("save_trig")),
            Cell::from(Span::raw(format!(
                "{:>14}",
                format!("{:?}", cal.save_trigger)
            ))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("record")),
            Cell::from(Span::raw(format!(
                "{:>14}",
                if cal.record_update { "Yes" } else { "No" }
            ))),
        ]),
    ];

    let table = Table::new(rows, [Constraint::Length(14), Constraint::Length(16)]).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MUTED))
            .title_style(Style::default().fg(theme::PURPLE))
            .title(" Persistence "),
    );

    f.render_widget(table, area);
}

fn flag_cell(value: bool) -> Cell<'static> {
    if value {
        Cell::from(Span::raw("[ON] ").fg(theme::GREEN).bold())
    } else {
        Cell::from(Span::raw("[OFF]").fg(theme::MUTED))
    }
}

fn render_freeze_frames_panel(f: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let ff_list = &app.manager.freeze_frames;

    if ff_list.is_empty() {
        f.render_widget(
            Paragraph::new("No freeze frames stored.\n\nTrigger events to create freeze frames.")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme::MUTED))
                        .title_style(Style::default().fg(theme::PURPLE))
                        .title(" Freeze Frames "),
                ),
            area,
        );
        return;
    }

    let selected_idx = app
        .selected_ff
        .unwrap_or(0)
        .min(ff_list.len().saturating_sub(1));

    let rows: Vec<Row> = ff_list
        .iter()
        .enumerate()
        .map(|(i, ff)| {
            let style = if i == selected_idx {
                Style::default().bg(theme::SELECTION)
            } else {
                Style::default()
            };

            let snapshot_values: Vec<String> = if app.show_raw_snapshot {
                vec![
                    format!("{:02X}", ff.snapshot_data[0]),
                    format!(
                        "{:04X}",
                        u16::from_le_bytes([ff.snapshot_data[1], ff.snapshot_data[2]])
                    ),
                    format!(
                        "{:04X}",
                        u16::from_le_bytes([ff.snapshot_data[3], ff.snapshot_data[4]])
                    ),
                    format!("{:02X}", ff.snapshot_data[5]),
                    format!("{:02X}", ff.snapshot_data[6]),
                    format!("{:02X}", ff.snapshot_data[7]),
                    format!("{:02X}", ff.snapshot_data[10]),
                    format!(
                        "{:04X}",
                        u16::from_le_bytes([ff.snapshot_data[11], ff.snapshot_data[12]])
                    ),
                    format!("{:02X}", ff.snapshot_data[13]),
                    format!("{:02X}", ff.snapshot_data[14]),
                    format!(
                        "{:04X}",
                        u16::from_le_bytes([ff.snapshot_data[15], ff.snapshot_data[16]])
                    ),
                    format!("{:02X}", ff.snapshot_data[17]),
                    format!("{:02X}", ff.snapshot_data[18]),
                ]
            } else {
                vec![
                    format!("{:.1}", ff.snapshot_data[0] as f32 / 10.0),
                    format!(
                        "{}",
                        u16::from_le_bytes([ff.snapshot_data[1], ff.snapshot_data[2]])
                    ),
                    format!(
                        "{}",
                        u16::from_le_bytes([ff.snapshot_data[3], ff.snapshot_data[4]])
                    ),
                    format!("{}", ff.snapshot_data[5]),
                    format!("{}", ff.snapshot_data[6]),
                    format!("{}", ff.snapshot_data[7]),
                    format!("{}", ff.snapshot_data[10]),
                    format!(
                        "{}",
                        u16::from_le_bytes([ff.snapshot_data[11], ff.snapshot_data[12]])
                    ),
                    format!("{}", ff.snapshot_data[13]),
                    format!("{}", ff.snapshot_data[14]),
                    format!(
                        "{}",
                        u16::from_le_bytes([ff.snapshot_data[15], ff.snapshot_data[16]])
                    ),
                    format!("{}", ff.snapshot_data[17]),
                    format!("{}", ff.snapshot_data[18]),
                ]
            };

            let mut row_data = vec![
                format!("{}[{}]", EVENT_NAMES[ff.event_id as usize], ff.event_id),
                format!("{}", ff.priority),
                format!("{}", ff.first_occurrence_time),
                format!("{}", ff.last_occurrence_time),
            ];
            row_data.extend(snapshot_values);

            Row::new(row_data).style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec![
            "Event",
            "Pri",
            "first",
            "last",
            "Bat(V)",
            "RPM",
            "Spd(km/h)",
            "Cool(C)",
            "Intk(C)",
            "Thr(%)",
            "Fuel(%)",
            "Oil(kPa)",
            "Trans(C)",
            "Exh(C)",
            "MAF(g/s)",
            "Load(%)",
            "FP(kPa)",
        ])
        .style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme::PURPLE),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::MUTED))
            .title_style(Style::default().fg(theme::CYAN))
            .title(" Freeze Frames "),
    );

    let inner_area = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(4));
    f.render_widget(table, inner_area);

    let show_snapshot_detail = if ff_list.len() == 1 {
        ff_list.iter().next()
    } else {
        app.selected_ff
            .and_then(|selected| ff_list.get_by_event_id(selected as EventId))
    };

    if let Some(ff) = show_snapshot_detail {
        let hex_lines = format_hex_dump(&ff.snapshot_data, 64);
        let hex_lines_len = hex_lines.len();
        let paragraph = Paragraph::new(hex_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::MUTED))
                    .title_style(Style::default().fg(theme::CYAN))
                    .title(format!(
                        " Snapshot Data - {}[{}] ",
                        EVENT_NAMES[ff.event_id as usize], ff.event_id
                    )),
            )
            .scroll((app.ff_scroll as u16, 0));

        let detail_area = Rect::new(
            area.x,
            area.y + area.height.saturating_sub(5),
            area.width,
            4,
        );
        f.render_widget(paragraph, detail_area);

        let mut scroll_state = ScrollbarState::new(hex_lines_len).position(app.ff_scroll);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).thumb_symbol("█"),
            detail_area,
            &mut scroll_state,
        );
    }

    let hint = Line::from(vec![
        Span::raw(" [↑/↓] Select FF  "),
        Span::raw("[R] Raw/Phys  "),
        Span::raw("[F] Close panel"),
    ])
    .style(Style::default().fg(theme::MUTED));

    let hint_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(1),
        area.width,
        1,
    );
    f.render_widget(Paragraph::new(hint), hint_area);
}

fn render_footer(f: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let status = if app.manager.state == EventManagerState::On {
        "RUNNING"
    } else {
        "STOPPED"
    };
    let status_color = if app.manager.state == EventManagerState::On {
        theme::GREEN
    } else {
        theme::RED
    };

    let line = if app.editing_calib {
        Line::from(vec![
            Span::raw("[").fg(theme::MUTED),
            Span::raw(status).fg(status_color).bold(),
            Span::raw("] ").fg(theme::MUTED),
            Span::raw(format!("Timestamp: {:04}  ", *app.manager.timestamp)),
            Span::raw("| ").fg(theme::MUTED),
            Span::raw("Editing: ").fg(theme::CYAN),
            Span::raw(format!("Event {} - ", app.viewed_event)),
            Span::raw(App::get_calib_field_name(app.editing_field)).bold(),
            Span::raw(" | Value: ").fg(theme::MUTED),
            Span::raw(app.get_calib_field_value()).fg(theme::YELLOW),
            Span::raw(" | Tab=Next  Esc=Cancel").fg(theme::MUTED),
        ])
    } else {
        Line::from(vec![
            Span::raw("[").fg(theme::MUTED),
            Span::raw(status).fg(status_color).bold(),
            Span::raw("] ").fg(theme::MUTED),
            Span::raw(format!("Timestamp: {:04}  ", *app.manager.timestamp)),
            Span::raw("| Last: ").fg(theme::MUTED),
            Span::raw(&app.last_action),
            Span::raw(" | ").fg(theme::MUTED),
            Span::raw("[Space] Select  [1] PreFail  [2] Fail  [3] PrePass  [4] Pass  [E] Edit  [I] Init  [S] Stop  [N] Cycle  [C] Clear  [F] FF  [?] Help  [Q] Quit"),
        ])
    };

    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::BG_LIGHT)),
        area,
    );
}

fn format_hex_dump<'a>(data: &'a [u8], max_bytes: usize) -> Vec<Line<'a>> {
    let display_data = &data[..max_bytes.min(data.len())];
    display_data
        .chunks(16)
        .enumerate()
        .map(|(i, chunk)| {
            let hex: String = chunk
                .iter()
                .enumerate()
                .map(|(j, b)| {
                    if j == 7 {
                        format!("{:02X} ", b)
                    } else {
                        format!("{:02X} ", b)
                    }
                })
                .collect();

            let ascii: String = chunk
                .iter()
                .map(|&b| {
                    if b.is_ascii_graphic() || b == b' ' {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();

            Line::from(vec![
                Span::raw(format!("{:04X}: ", i * 16)),
                Span::raw(format!("{:<47}", hex)),
                Span::raw(" |").fg(theme::MUTED),
                Span::raw(format!(" {}", ascii)).fg(theme::YELLOW),
                Span::raw("|").fg(theme::MUTED),
            ])
        })
        .collect()
}

fn render_help_overlay(f: &mut ratatui::Frame<'_>, app: &App) {
    if !app.show_help {
        return;
    }

    let area = f.area();
    let width = 48;
    let height = 26;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;

    let help_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, help_area);

    let border_style = Style::default().fg(theme::CYAN);
    let title_style = Style::default()
        .fg(theme::PURPLE)
        .add_modifier(Modifier::BOLD);

    let lines = vec![
        Line::from(vec![Span::raw("KEYBOARD SHORTCUTS")
            .bold()
            .fg(theme::PURPLE)]),
        Line::from(""),
        Line::from(vec![Span::raw("Event Triggering")
            .bold()
            .underlined()
            .fg(theme::BLUE)]),
        Line::from(vec![
            Span::raw("  [Space]        ").fg(theme::YELLOW),
            Span::raw("Select event for triggering (orange)"),
        ]),
        Line::from(vec![
            Span::raw("  [1]            ").fg(theme::YELLOW),
            Span::raw("Trigger selected event (PreFailed)"),
        ]),
        Line::from(vec![
            Span::raw("  [2]            ").fg(theme::YELLOW),
            Span::raw("Trigger selected event (Failed)"),
        ]),
        Line::from(vec![
            Span::raw("  [3]            ").fg(theme::YELLOW),
            Span::raw("Trigger selected event (PrePassed)"),
        ]),
        Line::from(vec![
            Span::raw("  [4]            ").fg(theme::YELLOW),
            Span::raw("Trigger selected event (Passed)"),
        ]),
        Line::from(""),
        Line::from(vec![Span::raw("Cycle Control")
            .bold()
            .underlined()
            .fg(theme::BLUE)]),
        Line::from(vec![
            Span::raw("  [I]            ").fg(theme::YELLOW),
            Span::raw("Initialize cycle"),
        ]),
        Line::from(vec![
            Span::raw("  [S]            ").fg(theme::YELLOW),
            Span::raw("Stop cycle"),
        ]),
        Line::from(vec![
            Span::raw("  [N]            ").fg(theme::YELLOW),
            Span::raw("Next cycle (stop + init)"),
        ]),
        Line::from(""),
        Line::from(vec![Span::raw("Data Management")
            .bold()
            .underlined()
            .fg(theme::BLUE)]),
        Line::from(vec![
            Span::raw("  [C]            ").fg(theme::YELLOW),
            Span::raw("Clear events & freeze frames"),
        ]),
        Line::from(vec![
            Span::raw("  [F]            ").fg(theme::YELLOW),
            Span::raw("Toggle freeze frames panel"),
        ]),
        Line::from(vec![
            Span::raw("  [R]            ").fg(theme::YELLOW),
            Span::raw("Toggle raw/physical snapshot view"),
        ]),
        Line::from(""),
        Line::from(vec![Span::raw("Navigation")
            .bold()
            .underlined()
            .fg(theme::BLUE)]),
        Line::from(vec![
            Span::raw("  [↑/↓]          ").fg(theme::YELLOW),
            Span::raw("Navigate (view details, yellow highlight)"),
        ]),
        Line::from(vec![
            Span::raw("  [PgUp/PgDn]    ").fg(theme::YELLOW),
            Span::raw("Scroll details panel"),
        ]),
        Line::from(""),
        Line::from(vec![Span::raw("Editing")
            .bold()
            .underlined()
            .fg(theme::BLUE)]),
        Line::from(vec![
            Span::raw("  [E]            ").fg(theme::YELLOW),
            Span::raw("Edit CalibConfig for event"),
        ]),
        Line::from(vec![
            Span::raw("  [↑/↓]          ").fg(theme::YELLOW),
            Span::raw("Change value (in edit mode)"),
        ]),
        Line::from(vec![
            Span::raw("  [Tab]          ").fg(theme::YELLOW),
            Span::raw("Next field (in edit mode)"),
        ]),
        Line::from(vec![
            Span::raw("  [Esc]          ").fg(theme::YELLOW),
            Span::raw("Cancel / Close"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  [Q]            ").fg(theme::RED),
            Span::raw("Quit"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Press Esc or any key to close").fg(theme::MUTED)
        ]),
    ];

    f.render_widget(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(border_style)
            .title_style(title_style),
        help_area,
    );

    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(theme::FG))
            .scroll((0, 0)),
        help_area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        }),
    );
}

fn main() -> Result<(), io::Error> {
    let mut app = App::new();
    app.manager.clear();
    let mut last_tick = Instant::now();

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    terminal.clear()?;

    loop {
        if app.manager.state == EventManagerState::On {
            if last_tick.elapsed() >= Duration::from_secs(1) {
                *app.manager.timestamp += 1;
                unsafe {
                    SIMULATION_TICK += 1;
                    update_live_system_state(SIMULATION_TICK);
                }
                last_tick = Instant::now();
            }
        }

        terminal.draw(|f| {
            render_ui(f, &app);
            render_help_overlay(f, &app);
        })?;

        if crossterm::event::poll(Duration::from_millis(100))? {
            match crossterm::event::read()? {
                crossterm::event::Event::Key(key) => {
                    if app.show_help {
                        app.show_help = false;
                    } else if app.editing_calib {
                        handle_calib_edit_input(&mut app, key);
                    } else if !handle_key_event(&mut app, key) {
                        break;
                    }
                }
                crossterm::event::Event::Mouse(_) => {}
                crossterm::event::Event::Resize(_, _) => {}
                crossterm::event::Event::Paste(_)
                | crossterm::event::Event::FocusGained
                | crossterm::event::Event::FocusLost => {}
            }
        }
    }

    execute!(io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    terminal.clear()?;
    println!("Goodbye!");
    Ok(())
}

fn handle_key_event(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return false,

        KeyCode::Char('?') | KeyCode::Char('h') | KeyCode::Char('H') => {
            app.show_help = !app.show_help;
        }

        KeyCode::Char('1') => {
            if let Some(event_id) = app.trigger_event {
                app.trigger_event(event_id, Status::PreFailed);
            } else {
                app.last_action = "Press Space to select an event first".to_string();
            }
        }

        KeyCode::Char('2') => {
            if let Some(event_id) = app.trigger_event {
                app.trigger_event(event_id, Status::Failed);
            } else {
                app.last_action = "Press Space to select an event first".to_string();
            }
        }

        KeyCode::Char('3') => {
            if let Some(event_id) = app.trigger_event {
                app.trigger_event(event_id, Status::PrePassed);
            } else {
                app.last_action = "Press Space to select an event first".to_string();
            }
        }

        KeyCode::Char('4') => {
            if let Some(event_id) = app.trigger_event {
                app.trigger_event(event_id, Status::Passed);
            } else {
                app.last_action = "Press Space to select an event first".to_string();
            }
        }

        KeyCode::Char(' ') => {
            app.trigger_event = Some(app.viewed_event);
            app.last_action = format!("Selected Event {} for triggering", app.viewed_event);
        }

        KeyCode::Char('i') | KeyCode::Char('I') => {
            app.init_cycle();
        }

        KeyCode::Char('s') | KeyCode::Char('S') => {
            app.stop_cycle();
        }

        KeyCode::Char('n') | KeyCode::Char('N') => {
            app.next_cycle();
        }

        KeyCode::Char('c') | KeyCode::Char('C') => {
            app.clear_all();
        }

        KeyCode::Char('f') | KeyCode::Char('F') => {
            app.show_freeze_frames = !app.show_freeze_frames;
            app.selected_ff = None;
            app.ff_scroll = 0;
            app.last_action = if app.show_freeze_frames {
                "Freeze frames panel opened".to_string()
            } else {
                "Freeze frames panel closed".to_string()
            };
        }

        KeyCode::Char('r') | KeyCode::Char('R') => {
            if app.show_freeze_frames {
                app.show_raw_snapshot = !app.show_raw_snapshot;
                app.last_action = if app.show_raw_snapshot {
                    "Showing raw snapshot data".to_string()
                } else {
                    "Showing physical snapshot data".to_string()
                };
            }
        }

        KeyCode::Char('e') | KeyCode::Char('E') => {
            if app.manager.events.len() > 0 {
                app.editing_calib = true;
                app.editing_field = 0;
                app.last_action = format!(
                    "Editing {} for Event {}",
                    App::get_calib_field_name(0),
                    app.viewed_event
                );
            }
        }

        KeyCode::Up => {
            if app.show_freeze_frames {
                if let Some(selected) = app.selected_ff {
                    if selected > 0 {
                        app.selected_ff = Some(selected - 1);
                        app.ff_scroll = 0;
                    }
                } else {
                    app.selected_ff = Some(0);
                }
            } else {
                if app.viewed_event > 0 {
                    app.viewed_event -= 1;
                    app.scroll = 0;
                }
            }
        }

        KeyCode::Down => {
            if app.show_freeze_frames {
                let max = app.manager.freeze_frames.len().saturating_sub(1);
                if let Some(selected) = app.selected_ff {
                    if selected < max {
                        app.selected_ff = Some(selected + 1);
                        app.ff_scroll = 0;
                    }
                } else if max > 0 {
                    app.selected_ff = Some(0);
                }
            } else {
                if app.viewed_event < app.manager.events.len() - 1 {
                    app.viewed_event += 1;
                    app.scroll = 0;
                }
            }
        }

        KeyCode::PageUp => {
            if app.scroll > 10 {
                app.scroll -= 10;
            } else {
                app.scroll = 0;
            }
        }

        KeyCode::PageDown => {
            app.scroll += 10;
        }

        _ => {}
    }

    true
}

fn handle_calib_edit_input(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.editing_calib = false;
            app.last_action = "Calib edit cancelled".to_string();
        }

        KeyCode::Tab => {
            app.editing_field = (app.editing_field + 1) % 9;
            app.last_action = format!(
                "Editing {} for Event {}",
                App::get_calib_field_name(app.editing_field),
                app.viewed_event
            );
        }

        KeyCode::Up => {
            app.adjust_calib_value(true);
        }

        KeyCode::Down => {
            app.adjust_calib_value(false);
        }

        _ => {}
    }
}
