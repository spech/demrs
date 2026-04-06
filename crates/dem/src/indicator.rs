//! Indicator lamp module.
//!
//! Provides lamp control for J1939 standard lamps: MIL, RSL, AWL, and PL.
//! Each lamp can be configured with different behaviors and manages blinking patterns.
//!
//! # Lamp Types
//! - `LampId::Mil` - Malfunction Indicator Lamp
//! - `LampId::Rsl` - Red Stop Lamp
//! - `LampId::Awl` - Amber Warning Lamp
//! - `LampId::Pl` - Protect Lamp
//!
//! # Lamp Behaviors
//! - `On` - Continuous (highest priority)
//! - `ShortFlash` - 3 flashes at 4Hz then 200ms pause
//! - `SlowBlink` - Slow flash at 1 Hz
//! - `FastBlink` - Fast flash at 4 Hz
//! - `Off` - No indication (lowest priority)

/// Lamp type identifiers following J1939 standard.
///
/// Represents the physical lamp types in the vehicle dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LampId {
    /// Malfunction Indicator Lamp - indicates powertrain/system faults
    Mil,
    /// Red Stop Lamp - indicates critical fault requiring immediate stop
    Rsl,
    /// Amber Warning Lamp - indicates non-critical warning
    Awl,
    /// Protect Lamp - indicates protection/protective action active
    Pl,
}

impl LampId {
    /// Converts LampId to array index.
    ///
    /// # Index Mapping
    /// - `Mil` = 0
    /// - `Rsl` = 1
    /// - `Awl` = 2
    /// - `Pl` = 3
    pub fn to_index(&self) -> usize {
        match self {
            LampId::Mil => 0,
            LampId::Rsl => 1,
            LampId::Awl => 2,
            LampId::Pl => 3,
        }
    }
}

/// Lamp behavior modes.
///
/// Priority (highest to lowest):
/// - `On` (4) - Continuous
/// - `ShortFlash` (3) - Short flashing (3 flashes at 4Hz then 200ms pause)
/// - `SlowBlink` (2) - Slow flash (1 Hz)
/// - `FastBlink` (1) - Fast flash (4 Hz)
/// - `Off` (0) - No indication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LampBehavior {
    #[default]
    Off = 0,
    FastBlink = 1,
    SlowBlink = 2,
    ShortFlash = 3,
    On = 4,
}

impl LampBehavior {
    pub fn to_index(&self) -> usize {
        *self as usize
    }
}

/// Individual lamp state and configuration.
#[derive(Debug, Clone, Copy, Default)]
pub struct IndicatorLamp {
    /// The behavior of this lamp.
    pub behavior: LampBehavior,
    /// Counters for each behavior [Off, FastBlink, SlowBlink, ShortFlash, On].
    pub counters: [u16; 5],
    /// Current blink state (true = on, false = off).
    pub state: bool,
    /// Internal tick counter for blink pattern tracking.
    pub tick_counter: u32,
}

/// Blink timing constants (at 10ms per tick).
mod timing {
    /// SlowBlink: 1 Hz (100 ticks per cycle, 50 on / 50 off)
    pub const SLOW_BLINK_CYCLE: u32 = 100;
    pub const SLOW_BLINK_ON: u32 = 50;

    /// FastBlink: 4 Hz (25 ticks per cycle, 12 on / 13 off)
    pub const FAST_BLINK_CYCLE: u32 = 25;
    pub const FAST_BLINK_ON: u32 = 12;

    /// ShortFlash: 3 flashes at 4Hz (75 ticks) then 200ms pause (20 ticks)
    pub const SHORT_FLASH_CYCLE: u32 = 56; // 36 ticks (3x12) + 20 ticks pause
    pub const SHORT_FLASH_ON: u32 = 36; // 3 flashes x 12 ticks
}

impl IndicatorLamp {
    /// Clears the lamp state, resetting all counters and behavior.
    pub fn clear(&mut self) {
        self.counters = [0; 5];
        self.behavior = LampBehavior::Off;
        self.state = false;
        self.tick_counter = 0;
    }

