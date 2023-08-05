# rp-scale Migration Instructions

This document is the working rulebook for replacing `gscale-platform` with a Rust implementation.

## Core Goal

`rp-scale` must replace `gscale-platform` without users noticing the change.

The replacement is correct only when observable behavior is the same:

- Same input produces the same output.
- Same hardware protocol produces the same readings, commands, statuses, and errors.
- Same mobile API contract produces the same app behavior.
- Same bridge state JSON shape produces the same monitor UI behavior.
- Same parser inputs produce mathematically identical parsed values.
- Same timing-sensitive scale scenarios produce equivalent stable/live decisions.

If the result differs from production behavior, the migration is not complete.

## Source Of Truth

`gscale-platform` is the production reference.

Use it as a behavior contract, not as code to copy blindly.

- Read production Go code before implementing the Rust equivalent.
- Preserve device protocol and output contracts.
- Do not trust README claims when code shows different behavior.
- If Go logic is confusing, risky, or likely wrong, stop and discuss before changing the behavior.
- Improve structure and naming in Rust, but do not change observable behavior without approval.

## Project Boundary

`rp-scale` starts with scale logic first.

Initial scope:

- Scale device detection.
- Serial port open/configuration.
- Serial stream reading.
- Frame extraction.
- Weight parsing.
- Reading model.
- Stable/live reading behavior.
- Error/status model.
- Bridge state shape only where scale state requires it.

Out of initial scope:

- Accord mobile app changes.
- `accord_mobile_server_rs` changes.
- ERP integration.
- Item search.
- Warehouse search.
- Printer migration.
- Batch business logic migration.
- Admin/settings logic.

Production repositories are read-only references unless explicitly approved.

## Scale Driver Direction

Scale must follow the same boundary principle as printers.

Core should not know whether the scale is serial, USB, Wi-Fi, Bluetooth, or a future protocol.

Core rule:

- A scale driver owns hardware communication.
- Core receives normalized realtime weight readings.
- Core decides what to do when weight becomes stable.
- Core must not parse hardware bytes directly outside a driver/protocol module.
- Core must not contain brand/protocol-specific scale checks when a driver can expose capabilities.

Current first driver:

- Serial scale driver based on `gscale-platform` production behavior.

Future drivers:

- Wi-Fi scale driver.
- Bluetooth scale driver.
- Vendor SDK based scale driver.
- Simulated/test scale driver.

The shared output from every scale driver should be a typed reading:

- Weight value.
- Unit normalized to the core unit, preferably `kg`.
- Stable/unstable state when the device can provide it.
- Raw source frame only for diagnostics and compatibility.
- Source metadata such as driver id, port/address, and baud/protocol.
- Timestamp.
- Error/status when hardware cannot produce a valid reading.

Core should consume this typed stream and handle:

- Stable/live decision.
- Stable/live state for later trigger policy.
- Batch state transition.
- API snapshot generation.
- Mobile/monitor state updates.

Manufacturing policy rule:

- Do not require zero-crossing as a global core rule.
- Do not require weight to drop below the previous quantity as a global core rule.
- These are optional workflow policies only.
- Core must expose state; workflow policy decides how to use it.

Hardware/protocol modules should handle:

- Serial frame splitting.
- Device byte parsing.
- Wi-Fi request/response or streaming protocol.
- Vendor-specific status frames.
- Device connection and reconnect.
- Device capability export.

This keeps `rp-scale` plug-and-play: a future scale driver can be added without changing core business flow.

Detailed scale behavior contract:

- `docs/scale_contract.md`

## Future Printer Direction

Printer migration is a later phase, but the architecture must be prepared now.

The printer layer must be capability-driven, not brand-hardcoded.

Core rule:

- Core decides what needs to be printed or encoded.
- A printer driver decides how that printer performs the job.
- Mobile app discovers printer capabilities from `rp-scale`.
- Unsupported printer modes are rejected explicitly unless a fallback is approved.

This means GoDEX, Zebra, and later printer brands should fit the same driver contract.

Capability examples:

- Thermal label print supported or unsupported.
- RFID EPC write supported or unsupported.
- QR/barcode support.
- Verify-after-print support.
- Required job fields.
- Unsupported print modes.

Runtime rule:

- Parse small printer manifests/config at startup only.
- Convert manifest data into typed Rust structs.
- Do not keep parsing JSON during print hot paths.
- Do not add brand-specific checks into core business flow when a driver capability check can solve it.

Detailed printer contract:

- `docs/printer_driver_contract.md`

