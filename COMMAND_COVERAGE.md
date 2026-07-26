# ESC/POS command coverage

This document defines the visible command boundary for `escpos2png`. It keeps
three different claims separate:

- **Implemented** means the command has public-rendering tests.
- **Partial** means useful behavior exists, but documented parameters, state
  interactions, profile gating, or result reporting are still missing.
- **Planned** means the command is framed by the project roadmap but is not
  implemented.

Physical validation is tracked separately because an automated test can prove
our interpretation without proving that the selected printer profile matches
real hardware.

## Version 1 boundary

Version 1 targets safe, deterministic Standard-mode previews for:

- text, common single-byte code pages, and character formatting;
- positioning, tabs, alignment, print areas, line spacing, and paper feeds;
- column, raster, and modern graphics;
- native one-dimensional barcodes and QR codes;
- full and partial cuts plus non-printing drawer pulses; and
- structured diagnostics, device events, limits, and reproducibility metadata.

This boundary covers the commands emitted by Receiptful's current
HTML-to-ESC/POS renderer and adds common native graphics and symbol commands
found in print jobs produced elsewhere.

Page mode, downloaded or non-volatile resources, macros, bidirectional status
emulation, multiple-tone graphics, and spot colors remain post-v1 work. The
architecture keeps room for them; version 1 must report them as unsupported
rather than guess.

## Standard-mode controls and text

| Command | Behavior | Implementation | Automated coverage | Hardware |
|---|---|---:|---:|---:|
| Printable bytes | Decode through the profile-selected table and compose text | Implemented | Implemented | Pending |
| `HT` | Move to the next horizontal tab stop | Implemented | Implemented | Pending |
| `LF` | Print and feed one line | Implemented | Implemented | Pending |
| `CR` | Ignore or print/feed according to the profile's auto-line-feed behavior | Implemented | Implemented | Pending |
| `ESC @` | Initialize printer state and clear pending print data | Implemented | Implemented | Pending |
| `ESC SP` | Set right-side character spacing | Implemented | Implemented | Pending |
| `ESC !` | Select Font A/B and common print modes | Implemented | Implemented | Pending |
| `ESC -` | Select underline thickness | Implemented | Implemented | Pending |
| `ESC 2` | Restore default line spacing | Implemented | Implemented | Pending |
| `ESC 3` | Set line spacing | Implemented | Implemented | Pending |
| `ESC D` | Set horizontal tab positions | Implemented | Implemented | Pending |
| `ESC E` | Select emphasized printing | Implemented | Implemented | Pending |
| `ESC J` | Print and feed by vertical motion units | Implemented | Implemented | Pending |
| `ESC M` | Select Font A/B | Implemented | Implemented | Pending |
| `ESC R` | Select international character substitutions | Implemented | Implemented | Pending |
| `ESC a` | Select line justification | Implemented | Implemented | Pending |
| `ESC d` | Print and feed whole lines | Implemented | Implemented | Pending |
| `ESC t` | Select a profile-defined character-code table | Implemented | Implemented | Pending |
| `GS !` | Select character width and height multipliers | Implemented | Implemented | Pending |
| `GS B` | Select white/black reverse text | Implemented | Implemented | Pending |

`ESC a` is covered for text and every currently implemented version 1
graphics and symbol family. See
[`tests/INTERACTIONS.md`](tests/INTERACTIONS.md).

`ESC R` implements Epson's common international sets 0–17 and resets to the
profile default on `ESC @`. The additional Indic sets 66–75 and 82 remain
post-v1 because the representative glyph asset does not yet cover those
scripts. The NT-5890K currently assumes that auto line feed is disabled, so
its ignored-`CR` behavior remains an explicit profile approximation until a
physical probe confirms it.

## Positioning and print area

