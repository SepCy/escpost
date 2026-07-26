# NT-5890K v1 hardware conformance receipt

This is the single consolidated physical comparison for the v1 commands that
the NT-5890K profile advertises. It combines already-isolated conformance
streams rather than introducing different test data for hardware:

1. Font A/B metrics, double size, emphasis, underline, and reverse text.
2. Absolute/relative positioning, character spacing, margins, print area, and
   centering, exposed with solid reversed-space cells.
3. All four `ESC *` column-image density modes.
4. All four `GS v 0` raster scaling modes.
5. EAN-13 through both `GS k` framings and a Model 2 QR symbol.
6. Full/partial `GS V` Function B forms on a printer without an autocutter,
   exposed with horizontally staggered raster markers.

Each section starts from `ESC @`, but initialization does not retract or erase
paper already rendered. No cut or drawer-pulse command is included. Final
feeds leave enough paper for manual tearing.

The labels aid paper/PNG comparison but their glyph outlines are not expected
to match because the renderer deliberately uses a representative bundled
font. Compare:

- line origins, wrapping, cell sizes, and vertical advancement;
- visible reversed-cell coordinates and centered print-area cell;
- the four column-graphics and four raster-graphics scales;
- barcode width, height, HRI placement, and centering;
- QR dimensions and placement; and
- the first Function B marker gap and the vertically adjacent final markers.

The exact PNG is served at <http://localhost:8765/tools/preview/> with integer
nearest-neighbor zoom.

The initial physical comparison showed that this firmware paints `ESC *`
8-dot source rows adjacently and adds a faint trailing line. The typed profile
models the material one-dot vertical pitch; the trailing line remains an
explicit approximation. Positioning and no-cut sections now use other marker
families so that artifact cannot obscure their measurements.

The initial comparison also showed that the printer restores barcode defaults
after Function A. The revised stream therefore sends `GS H`, `GS h`, and
`GS w` again before Function B. This isolates command framing from that
firmware state-reset quirk.

A second isolated feed probe established that this firmware ignores `ESC J`
and `GS V 66 n`, while `GS V 65 n` performs the requested feed. The profile
models all three behaviors. The final two no-cut markers therefore occupy
adjacent vertical rows. Their different horizontal insets keep the command
boundaries visible instead of forming one double-height block.

## Physical run

```text
date: 2026-07-26
input SHA-256: 72366422ef0185da2cf13b4088649773f4569e9f7cbc602ca868f31c0dc59537
renderer commit: pending
printer profile: NT-5890K
canonical profile SHA-256: 3a7459577213318b7b55b3758dc3cd94669c70f46decb9752b99460e03824334
printer USB identity: 0416:5011
serial: B120300001
connection: USB interface 0, OUT endpoint 0x01
transport result: all 1208 hash-verified bytes sent without a USB error
visual comparison: pending
```

A successful USB write proves only that the exact fixture reached the device;
it does not prove what appeared on paper. Record the geometry and symbol
observations here after comparing the receipt with the served PNG.