    /// Updates the lamp state.
    ///
    /// Only updates behavior if incoming behavior has higher priority.
    /// Counter for the specified behavior is incremented if active, decremented otherwise.
    pub fn update(&mut self, behavior: LampBehavior, active: bool) {
        let behavior_index = behavior.to_index();

        // Update counter for the specified behavior
        if active {
            self.counters[behavior_index] = self.counters[behavior_index].saturating_add(1);
        } else {
            self.counters[behavior_index] = self.counters[behavior_index].saturating_sub(1);
        }

        // Only update behavior to the highest counter value
        // Off is last to ensure it's only selected if all other counters are 0
        let old_behavior = self.behavior;
        let mut highest_behavior = LampBehavior::Off;
        let mut highest_count = 0u16;

        for b in [
            LampBehavior::FastBlink,
            LampBehavior::SlowBlink,
            LampBehavior::ShortFlash,
            LampBehavior::On,
            LampBehavior::Off,
        ] {
            let count = self.counters[b.to_index()];
            if count > highest_count {
                highest_count = count;
                highest_behavior = b;
            }
        }

        self.behavior = highest_behavior;

        if self.behavior != old_behavior {
            self.tick_counter = 0;
        }
    }

    /// Handles blink pattern for this lamp.
    ///
    /// Called every 10ms to update the blink state based on current behavior.
    pub fn handler_10ms(&mut self) {
        match self.behavior {
            LampBehavior::On => self.state = true,
            LampBehavior::Off => self.state = false,
            LampBehavior::SlowBlink => {
                let cycle_position = self.tick_counter % timing::SLOW_BLINK_CYCLE;
                self.state = cycle_position < timing::SLOW_BLINK_ON;
                self.tick_counter = (self.tick_counter + 1) % timing::SLOW_BLINK_CYCLE;
            }
            LampBehavior::FastBlink => {
                let cycle_position = self.tick_counter % timing::FAST_BLINK_CYCLE;
                self.state = cycle_position < timing::FAST_BLINK_ON;
                self.tick_counter = (self.tick_counter + 1) % timing::FAST_BLINK_CYCLE;
            }
            LampBehavior::ShortFlash => {
                let cycle_position = self.tick_counter % timing::SHORT_FLASH_CYCLE;
                if cycle_position < timing::SHORT_FLASH_ON {
                    let flash_position = cycle_position % 12;
                    self.state = flash_position < 6; // 6 ON, 6 OFF per flash
                } else {
                    self.state = false; // Pause
                }
                self.tick_counter = (self.tick_counter + 1) % timing::SHORT_FLASH_CYCLE;
            }
        }
    }
}

/// Container for 4 lamps (Mil, Rsl, Awl, Pl).
#[derive(Debug, Clone, Copy)]
pub struct IndicatorLamps {
    lamps: [IndicatorLamp; 4],
}

impl Default for IndicatorLamps {
    fn default() -> Self {
        Self::new()
    }
}

impl IndicatorLamps {
    /// Creates a new IndicatorLamps with all lamps set to Off.
    pub const fn new() -> Self {
        Self {
            lamps: [
                IndicatorLamp {
                    behavior: LampBehavior::Off,
                    counters: [0; 5],
                    state: false,
                    tick_counter: 0,
                },
                IndicatorLamp {
                    behavior: LampBehavior::Off,
                    counters: [0; 5],
                    state: false,
                    tick_counter: 0,
                },
                IndicatorLamp {
                    behavior: LampBehavior::Off,
                    counters: [0; 5],
                    state: false,
                    tick_counter: 0,
                },
                IndicatorLamp {
                    behavior: LampBehavior::Off,
                    counters: [0; 5],
                    state: false,
                    tick_counter: 0,
                },
            ],
        }
    }
    /// Updates all lamps based on active status and their configured behaviors.
    ///
    /// # Arguments
    ///
    /// * `behaviors` - Array of behaviors for each lamp [Mil, Rsl, Awl, Pl]
    /// * `active` - Whether the lamp should be active
    pub fn update_all(&mut self, behaviors: &[LampBehavior; 4], active: bool) {
        for (i, lamp) in self.lamps.iter_mut().enumerate() {
            lamp.update(behaviors[i], active);
        }
    }

    /// Clears all lamp states, resetting all counters and behaviors.
    pub fn clear_all(&mut self) {
        for lamp in &mut self.lamps {
            lamp.clear();
        }
    }

    /// Calls handler_10ms on all lamps.
    pub fn handler_10ms_all(&mut self) {
        for lamp in &mut self.lamps {
            lamp.handler_10ms();
        }
    }

    /// Gets lamp state by LampId.
    pub fn is_on(&self, lamp_id: LampId) -> bool {
        self.lamps[lamp_id.to_index()].state
    }

