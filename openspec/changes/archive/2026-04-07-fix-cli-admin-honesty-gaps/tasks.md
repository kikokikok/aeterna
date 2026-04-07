## 1. Remove fabricated live output

- [x] 1.1 Replace fabricated live output in `admin drift`, `admin validate`, `admin migrate`, and `admin import` with real backend-backed behavior or explicit unsupported errors.
- [x] 1.2 Remove example-result rows from live human-readable org/user/team/govern flows that are not backed by real data.

## 2. Complete shared client and route wiring

- [x] 2.1 Add shared authenticated client methods needed by the honest admin/operator surface, including helpers for routes that may be completed in follow-up changes.
- [x] 2.2 Add or complete only the corresponding server routes that are actually supported by this change, and remove speculative route scaffolding for the rest.
- [x] 2.3 Wire CLI command handlers to real client/server flows where backend paths exist and preserve explicit unsupported behavior everywhere else.

## 3. Honest local-only flows and verification

- [x] 3.1 Make locally executable context-selection flows perform real local state updates rather than preview-only output.
- [x] 3.2 Add Rust CLI E2E coverage for supported admin/operator flows and unsupported-path error semantics.
- [x] 3.3 Confirm Newman/Postman coverage for the HTTP admin/operator workflows actually completed by this change, and document deferment of broader scenarios until real org/team/user/govern routes exist.