## Architecture Direction

Final architecture:

- `rp-scale` is the local hardware edge agent.
- `accord_mobile_server_rs` owns auth, ERP credentials, catalog, warehouse search, ERP write, config, audit, and idempotency.
- `accord_mobile` talks to Accord server for account/business flows and to local `rp-scale` for hardware control.

`rp-scale` should own:

- Scale reader.
- Stable weight detection.
- Printer/RFID/Godex/Zebra hardware logic in later phases.
- EPC/print lifecycle in later phases.
- Local batch state in later phases.
- Local monitor API.
- LAN discovery.
- Accord server client in later phases.

`rp-scale` should not own:

- ERP API key/secret.
- ERP setup UI/API.
- ERPNext REST client.
- ERP read discovery.
- Direct item/warehouse search.
- Direct Stock Entry create/submit/delete logic.
- User login/session/role management.
- Admin/profile/business settings.

## Code Organization Rules

- No source file may exceed 500 lines.
- Split modules before files grow large.
- Use clear folder and file names.
- Keep modules focused on one responsibility.
- Avoid copying Go structure when a cleaner Rust structure preserves the same behavior.
- Avoid broad abstractions until there is real repeated behavior.
- Keep public API small and explicit.

## JSON And High-Load State Rule

Avoid JSON in hot paths and high-load internal state.

Past Go rewrites had production crashes when JSON-backed state could not keep up with load. `rp-scale` must not repeat that pattern.

Allowed JSON usage:

- Public HTTP API responses where the mobile app contract requires JSON.
- Compatibility bridge snapshots only when matching the existing GScale contract requires it.
- Small config files loaded at startup.
- Debug/export fixtures.
- Test golden files.

Avoid JSON usage for:

- High-frequency scale readings.
- Print request queues.
- Batch runtime state.
- Internal event streams.
- Retry queues.
- Any state that is updated many times per second.

Preferred internal formats:

- In-memory typed structs for live state.
- Bounded channels for realtime events.
- Compact binary or append-only records for durable high-frequency data.
- Explicit snapshots generated from typed state only at API/bridge boundaries.

Before adding JSON-backed persistence or a JSON hot path:

1. Identify the update frequency and worst-case load.
2. Explain why typed memory or a compact format is not enough.
3. Get approval.

Expected initial shape:

```text
rp-scale/
  Cargo.toml
  docs/
    migration_instructions.md
  src/
    main.rs
    config.rs
    scale/
      mod.rs
      detect.rs
      errors.rs
      parser.rs
      reading.rs
      serial.rs
      stable.rs
    state/
      mod.rs
      bridge_snapshot.rs
  tests/
    contract/
      fixtures/
      test_contract.py
```

## Test Strategy

Primary test language: Rust.

Use Rust tests for:

- Parser behavior.
- Frame extraction.
- Reading model.
- Stable/live decisions.
- Serialization shape.
- Small module-level invariants.

Contract/differential test language: Python standard library.

Use Python `unittest`, `subprocess`, and `json` for:

- Comparing Go reference output with Rust output.
- Checking exact JSON/text fixtures.
- Running black-box command tests.
- Checking source file line counts.

Do not require external Python packages for core verification.

## Golden Behavior Rules

Golden behavior must come from production code or captured production-compatible fixtures.

For every migrated behavior, create tests around:

- Normal valid input.
- Noisy serial input.
- Partial frames.
- Multiple frames in one buffer.
- Invalid frames.
- Negative and positive weights.
- Unit normalization.
- Stability transitions.
- Timeout/error behavior where deterministic.
- JSON field names and value formats.

The test target is not "reasonable output"; it is "same output as production contract".

## Hardware Protocol Rule

Hardware-facing behavior must remain protocol-compatible.

When Go sends or expects a specific byte/string/frame pattern, Rust must preserve the protocol unless an explicit decision is made to change it.

Before changing hardware protocol behavior:

1. Identify the Go reference code.
2. Explain the observed behavior.
3. Explain the proposed change.
4. Wait for approval.

## Decision Rule

When a Go behavior appears wrong but production may depend on it:

- Do not silently fix it.
- Document the issue.
- Keep the compatible behavior by default.
- Ask before changing output, timing, protocol, or API shape.

## Completion Criteria For A Ported Module

A module is considered ported only when:

- Rust implementation exists.
- Rust tests cover normal and edge cases.
- Contract fixtures compare with Go behavior where useful.
- File length check passes.
- Public behavior is documented.
- Known uncertainties are listed.
