# Printer Discovery Flash Restoration

## Context

The printers page previously treated a discovery result for an already-configured printer as immediate positive reachability evidence. It marked the matching configured printer connected and flashed its existing inventory row. The printer-monitor refactor removed that bridge: discovery results still carry `configured_names`, but now only update discovery state.

The live inventory stream still correctly flashes printers added by a later snapshot and printers whose monitored availability changes. The missing behavior is the distinct occurrence of discovery re-finding a configured printer, including one already shown as connected. Availability snapshots alone cannot represent that occurrence because connected-to-connected has no state difference.

## Decision

Restore the behavior entirely in the frontend. Discovery already delivers the occurrence and the configured names to the browser running the scan, so adding a backend monitor event would duplicate an event that is already available and expand the API solely for presentation feedback.

`PrinterInventoryProvider` will expose an operation that accepts the configured names reported by discovery. For every matching printer in its current snapshot, the operation will:

- set availability to `connected` immediately;
- add the existing `found` flash state, even if availability was already `connected`; and
- schedule removal through the existing flash timeout mechanism.

The operation will leave names absent from the current snapshot unchanged. It will not create inventory entries or modify their profiles or connection facts.

`AppDataProvider`, which owns the discovery stream across route changes, will call this operation when a USB or network discovery event has non-empty `configured_names`. The discovery result will continue to be recorded in scan state exactly as it is now.

## Data Flow

1. The discovery SSE reports a printer with one or more `configured_names`.
2. `AppDataProvider` records the discovery result and reports those names to `PrinterInventoryProvider`.
3. `PrinterInventoryProvider` updates its cached snapshot and starts or restarts each matching printer's `found` flash.
4. `PrinterList` receives the existing `printer-row-found` class through `printerFlashes`.
5. Later printer-monitor snapshots remain authoritative and replace the optimistic availability normally.

No backend route, monitor type, or SSE payload changes.

## Lifecycle and Failure Behavior

The flash uses the existing 1.2-second timeout and cleanup owned by `PrinterInventoryProvider`. Re-finding the same printer restarts the flash window rather than stacking independent timers.

If discovery reports a configured name before the inventory has produced its first snapshot, or after that name has disappeared from the latest snapshot, the operation does nothing for that name. The next monitor snapshot remains the only source allowed to add or remove configured printers.

A later monitor snapshot may mark the printer unavailable again. That is expected: discovery supplies immediate positive evidence, while the monitor continues to own subsequent availability.

## Testing

Frontend tests will cover the user-visible contract through the assembled printers page:

- a discovery event for a configured unavailable printer immediately renders it connected and flashes both responsive inventory representations;
- a discovery event for a configured printer that is already connected still flashes both representations; and
- the configured discovery result remains counted rather than offered for addition.

The focused tests, full frontend test suite, type checking, and production build will run through Docker Compose as required by the repository.
