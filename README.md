# DEM - Diagnostic Event Manager

A Rust implementation of diagnostic event management following UDS principles,
with support for time-based and counter-based debouncing, DTC tracking, and
priority-ordered event processing.

## What is DEM?

**DEM** (Diagnostic Event Manager) is a core component in automotive diagnostics that
manages the lifecycle of diagnostic events. It implements debouncing algorithms to
filter spurious signals and tracks the status of diagnostic trouble codes (DTCs).

### UDS Overview

**UDS** (Unified Diagnostic Services) is a standardized protocol for automotive
diagnostics defined in ISO 14229. It defines how ECUs report and manage diagnostic
information, including:

- **DTCs (Diagnostic Trouble Codes)** - Identifiers for specific fault conditions
- **Debouncing** - Filtering transient signals before confirming a fault
- **Status Tracking** - Monitoring DTC lifecycle (pending, confirmed, aging)

### Debouncing Concepts

Debouncing prevents immediate fault confirmation when a signal fluctuates. Instead
of confirming on a single bad reading, the system requires multiple consecutive
readings to confirm a fault:

- **PreFailed / PrePassed** - Transitional states before confirmation
- **Confirmation Threshold** - Number of consecutive failures needed to confirm
- **Time-based Debouncing** - Uses elapsed time rather than tick count
- **Counter-based Debouncing** - Uses number of samples

### DTC Lifecycle

```
[Not Complete] → [PreFailed/PrePassed] → [Confirmed]
                        ↓                      ↓
                   [Healed] ← ← ← ← [Pending] → [Aged] → [Cleared]
```

- **PDTC (Pending DTC)** - Temporary fault indicator
- **CDTC (Confirmed DTC)** - Fully confirmed fault requiring service

### Status Byte Flags

The UDS status byte tracks DTC state across 7 bits:

| Bit | Name                                      | Description                                                               |
|-----|-------------------------------------------|---------------------------------------------------------------------------|
| 0   | Test Failed (tf)                          | Confirmed failure (`true` = event confirmed as failed)                    |
| 1   | Test Failed This Op. Cycle (tftoc)       | Latched failure; set with tf, never cleared                              |
| 2   | Pending DTC (pdtc)                       | Indicates a pending diagnostic trouble code                                |
| 3   | Confirmed DTC (cdtc)                     | Indicates a confirmed diagnostic trouble code                              |
| 4   | Test Not Complete Since Last Clear (tncslc) | Not-complete status since last clear                                   |
| 5   | Test Failed Since Last Clear (tfslc)     | Failure occurred since last clear                                          |
| 6   | Test Not Complete This Op. Cycle (tnctoc) | Not-complete flag (`true` = event not yet confirmed)                    |
| 7   | Reserved                                  | Unused bit                                                               |

## Crates

This workspace contains two crates:

### dem

Core event management with debouncing support.

Key components:
- [`Event`] - Main event struct with configurable debouncing behavior
- [`UdsStatusByte`] - Status bitfield for DTC tracking
- [`Status`] - Event conditions (PreFailed, PrePassed, Failed, Passed)
- [`EventError`] - Error types for invalid operations
- [`CalibConfig`] - Calibration parameters (lifetime-bound references)
- [`NvmConfig`] - Non-volatile configuration storage
- [`FreezeFrame`] - Persistent data associated with an Event (survives power cycles)
- [`FreezeFrameList`] - Fixed-capacity (24) list of FreezeFrame entries, priority-ordered
- [`EventManager`] - Manages a collection of Events and their FreezeFrame data
- [`EventManagerError`] - Error types for EventManager operations

### confirmator

Condition confirmation by accumulating consecutive true steps.

Key components:
- [`Confirmator<T>`] - Generic confirmator supporting `u8`, `u16`, `u32`, `f32`
- [`ConfirmatorError`] - Error types for invalid operations
- [`ConfirmatorValue`] - Trait for types supporting confirmation

## Installation

Add the crates you need to your `Cargo.toml`:

```toml
[dependencies]
dem = "0.1"
confirmator = "0.1"
```

## Usage Examples

### DEM TUI

An interactive terminal UI for observing and interacting with the DEM library.

**Run the TUI:**

```bash
cargo run --example dem_tui -p dem
```

**Features:**
- View 25 events with real-time UDS status flags (TF, TFTOC, PDTC, CDTC)
- See debounce counter values and NVM statistics (occurrence, aging, confirmation cycles)
- Edit CalibConfig parameters at runtime using Up/Down arrows
- View stored freeze frames with physical snapshot data
- Snapshot detail panel shows raw hex data
- Manual operating cycle simulation (stop/init cycles)
- Trigger events via keyboard shortcuts
- Press `?` to show the keyboard shortcuts legend

**Keyboard Controls:**

| Key | Action |
|-----|--------|
| `Space` | Select event for triggering (highlighted in teal) |
| `1` | Trigger PreFailed on selected event |
| `2` | Trigger Failed on selected event |
| `3` | Trigger PrePassed on selected event |
| `4` | Trigger Passed on selected event |
| `↑/↓` | Navigate between events or freeze frames |
| `PgUp/PgDn` | Scroll the details panel |
| `I` | Initialize operating cycle |
| `S` | Stop operating cycle |
| `N` | Advance to next cycle (stop + init) |
| `C` | Clear all events and freeze frames |
| `F` | Toggle freeze frames panel |
| `R` | Toggle raw/physical snapshot view (in freeze frames panel) |
| `E` | Edit CalibConfig for selected event |
| `?` / `H` | Show/hide help overlay |
| `Q` | Quit |

**Event Selection:**
- Use `↑/↓` to navigate and view event details
- Press `Space` to select an event for triggering (shown in teal)
- Selected event persists until a new selection

**Editing CalibConfig:**
1. Press `E` to enter edit mode
2. Use `↑/↓` to change values (applies immediately)
3. Use `Tab` to cycle through fields (step_up, step_down, debounce_type, debounce_behavior, confirmation_threshold, aging_threshold, priority, save_trigger, record_update)
4. Press `Esc` to cancel and exit edit mode

## Development

Run all tests across the workspace:

```bash
cargo test --workspace
```

Run tests for a specific crate:

```bash
cargo test -p dem
cargo test -p confirmator
```

Run documentation tests:

```bash
cargo test --doc --workspace
```

## License

MIT License - see [LICENSE](LICENSE) file
