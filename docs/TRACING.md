# Command tracing

## Purpose

Command tracing explains how an immutable ESC/POS byte stream changes printer
state and produces rendered output. A PNG answers what the simulated printer
produced; a trace connects that output to the exact input commands that caused
it.

Tracing is optional. Ordinary `render` calls continue to use the plain
monochrome surface and do not allocate trace records, copy command bytes, or
calculate trace-only state. The initial tracer is an internal test proof, not a
public API or stable serialization format.

## Rendering surfaces

The renderer is generic over the private `RenderSurface` contract:

```text
PrinterState<S: RenderSurface>
                │
                ├── MonoSurface
                │     canonical raster and PNG output
                │
                └── TracingSurface
                      wraps MonoSurface in the current test proof
```

The implementation is split by responsibility:

```text
crates/escpost/src/surface/
├── mod.rs       RenderSurface contract and module exports
├── mono.rs      monochrome raster storage and PNG encoding
└── tracing.rs   test-only provenance-decorating surface proof
```

`TracingSurface` forwards every drawing operation to its inner surface. This
keeps traced and untraced rendering on the same command interpreter and raster
implementation. Static generic dispatch lets an optimized ordinary render
compile away the default no-op command hook.

`RenderSurface::fork` creates an empty related surface while preserving
decorator context. The renderer uses it for temporary line buffers, resized
print areas, HRI text, and sheets created after cuts. Trace metadata therefore
follows the same composition and positioning operations as pixels.

## Command decoding seam

The current proof also has a private, compile-time `CommandSink` seam. A traced
render specializes the interpreter with a recording sink; an ordinary render
specializes it with `NoTrace`:

```text
render_surfaces_with_sink<S, C: CommandSink>
                              │
                              ├── C::ENABLED = false → NoTrace
                              └── C::ENABLED = true  → recording sink
```

The interpreter always decodes the parameters needed to execute a command.
It constructs the additional semantic `DecodedCommand` value and byte-range
record only inside an `if C::ENABLED` branch. Thus the ordinary path does not
construct decoded-command objects, call the surface command hook, allocate
trace storage, copy payloads, format values, use dynamic dispatch, or test a
runtime tracing flag. Static generic dispatch lets an optimized build remove
the disabled branch entirely.

This is a source-level performance contract, backed by the command-heavy
release benchmark in `examples/render_bench.rs`. Exact machine-code identity
is a compiler outcome rather than a Rust language guarantee, so benchmark
comparisons remain part of changes to this seam.

The vertical slice currently models only justification, printable bytes, and
line feed. It exists to validate the abstraction and its disabled-path cost
before the complete command model is designed.

## Target production model