    /// Gets lamp behavior by LampId.
    pub fn get_behavior(&self, lamp_id: LampId) -> LampBehavior {
        self.lamps[lamp_id.to_index()].behavior
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fn_update_increment_counter_when_active_is_true() {
        let mut lamp = IndicatorLamp {
            behavior: LampBehavior::Off,
            counters: [0; 5],
            state: false,
            tick_counter: 0,
        };

        lamp.update(LampBehavior::FastBlink, true);
        assert_eq!(lamp.counters[1], 1);

        lamp.update(LampBehavior::FastBlink, true);
        assert_eq!(lamp.counters[1], 2);
    }

    #[test]
    fn fn_update_decrement_counter_when_active_is_false() {
        let mut lamp = IndicatorLamp {
            behavior: LampBehavior::Off,
            counters: [0; 5],
            state: false,
            tick_counter: 0,
        };

        lamp.update(LampBehavior::FastBlink, true);
        lamp.update(LampBehavior::FastBlink, true);
        assert_eq!(lamp.counters[1], 2);

        lamp.update(LampBehavior::FastBlink, false);
        assert_eq!(lamp.counters[1], 1);

        lamp.update(LampBehavior::FastBlink, false);
        assert_eq!(lamp.counters[1], 0);
    }

    #[test]
    fn fn_handler_10ms_on_always_on() {
        let mut lamp = IndicatorLamp {
            behavior: LampBehavior::On,
            counters: [0; 5],
            state: false,
            tick_counter: 0,
        };

        for _ in 0..10 {
            lamp.handler_10ms();
            assert!(lamp.state);
        }
    }

    #[test]
    fn fn_handler_10ms_off_always_off() {
        let mut lamp = IndicatorLamp {
            behavior: LampBehavior::Off,
            counters: [0; 5],
            state: true,
            tick_counter: 0,
        };

        for _ in 0..10 {
            lamp.handler_10ms();
            assert!(!lamp.state);
        }
    }

    #[test]
    fn fn_handler_10ms_slow_blink_alternates() {
        let mut lamp = IndicatorLamp {
            behavior: LampBehavior::SlowBlink,
            counters: [0; 5],
            state: false,
            tick_counter: 0,
        };

        // First 50 ticks should be on
        for _ in 0..50 {
            lamp.handler_10ms();
            assert!(lamp.state, "First 50 ticks should be on");
        }

        // Next 50 ticks should be off
        for _ in 0..50 {
            lamp.handler_10ms();
            assert!(!lamp.state, "Next 50 ticks should be off");
        }

        // Should be on again (cycle repeats)
        lamp.handler_10ms();
        assert!(lamp.state);
    }

    #[test]
    fn fn_handler_10ms_fast_blink_alternates() {
        let mut lamp = IndicatorLamp {
            behavior: LampBehavior::FastBlink,
            counters: [0; 5],
            state: false,
            tick_counter: 0,
        };

        // First 12 ticks should be on
        for _ in 0..12 {
            lamp.handler_10ms();
            assert!(lamp.state, "First 12 ticks should be on");
        }

        // Next 13 ticks should be off
        for _ in 0..13 {
            lamp.handler_10ms();
            assert!(!lamp.state, "Next 13 ticks should be off");
        }

        // Should be on again (cycle repeats)
        lamp.handler_10ms();
        assert!(lamp.state);
    }

    #[test]
    fn fn_handler_10ms_short_flash_pattern() {
        let mut lamp = IndicatorLamp {
            behavior: LampBehavior::ShortFlash,
            counters: [0; 5],
            state: false,
            tick_counter: 0,
        };

        // ShortFlash: 3 flashes (6 ON, 6 OFF each) then 200ms pause (20 OFF)
        // Total cycle = 56 ticks
        // Flash 1: ticks 0-5 ON, 6-11 OFF
        // Flash 2: ticks 12-17 ON, 18-23 OFF
        // Flash 3: ticks 24-29 ON, 30-35 OFF
        // Pause: ticks 36-55 OFF (20 ticks)

        for tick in 0..56 {
            lamp.handler_10ms();
            let expected_on = match tick {
                0..=5 | 12..=17 | 24..=29 => true,
                _ => false,
            };
            assert_eq!(
                lamp.state,
                expected_on,
                "Tick {} should be {}",
                tick,
                if expected_on { "on" } else { "off" }
            );
        }
    }

    #[test]
    fn fn_indicator_lamps_is_on_returns_correct_state() {
        let mut lamps = IndicatorLamps::default();
        let behaviors = [
            LampBehavior::On,
            LampBehavior::Off,
            LampBehavior::Off,
            LampBehavior::Off,
        ];

        lamps.update_all(&behaviors, true);
        lamps.handler_10ms_all();

        assert!(lamps.is_on(LampId::Mil));
        assert!(!lamps.is_on(LampId::Rsl));
        assert!(!lamps.is_on(LampId::Awl));
        assert!(!lamps.is_on(LampId::Pl));
    }

    #[test]
    fn fn_indicator_lamps_update_all_updates_all_lamps() {
        let mut lamps = IndicatorLamps::default();
        let behaviors = [
            LampBehavior::On,
            LampBehavior::FastBlink,
            LampBehavior::Off,
            LampBehavior::SlowBlink,
        ];

        lamps.update_all(&behaviors, true);

        assert_eq!(lamps.lamps[0].behavior, LampBehavior::On);
        assert_eq!(lamps.lamps[1].behavior, LampBehavior::FastBlink);
        assert_eq!(lamps.lamps[2].behavior, LampBehavior::Off);
        assert_eq!(lamps.lamps[3].behavior, LampBehavior::SlowBlink);
    }

    #[test]
    fn fn_indicator_lamps_handler_10ms_all_updates_all_lamps() {
        let mut lamps = IndicatorLamps::default();
        let behaviors = [
            LampBehavior::SlowBlink,
            LampBehavior::FastBlink,
            LampBehavior::On,
            LampBehavior::Off,
        ];

        lamps.update_all(&behaviors, true);

        lamps.handler_10ms_all();

        assert!(lamps.is_on(LampId::Mil));
        assert!(lamps.is_on(LampId::Rsl));
        assert!(lamps.is_on(LampId::Awl));
        assert!(!lamps.is_on(LampId::Pl));
    }

    #[test]
    fn fn_clear_resets_lamp_state() {
        let mut lamp = IndicatorLamp {
            behavior: LampBehavior::On,
            counters: [10, 20, 30, 40, 50],
            state: true,
            tick_counter: 100,
        };

        lamp.clear();

        assert_eq!(lamp.behavior, LampBehavior::Off);
        assert_eq!(lamp.counters, [0; 5]);
        assert!(!lamp.state);
        assert_eq!(lamp.tick_counter, 0);
    }

    #[test]
    fn fn_clear_all_resets_all_lamps() {
        let mut lamps = IndicatorLamps::default();
        let behaviors = [
            LampBehavior::On,
            LampBehavior::FastBlink,
            LampBehavior::SlowBlink,
            LampBehavior::ShortFlash,
        ];

        lamps.update_all(&behaviors, true);
        lamps.handler_10ms_all();

        assert!(lamps.is_on(LampId::Mil));
        assert!(lamps.is_on(LampId::Rsl));
        assert!(lamps.is_on(LampId::Awl));
        assert!(lamps.is_on(LampId::Pl));

        lamps.clear_all();

        assert!(!lamps.is_on(LampId::Mil));
        assert!(!lamps.is_on(LampId::Rsl));
        assert!(!lamps.is_on(LampId::Awl));
        assert!(!lamps.is_on(LampId::Pl));
        assert_eq!(lamps.lamps[0].behavior, LampBehavior::Off);
        assert_eq!(lamps.lamps[1].behavior, LampBehavior::Off);
        assert_eq!(lamps.lamps[2].behavior, LampBehavior::Off);
        assert_eq!(lamps.lamps[3].behavior, LampBehavior::Off);
    }

    #[test]
    fn fn_get_behavior_returns_correct_behavior() {
        let mut lamps = IndicatorLamps::default();
        let behaviors = [
            LampBehavior::On,
            LampBehavior::FastBlink,
            LampBehavior::SlowBlink,
            LampBehavior::ShortFlash,
        ];

        lamps.update_all(&behaviors, true);

        assert_eq!(lamps.get_behavior(LampId::Mil), LampBehavior::On);
        assert_eq!(lamps.get_behavior(LampId::Rsl), LampBehavior::FastBlink);
        assert_eq!(lamps.get_behavior(LampId::Awl), LampBehavior::SlowBlink);
        assert_eq!(lamps.get_behavior(LampId::Pl), LampBehavior::ShortFlash);

        lamps.clear_all();

        assert_eq!(lamps.get_behavior(LampId::Mil), LampBehavior::Off);
        assert_eq!(lamps.get_behavior(LampId::Rsl), LampBehavior::Off);
        assert_eq!(lamps.get_behavior(LampId::Awl), LampBehavior::Off);
        assert_eq!(lamps.get_behavior(LampId::Pl), LampBehavior::Off);
    }
}
