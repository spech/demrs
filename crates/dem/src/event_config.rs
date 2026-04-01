// ─────────────────────────────────────────────
// Event Configuration
// ─────────────────────────────────────────────

/// Controls the behavior of the debounce counter when the event is disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebounceBehavior {
    /// Leave the `debounce_counter` unchanged while disabled.
    Freeze,
    /// Reset the `debounce_counter` to zero while disabled.
    Reset,
}

/// Specifies how the debounce counter is incremented/decremented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebounceType {
    /// `step_up`/`step_down` represent the number of ticks needed to reach max/min.
    CounterBased,
    /// `step_up`/`step_down` represent the time needed to reach max/min (requires sampling period).
    TimeBased,
}

/// Trigger condition for saving extended records.
///
/// Determines when an event's extended record should be created or updated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveTrigger {
    /// Save extended record when `cdtc` (confirmed DTC) is set.
    OnCdtc,
    /// Save extended record when `pdtc` (pending DTC) is set.
    OnPdtc,
}

/// Calibration configuration for [`Event`](crate::event::Event).
///
/// This struct holds step counts, debounce behavior, and type,
/// allowing a single configuration to be shared across multiple events.
#[derive(Clone)]
pub struct CalibConfig {
    /// Number of `PreFailed` ticks to reach `i16::MAX`.
    /// - `0`: Counter unchanged (no debouncing).
    /// - `1`: Immediate snap to confirmed.
    /// - `n > 1`: Gradual accumulation.
    pub step_up: i16,
    /// Number of `PrePassed` ticks to reach `i16::MIN`.
    /// - `0`: Counter unchanged.
    /// - `1`: Immediate snap.
    /// - `n > 1`: Gradual accumulation.
    pub step_down: i16,
    /// Behavior when the event is disabled.
    pub debounce_behavior: DebounceBehavior,
    /// How the counter is changed (tick-based or time-based).
    pub debounce_type: DebounceType,
    /// Number of cycles where failure must be confirmed before setting `cdtc`.
    pub confirmation_threshold: u8,
    /// Number of aging cycles before clearing `cdtc`.
    pub aging_threshold: u8,
    /// Priority of this event in the ordered list.
    pub priority: u8,
    /// Trigger condition for saving extended records.
    pub save_trigger: SaveTrigger,
}
