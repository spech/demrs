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
- **Status Tracking** - Monitoring DTC lifecycle (pending, confirmed, healing)

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

### Freeze Frames

Freeze frames capture a snapshot of system state when a diagnostic event occurs. They provide context for debugging by storing:
- Sensor values (voltage, RPM, speed, temperatures)
- System conditions at the time of fault detection

Freeze frames are stored persistently and survive power cycles. The DEM supports up to 24 freeze frames, prioritized by event priority.

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
| 7   | Warning Indicator Requested (wir)         | Warning indicator (MIL) activation flag                                  |

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
- Interactive TUI with live system state display
- 25 configurable diagnostic events with UDS status tracking
- Freeze frame storage with physical and raw hex views
- Press `?` in the TUI to see all keyboard controls

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
