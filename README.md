# DEM TUI

An interactive terminal UI for exploring automotive diagnostic event management, built while learning Rust and experimenting with AI-assisted development.

## Foreword

I wrote this to learn Rust for embedded automotive work. Automotive diagnostics seemed like a good fit because the domain is well-defined (ISO 14229 UDS), the logic is testable, and there's real embedded C code out there for reference.

The AI angle was an experiment too. I wanted to see how well a language model could handle:
- Scaffolding a Rust workspace with proper error handling
- Implementing state machines correctly (these bugs are subtle)
- Writing tests that actually verify behavior

Spoiler: pretty well, but you still need to understand the domain.

## DEM TUI

The TUI is the main entry point. It lets you interact with 25 diagnostic events, observe status changes, and explore freeze frame data.

### Features

- Live system state display with UDS status tracking
- 25 configurable diagnostic events
- Freeze frame storage with hex/physical views
- Indicator lamp control (MIL, RSL, AWL, PL)
- Time-based and counter-based debouncing

### Run It

```bash
cargo run --example dem_tui -p dem
```

Press `?` in the TUI for keyboard controls.

### Screens

| Key | Screen | Description |
|-----|--------|-------------|
| `1` | Events List | All 25 events with status, counter, priority |
| `2` | Event Detail | Selected event config, debounce state, NVM data |
| `3` | Freeze Frames | Stored snapshots, sorted by priority |
| `4` | Lamps | MIL/RSL/AWL/PL state and blink patterns |

### Controls

| Key | Action |
|-----|--------|
| `↑/↓` | Navigate |
| `←/→` | Adjust value (in edit mode) |
| `Space` | Toggle event pass/fail |
| `+ / -` | Change debounce direction |
| `1-9` | Speed multiplier for simulation |
| `c` | Clear all events |
| `r` | Reset selected event |
| `e` | Edit selected event config |

## Installation

Requires Rust (stable). No external dependencies.

```bash
cargo build --example dem_tui -p dem
```

## Development

```bash
cargo test --workspace
cargo test -p dem
cargo test -p confirmator
```

## Reference

Code is organized as a workspace:
- `dem/` - Core event management, UDS status, freeze frames
- `confirmator/` - Debouncing algorithms (time-based, counter-based)

For UDS/DTC concepts, see the code comments and tests—they're more accurate than prose.

## License

MIT License - see [LICENSE](LICENSE) file