The following sections specify intended production behavior. The current
implementation does not yet produce complete command entries, typed effects,
byte ranges, or stable painted rectangles; its narrower guarantees are listed
under [Current proof](#current-proof).

### Command identity

Every safely framed command will be identified by its byte range in the
submitted input. The range, not a copied payload, is the authoritative link
back to the immutable source. A future serialized trace may include raw bytes
for convenience, but they must match that range exactly.

The current vertical slice records the complete range after parsing determines
the command length and also uses that range for command relationships.

Printable bytes may initially appear as individual commands. Grouping adjacent
text bytes into display runs is a presentation decision and must not lose the
underlying byte boundaries.

### Command effects

Every parsed command will receive a trace entry, including commands that paint
nothing. A command can have more than one effect:

- **Paint** — one or more regions on ordered output sheets.
- **State change** — typed before/after values such as justification, font,
  print area, or line spacing.
- **Motion** — logical print-position movement, including the positions before
  and after the command.
- **Flush** — buffered commands committed to a sheet by another command.
- **Device event** — a drawer pulse or another non-printing physical action.
- **Sheet boundary** — a completed sheet and the cut that caused it.
- **Ignored** — a valid command that had no effect, with a typed reason.
- **Diagnostic** — malformed, unsupported, unavailable, clipped, or otherwise
  noteworthy behavior.

This model avoids inventing a painted rectangle for state-only commands. The
CLI and web interface can visualize each effect appropriately: a region, state
diff, movement marker, event, boundary, or diagnostic.

### Painted regions

The production tracer will store rectangles, not a record for every contributed
pixel. A printable command can own several rectangles, and every rectangle
identifies its sheet and printer-dot coordinates.

The intended initial semantics are:

- **requested bounds** describe the area the command attempted to affect;
- **contributing bounds** describe the clipped area in which the command
  changed raster coverage; and
- the primary web highlight uses contributing bounds, with requested bounds
  optionally shown to explain clipping.

Final visible pixel ownership is not stored initially. Later commands may
overlap, reverse, or erase earlier output, making a single final owner
ambiguous. If the UI eventually needs exact final-visible selection, it should
derive that view from command effects under separately documented overlap
rules.

The test-only vertical slice records only primitive operations that change
raster coverage, translates them through composition, and coalesces them into
contributing rectangles per command and sheet. It does not yet expose requested
bounds or define final-visible ownership when commands overlap.

### Buffered output and motion

Standard-mode text and column graphics are first painted into a line-local
surface. Their final sheet position is not known until a feed operation applies
the print area and justification.

In the production model, when `LF` flushes a line:

1. regions already belong to the commands that produced the buffered content;
2. composition translates those regions into final sheet coordinates;
3. `LF` records a flush relationship to those commands; and
4. `LF` records its own print-position movement.

`LF` does not take ownership of the flushed pixels. In the web interface,
hovering the printable command highlights its final rectangle. Hovering `LF`
can show before/after position markers, a paper-advance indicator, and a
secondary highlight of the commands it flushed.

The same rule applies to other positioning and feed commands: they record
motion rather than fabricated paint.

### State, resources, and events

State-setting commands will record only values they actually change. An ignored
setting records an `Ignored` effect and its reason rather than a false state
transition.

Commands that store QR or graphics data change an internal resource but paint
nothing. The later print command owns the resulting painted region and may
reference the stored-data command as an input dependency.

Cuts can combine motion and a sheet boundary. Drawer pulses and similar
non-printing actions are device events. Initialization records restored state
and any buffered data it discards.

### Errors and safe framing

Tracing must remain useful when strict rendering fails. It will record every
command whose boundary and effects were established safely, then report the
failure at the exact byte offset. Remaining bytes may be exposed as opaque input
but must not be presented as speculatively decoded commands.

Diagnostics must distinguish malformed or truncated input, an unimplemented
valid command, a profile-unavailable command, an ignored command, clipped
output, and a profile-confirmed behavioral deviation.

## Current vertical slice

The test-only tracer renders centered text followed by `LF` and assembles a
crate-private `Trace` containing ordered `CommandTrace` entries. Each entry has
its exact input byte range, a semantic `DecodedCommand`, and typed effects. The
slice implements justification state changes, printer-position motion, flush
relationships, and contributing `PaintRegion` rectangles with sheet indices.

The end-to-end test verifies that:

- traced and ordinary raster surfaces are identical;
- `ESC a` records the `Left` to `Center` state transition without paint;
- the text's coalesced rectangles retain the printable byte's input range and
  are translated into the centered position on sheet zero; and
- `LF` records its before/after position and its relationship to the flushed
  text command without taking ownership of that command's paint.

The types are deliberately crate-private while their shape is validated. The
slice does not yet trace other commands, return a partial trace on failure, or
make its in-memory representation a stable public contract.

## Open design decisions

Before exposing tracing publicly, decide and document:

- the Rust trace types and versioned JSON representation;
- typed state deltas and command parameter models;
- rectangle clipping and coalescing rules;
- how command dependencies such as stored data and later printing are linked;
- trace behavior on render errors and resource-limit failures;
- whether traces are retained only in memory or persisted with captures; and
- CLI output and web selection behavior.

These decisions belong to the trace model, not to `MonoSurface` or the ordinary
PNG rendering API.
