# Agent Guidelines

## Embedded Context

This is an embedded automotive project with persistent (non-volatile) data structures stored in **NVM** (non-volatile memory):

- **`NvmConfig`**: Stores event state that persists across power cycles, including:
  - `uds_status`: UDS status byte flags (tf, tftoc, pdtc, cdtc, etc.)
  - `confirmation_cycles`: Cycle counter for DTC confirmation
  - `aging_cycles`: Cycle counter for DTC aging
  - `occurence_cntr`: Failure occurrence counter

- **`FreezeFrameList`**: Persistent storage for freeze frames (24 slots)

Both `NvmConfig` and `FreezeFrameList` persist across power cycles. They are stored in NVM and are **not cleared** by `clear()`. Only the working registers (debounce counter, etc.) are reset.

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
- Integration tests: `#[test]` in test files (`tests/*.rs`) - use `bdd_` prefix instead

### Naming Convention (Integration Tests)

Integration test names must follow the `bdd_<subject>_<scenario>` pattern:

```rust
#[test]
fn bdd_freeze_frame_list_full_eviction() {
    // ...
}
```

**Rules:**
- Prefix all test function names with `bdd_`
- Use `snake_case` for the entire name
- Include the subject being tested
- Follow with a descriptive scenario name

**Examples:**
| Good | Bad |
|------|-----|
| `bdd_freeze_frame_list_full_eviction` | `test_freeze_frame_list_full` |
| `bdd_freeze_frame_list_onpdtc_trigger` | `save_trigger_onpdtc_creates_freeze_frame_on_pdtc_rising` |