| Command | Behavior | Implementation | Automated coverage | Hardware |
|---|---|---:|---:|---:|
| `ESC $` | Set absolute horizontal position | Implemented | Implemented | Pending |
| `ESC \` | Set relative horizontal position | Implemented | Implemented | Pending |
| `GS L` | Set left margin | Implemented | Implemented | Pending |
| `GS P` | Set horizontal and vertical motion units | Implemented | Implemented | Pending |
| `GS W` | Set print-area width | Implemented | Implemented | Pending |

`GS L` and `GS W` cover origin, width, justification, clipping, and
oversized-data behavior for every currently implemented version 1 printable
family.

## Graphics

| Command | Behavior | Implementation | Automated coverage | Hardware |
|---|---|---:|---:|---:|
| `ESC *` | Print column-format bit image in all four modes | Implemented | Implemented | Pending |
| `GS v 0` | Print raster-format bit image in all four scaling modes | Implemented | Implemented | Pending |
| `GS ( L` Function 50 | Print buffered graphics data | Implemented | Implemented | Pending |
| `GS ( L` / `GS 8 L` Function 112 | Store raster graphics data | Implemented | Implemented | Pending |

The implemented image commands include profile capability checks and their
documented beginning-of-line, print-area, and justification interactions.
Function 112 currently accepts the version 1 monochrome plane (`a=48`,
`c=49`) and both documented 1×/2× scales. Multiple-tone and additional-color
planes remain post-v1 as defined above.

## Native symbols

| Command family | Behavior | Implementation | Automated coverage | Hardware |
|---|---|---:|---:|---:|
| `GS H`, `GS h`, `GS f`, `GS w`, `GS k` | Configure and print one-dimensional barcodes | Partial | Implemented | Validated on NT-5890K |
| `GS ( k` QR functions | Configure, store, and print QR symbols | Partial | Implemented | Validated on NT-5890K |
| `GS ( k` PDF417 functions | Configure, store, and print PDF417 symbols | Post-v1 | Planned | Pending |
| `GS ( k` Data Matrix functions | Configure, store, and print Data Matrix symbols | Post-v1 | Planned | Pending |

Receiptful currently rasterizes barcodes and QR codes before emitting
ESC/POS. Native symbol support is nevertheless included in v1 so arbitrary
Standard-mode jobs do not depend on that implementation detail.

Common `GS k` systems UPC-A, UPC-E, EAN-13, EAN-8, Code 39, ITF, Codabar,
Code 93, and explicit-set Code 128 are implemented for the profile-advertised
Function A/B forms. The newer GS1-128, GS1 DataBar, and automatic Code 128
systems remain pending. Both barcode command framings return bytes after an
early Code 39 stop to normal ESC/POS processing. Code 93 HRI includes the
specified start/stop and control-character placeholders. The Epson barcode
reset defaults are implemented but still need profile-level hardware
validation.

QR Functions 165, 167, 169, 180, and 181 implement Model 2 selection, module
size, error correction, raw-byte storage, and printing. Model 1, Micro QR, and
Function 182's bidirectional size response remain pending. QR matrices are
valid and deterministic, but their mask choice is not yet claimed to match a
specific Epson firmware. The NT-5890K hardware case confirms native command
availability and the expected 84-dot QR dimensions; it does not yet compare
every logical module with a digitized physical print.

## Paper and device actions

| Command | Behavior | Implementation | Automated coverage | Hardware |
|---|---|---:|---:|---:|
| `GS V` Function A | Full or partial cut at the current position | Implemented | Implemented | Pending |
| `GS V` Function B | Feed to cutter and cut | Partial framing | Planned | Pending |
| `ESC p` | Generate a cash-drawer pulse without printing | Implemented | Implemented | Pending |

Feed-to-cutter commands require profile geometry rather than an assumed
universal distance.

## Post-v1 protocol families

The following remain explicit long-term work:

- Page mode selection, area, direction, composition, printing, and canceling;
- user-defined characters and international/multibyte character systems;
- downloaded, non-volatile, and keyed graphics resources;
- macros and stored command sequences;
- upside-down, rotated, smoothed, and model-specific print modes;
- automatic status, real-time status, and bidirectional responses;
- panel buttons, buzzers, displays, slip stations, and other mechanisms; and
- multiple-tone or indexed spot-color rendering.

Adding a command requires its Epson reference, framing tests, public behavior
tests, relevant interaction entries, profile capability behavior, and an
honest hardware-validation status.
