# Agent Guidelines

## Embedded Context

This is an embedded automotive project with persistent (non-volatile) data structures:

- **`NvmConfig`**: Stores event state that persists across power cycles, including:
  - `uds_status`: UDS status byte flags (tf, tftoc, pdtc, cdtc, etc.)
  - `confirmation_cycles`: Cycle counter for DTC confirmation
  - `aging_cycles`: Cycle counter for DTC aging
  - `occurence_cntr`: Failure occurrence counter

- **`ExtendedRecordList`**: Persistent storage for extended records (24 slots)

When writing tests for functions that modify persistent state, be aware that:
- Tests may need to set up initial `NvmConfig` values before calling `init()`
- Some state changes only occur when specific flag combinations are present (e.g., `tftoc`, `tnctoc`, `cdtc`)
- Functions like `stop()` have different branches based on the combination of these flags

## Tests

### Naming Convention (Unit Tests Only)

Unit test names must follow the `fn_<method>_<description>` pattern:

```rust
#[test]
fn fn_disable_sets_disabled_field() {
    // ...
}
```

**Rules:**
- Prefix all test function names with `fn_`
- Use `snake_case` for the entire name
- Include the method/function being tested as the first word after `fn_`
- Follow with a descriptive action/assertion phrase

**Examples:**
| Good | Bad |
|------|-----|
| `fn_disable_sets_disabled_field` | `event_disabled_field_setting` |
| `fn_clear_resets_all_state` | `event_clear_resets_state` |
| `fn_step_timebased_negative_sampling_returns_error` | `timebased_negative_sampling_returns_error` |

**Scope:** This convention applies to **unit tests only**:
- Unit tests: `#[test]` in source files (`src/*.rs`)
- Integration tests: `#[test]` in test files (`tests/*.rs`) - excluded
