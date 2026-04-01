# Agent Guidelines

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
