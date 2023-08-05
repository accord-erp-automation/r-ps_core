# Scale Logic Contract

This document records the production behavior copied from `gscale-platform/scale`.

Reference files:

- `/Volumes/Samsung990P/gscale/gscale-platform/scale/parser.go`
- `/Volumes/Samsung990P/gscale/gscale-platform/scale/serial_reader.go`
- `/Volumes/Samsung990P/gscale/gscale-platform/scale/detect.go`
- `/Volumes/Samsung990P/gscale/gscale-platform/scale/types.go`
- `/Volumes/Samsung990P/gscale/gscale-platform/polygon/main.go`

## Architecture Boundary

Scale code must be split into driver/protocol and core layers.

Driver/protocol layer owns:

- Hardware connection.
- Serial, USB, Wi-Fi, Bluetooth, or vendor SDK transport.
- Frame splitting.
- Device-specific byte parsing.
- Device status/error mapping.
- Capability export.

Core layer owns:

- Receiving normalized realtime readings.
- Stable/live decision.
- Exposing stable/live state for print trigger policy.
- Batch state transition.
- API and bridge snapshots.

Core must not depend on a specific hardware transport.

Core must not impose manufacturing-specific reset rules globally.

Forbidden global rules:

- Require weight to return to zero before accepting the next stable reading.
- Require weight to drop below the previous quantity before accepting the next stable reading.

Those rules are manufacturing-specific policies and must be optional configuration only.

The current production-compatible driver is serial scale.

Future scale drivers must emit the same typed reading shape so core behavior does not change.

## Driver Reading Shape

Every scale driver should produce a typed reading containing:

- Numeric weight.
- Unit normalized for core use, preferably `kg`.
- Stable flag when available.
- Raw frame only for diagnostics or compatibility output.
- Driver/source metadata.
- Timestamp.
- Error/status when no valid weight can be produced.

The serial driver may preserve production raw-frame behavior for compatibility.

## Stable State Contract

Stable state tracking is observation, not automatic production trigger.

The tracker may report:

- No valid weight.
- Moving.
- Stable but still inside hold duration.
- Ready stable reading.
- Error state.

The tracker must not decide manufacturing workflow by itself.

Trigger policy must be separate and configurable.

Default core behavior:

- No zero-crossing requirement.
- No previous-quantity drop requirement.
- Device `stable=false` resets stable hold.
- Missing weight resets stable hold.
- Weight changes outside tolerance reset stable hold.
- Weight changes inside tolerance keep the current stable hold.
- A ready stable reading means "safe current observation", not "must print".

## Polygon Simulator Contract

`gscale-platform/polygon` is the production development simulator.

Useful endpoint:

- `GET /api/v1/scale`
- `POST /api/v1/dev/weight`

Observed raw scale frames:

- Stable weight: `1.250 kg ST`
- Unstable weight: `2.750 kg US`
- Stable zero: `0.000 kg ST`

Polygon returns `port="polygon://scale"`.

These frames must parse through the same scale stream decoder contract.

Important fact:

- Current `polygon` code does not create a serial PTY device.
- Production `scale` reads polygon through HTTP bridge fallback.
- Real scale compatibility still requires direct serial port probing.

`rp-scale` verifies direct serial compatibility with a pseudo-terminal test:

- Python creates a PTY pair.
- The simulator side writes `1.250 kg ST` to the master fd.
- Rust opens the slave device path with `SerialPortProbe`.
- The probe must return `parsed_weight=true` and `has_data=true`.

`rp-scale` also provides a Rust scale simulator:

- Binary: `rp-scale-sim-scale`
- It creates a PTY slave device.
- It prints the slave device path as `device=/dev/...`.
- It writes scale frames such as `1.250 kg ST\r` to the PTY.
- `rp-scale-probe-serial` must detect this PTY as a valid scale source.
- The batch scenario ramps between weights, holds stable values for realistic periods, and returns to zero between some movements.

## Parser Contract

Production regex:

```text
(?i)([-+N]?)\s*(\d+(?:[.,]\d+)?)\s*(kg|g|lb|lbs|oz)?\s*([-+]?)
```

Behavior:

- Unicode minus variants are normalized to `-`.
- `N` prefix means negative.
- `-` prefix or suffix means negative.
- `+` prefix or suffix means positive.
- Comma decimal separator is converted to dot.
- Values outside `[-1000000, 1000000]` are rejected.
- Missing unit falls back to the configured default unit.
- Unit is lowercased.
- Candidate score:
  - explicit unit: `+80`
  - explicit sign: `+40`
  - negative sign: `+120`
  - positive sign: `+10`
- Highest score wins.
- If scores are equal, later candidate in the frame wins.
- `US` or `UNSTABLE` marker returns `stable=false`.
- `ST` or `STABLE` marker returns `stable=true`.
- Unstable marker is checked before stable marker.

## Serial Frame Contract

Production frame splitting:

- Frame ends at first `\r` or `\n`.
- Consecutive `\r` and `\n` bytes are consumed together.
- Buffer without delimiter produces no frame.
- Pending serial buffer keeps only the last `1024` bytes.

## Stream Decode Contract

Production stream behavior:

- Empty frames before the first parsed value are ignored.
- Empty frames after a parsed value emit weight `0.0`.
- Parse misses emit a reading with raw frame and no weight.
- Parse misses do not stop the stream.
- Last parsed unit is reused when later frames do not include a unit.
- Initial fallback unit defaults to lowercase configured unit, or `kg` if blank.

## Serial Reader Runtime Contract

Production serial reader behavior:

- Opens the selected serial device with a read timeout.
- Emits a serial reading when the port opens.
- Reads chunks from the port and decodes frames with the serial stream decoder.
- Keeps stream alive when a frame cannot be parsed.
- Emits `open error: ...` when the port cannot be opened.
- Emits `read error: ...` when an opened stream fails.
- Reconnects after open/read failures.

`rp-scale-read-serial` is the diagnostic CLI for this layer.

It must read from the Rust PTY scale simulator without real hardware.

## Detection Contract

Production detection behavior:

- Explicit `--device` wins and uses the first baud in the baud list.
- Candidate order:
  - `/dev/serial/by-id/*`, sorted, symlinks resolved when possible.
  - `/dev/ttyUSB*`, sorted.
  - `/dev/ttyACM*`, sorted.
- Duplicate device paths are removed.
- Probe opens each candidate for each baud.
- If parsed weight is found, candidate is selected.
- If any data is seen but no parsed weight, candidate is still selected.
- Busy errors include:
  - `resource busy`
  - `device or resource busy`
  - `permission denied`
- If no candidate works but a busy error was seen, detection returns busy error.
- Otherwise detection falls back to first candidate and first baud.
