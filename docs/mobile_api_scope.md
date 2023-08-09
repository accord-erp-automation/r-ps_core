# Mobile API Scope

This document records the fact-checked split between the production
`gscale-platform` mobile API and the new `rp-scale` driver role.

## Source Facts

Production `gscale-platform/internal/mobileapi/server.go` exposes:

- `GET /healthz`
- `GET /v1/mobile/handshake`
- `POST /v1/mobile/auth/login`
- `POST /v1/mobile/auth/logout`
- `GET|PUT /v1/mobile/profile`
- `GET /v1/mobile/setup/status`
- `POST|DELETE /v1/mobile/setup/erp`
- `POST /v1/mobile/setup/warehouse`
- `GET /v1/mobile/monitor/state`
- `GET /v1/mobile/monitor/stream`
- `GET /v1/mobile/items`
- `GET /v1/mobile/items/{item_code}/warehouses`
- `GET /v1/mobile/warehouses`
- `GET /v1/mobile/batch/state`
- `POST /v1/mobile/batch/start`
- `POST /v1/mobile/batch/manual-print`
- `POST /v1/mobile/batch/stop`
- `GET /v1/mobile/archive`
- `POST /v1/mobile/archive/print`

Current `accord_mobile` GScale screen calls:

- `/healthz`
- `/v1/mobile/monitor/stream`
- `/v1/mobile/monitor/state`
- `/v1/mobile/setup/status`
- `/v1/mobile/items`
- `/v1/mobile/items/{item_code}/warehouses`
- `/v1/mobile/warehouses`
- `/v1/mobile/setup/warehouse`
- `/v1/mobile/batch/start`
- `/v1/mobile/batch/manual-print`
- `/v1/mobile/batch/stop`
- `/v1/mobile/archive`
- `/v1/mobile/archive/print`

Current `rp-scale` implements:

- `GET /healthz`
- `GET /v1/mobile/handshake`
- `GET /v1/mobile/printer/capabilities`

## Driver Boundary

`rp-scale` is not the ERP owner.

It must not own:

- ERP API key or secret.
- ERP setup workflow.
- ERP item search.
- ERP warehouse search.
- ERP stock entry create, submit, or delete.
- User login and account session.
- Long-term business archive.

It should own:

- LAN discovery.
- Mobile handshake.
- Hardware health.
- Scale state.
- Printer state.
- Printer capabilities.
- Local hardware job execution.
- Short-lived local job state needed for monitor UI.

## Endpoint Classification

Must stay compatible in `rp-scale`:

- `GET /healthz`
- `GET /v1/mobile/handshake`
- `GET /v1/mobile/monitor/state`
- `GET /v1/mobile/monitor/stream`
- `GET /v1/mobile/batch/state`

RPS-specific extension:

- `GET /v1/mobile/printer/capabilities`

Should move to Accord server:

- `POST /v1/mobile/auth/login`
- `POST /v1/mobile/auth/logout`
- `GET|PUT /v1/mobile/profile`
- `GET /v1/mobile/setup/status`
- `POST|DELETE /v1/mobile/setup/erp`
- `POST /v1/mobile/setup/warehouse`
- `GET /v1/mobile/items`
- `GET /v1/mobile/items/{item_code}/warehouses`
- `GET /v1/mobile/warehouses`
- `GET /v1/mobile/archive`

Needs a new split contract:

- `POST /v1/mobile/batch/start`
- `POST /v1/mobile/batch/manual-print`
- `POST /v1/mobile/batch/stop`
- `POST /v1/mobile/archive/print`

The server should own business validation and item or warehouse selection.
`rp-scale` should receive only a prepared hardware job or a short-lived local
batch intent that does not require ERP credentials.

## Compatibility Rule

Mobile app discovery should keep working against `rp-scale`.

That means handshake and discovery fields must keep the GScale shape:

- `service=mobileapi`
- `app=gscale-zebra`
- `server_name`
- `server_ref`
- `display_name`
- `role`
- `http_port`
- `discovery_port`
- `candidate_ports`
- `monitor_path`
- `profile_path`
- `items_path`
- `batch_state_path`
- `requires_auth=false`

The path fields can stay present for old mobile compatibility, but ownership
can change. The mobile app should prefer Accord server for ERP and catalog
paths once the split is implemented.

## Monitor Shape Required By Mobile

`accord_mobile` parses monitor payload as:

- top-level `ok`
- top-level `state`
- `state.scale`
- `state.zebra`
- `state.printer`
- `state.batch`
- `state.print_request`

Minimum safe driver payload:

```json
{
  "ok": true,
  "state": {
    "scale": {
      "source": "serial",
      "port": "/dev/tty.usbserial",
      "weight": 0,
      "unit": "kg",
      "stable": false,
      "error": "",
      "updated_at": ""
    },
    "zebra": {
      "connected": false
    },
    "printer": {
      "connected": false,
      "kind": "",
      "label": "ulanmagan",
      "device_paths": [],
      "error": "",
      "updated_at": ""
    },
    "batch": {
      "active": false
    },
    "print_request": {
      "qty": null,
      "status": "idle"
    },
    "archive_print": {},
    "updated_at": ""
  },
  "printer": {
    "ok": false,
    "connected": false,
    "kind": "",
    "label": "ulanmagan",
    "device_paths": [],
    "error": "",
    "updated_at": ""
  }
}
```

## Next Implementation Order

1. Add typed Rust monitor snapshot structs matching the GScale JSON shape.
2. Add `GET /v1/mobile/monitor/state` with safe driver defaults.
3. Add `GET /v1/mobile/monitor/stream` as SSE snapshots.
4. Add `GET /v1/mobile/batch/state` with local short-lived batch status.
5. Keep ERP/catalog endpoints out of `rp-scale`; expose explicit unavailable or
   delegated responses only if old mobile compatibility requires them.
