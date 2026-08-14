# ESC/POS command coverage

This document defines the visible command boundary for ESCPost. It keeps
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
- native one-dimensional barcodes and Model 2 QR codes;
- full and partial cuts plus non-printing drawer pulses; and
- device events, bounded resource use, and reproducibility metadata.

This boundary covers the commands emitted by Receiptful's current
HTML-to-ESC/POS renderer and adds common native graphics and symbol commands
found in print jobs produced elsewhere.

The virtual `REFERENCE` profile enables every capability in this implemented
boundary and uses baseline command behavior without physical-printer quirks.
It does not change commands listed as post-v1 into implemented commands.

Page mode, downloaded or non-volatile resources, macros, bidirectional status
emulation, multiple-tone graphics, and spot colors remain post-v1 work.
Version 1 reports them as unsupported rather than guessing their behavior.

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
| `ESC J` | Print and feed by vertical motion units, or consume without feeding when the profile specifies | Implemented | Implemented | Connected NT-5890K ignores it |
| `ESC M` | Select Font A/B | Implemented | Implemented | Pending |
| `ESC R` | Select international character substitutions | Implemented | Implemented | Pending |
| `ESC a` | Select line justification | Implemented | Implemented | Pending |
| `ESC d` | Print and feed whole lines | Implemented | Implemented | Pending |
| `ESC t` | Select a profile-defined character-code table | Implemented | Implemented | Pending |
| `GS !` | Select character width and height multipliers | Implemented | Implemented | Pending |
| `GS B` | Select white/black reverse text | Implemented | Implemented | Pending |

`ESC a` is covered for text and every currently implemented version 1
graphics and symbol family. See
[`crates/escpost-render/tests/INTERACTIONS.md`](../crates/escpost-render/tests/INTERACTIONS.md).

`ESC R` implements Epson's common international sets 0–17 and resets to the
profile default on `ESC @`. The additional Indic sets 66–75 and 82 remain
post-v1 because the representative glyph asset does not yet cover those
scripts. The NT-5890K is modeled with auto line feed disabled, so its
ignored-`CR` behavior is stated as an explicit `defaults.carriage_return`
value.

`ESC t` decodes the profile's supported single-byte tables. Printable ASCII
`20h`–`7Eh` remains available after selecting a known post-v1 multibyte table;
this is required by Receiptful's NT-5890K output, which selects CP932 before
ordinary ASCII. Extended CP932 input still returns a clear error that names
the unsupported code page. Multibyte decoding and its additional glyph assets
remain post-v1.

## Positioning and print area

| Command | Behavior | Implementation | Automated coverage | Hardware |
|---|---|---:|---:|---:|
| `ESC $` | Set absolute horizontal position, subject to profile behavior after printable data | Implemented | Implemented | NT-5890K ignores it after data |
| `ESC \` | Set relative horizontal position, subject to profile behavior for negative values | Implemented | Implemented | NT-5890K applies positive and ignores negative movement |
| `GS L` | Set left margin | Implemented | Implemented | Pending |
| `GS P` | Set horizontal and vertical motion units | Implemented | Implemented | Pending |
| `GS W` | Set print-area width | Implemented | Implemented | Pending |

`GS L` and `GS W` cover origin, width, justification, clipping, and
oversized-data behavior for every currently implemented version 1 printable
family.

## Graphics

| Command | Behavior | Implementation | Automated coverage | Hardware |
|---|---|---:|---:|---:|
| `ESC *` | Print column-format bit image in all four modes | Implemented | Implemented | Validated on NT-5890K with a profile-specific 8-dot vertical pitch |
| `GS v 0` | Print raster-format bit image in all four scaling modes | Implemented | Implemented | Validated on NT-5890K, including its one-shot following-LF suppression |
| `GS ( L` Function 50 | Print buffered graphics data | Implemented | Implemented | Pending |
| `GS ( L` / `GS 8 L` Function 112 | Store raster graphics data | Implemented | Implemented | Pending |

The implemented image commands include profile capability checks and their
documented beginning-of-line, print-area, and justification interactions.
The Epson baseline places 8-dot-mode source rows on a three-printer-dot
vertical pitch at 203 DPI. The connected NT-5890K instead paints those rows
adjacently, so that material geometry difference is a typed profile value.
The same printer also consumes one LF immediately following `GS v 0`; a second
consecutive LF feeds normally. This material vertical-placement difference is
another typed profile behavior, while Epson-compatible profiles keep the
documented LF feed.
Function 112 currently accepts the version 1 monochrome plane (`a=48`,
`c=49`) and both documented 1×/2× scales. Multiple-tone and additional-color
planes remain post-v1 as defined above.

## Native symbols

| Command family | Behavior | Implementation | Automated coverage | Hardware |
|---|---|---:|---:|---:|
| `GS H`, `GS h`, `GS f`, `GS w`, `GS k` | Configure and print one-dimensional barcodes | Implemented | Implemented | Common systems validated on NT-5890K; DataBar unsupported by that printer |
| `GS ( k` Model 2 QR Functions 165, 167, 169, 180, and 181 | Configure, store, and print QR symbols | Implemented | Implemented | Validated on NT-5890K |
| `GS ( k` Model 1 and Micro QR | Configure, store, and print legacy/compact QR variants | Post-v1 | Planned | Pending |
| `GS ( k` QR Function 182 | Return the stored symbol size to a bidirectional host | Post-v1 | Planned | Pending |
| `GS ( k` PDF417 functions | Configure, store, and print PDF417 symbols | Post-v1 | Planned | Pending |
| `GS ( k` Data Matrix functions | Configure, store, and print Data Matrix symbols | Post-v1 | Planned | Pending |

Receiptful currently rasterizes barcodes and QR codes before emitting
ESC/POS. Native symbol support is nevertheless included in v1 so arbitrary
Standard-mode jobs do not depend on that implementation detail.

Common `GS k` systems UPC-A, UPC-E, EAN-13, EAN-8, Code 39, ITF, Codabar,
Code 93, explicit-set Code 128, GS1-128, and automatic Code 128 are implemented
for the profile-advertised Function A/B forms. `GS k m=74` implements Epson's
automatic start/FNC1 behavior, AI delimiters, concatenated fields, brace
escapes, modulus-10 placeholders, code-set planning, and special-character
HRI rules. `GS k m=79` accepts the documented `00h`–`FFh` range and minimizes
symbol width across Code Sets A/B/C, SHIFT, and FNC4 upper-mode shifts and
latches. GS1 DataBar Omnidirectional (`m=75`) and Truncated (`m=76`) implement
the identical 96-module symbol pattern, automatic AI `01` and check digit,
18-character HRI, and their respective `33X` and `13X` minimum heights. Their
logical patterns are checked against independent BWIPP vectors. DataBar
Limited (`m=77`) implements its restricted numeric range, 79-module pattern,
automatic AI and check digit, HRI, and `10X` minimum height. Its combinatorial
groups and ISO Figure 7 pattern are checked against independent Zint vectors.
DataBar Expanded (`m=78`) implements all fourteen standard compaction methods,
Numeric/Alphanumeric/ISO/IEC 646 general-field transitions, explicit FNC1 and
literal-parenthesis escapes, HRI, its 77-byte reduced-data limit, and the `34X`
minimum height. ISO figures and independent BWIPP/Zint vectors cover every
compaction method and the Epson-specific input framing. Both barcode command
framings return bytes after an early Code 39 stop to normal ESC/POS processing.
Code 93 HRI includes the specified start/stop and control-character
placeholders. Every system is gated independently by the printer profile. The
Epson barcode reset defaults, GS1-128, automatic Code 128, and the implemented
DataBar systems remain pending hardware validation on a supporting printer.
The connected NT-5890K probe confirmed that its firmware does not recognize
DataBar `m=75`–`78`.

QR Functions 165, 167, 169, 180, and 181 implement Model 2 selection, module
size, error correction, raw-byte storage, and printing. Model 1, Micro QR, and
Function 182's bidirectional size response are explicitly post-v1. QR matrices
are valid and deterministic, but their mask choice is not yet claimed to match
a specific Epson firmware. The NT-5890K hardware case confirms native command
availability and the expected 84-dot QR dimensions; it does not yet compare
every logical module with a digitized physical print.

## Paper and device actions

| Command | Behavior | Implementation | Automated coverage | Hardware |
|---|---|---:|---:|---:|
| `GS V` Function A | Full or partial cut at the current position | Implemented | Implemented | Pending |
| `GS V` Function B | Feed to the cutter plus `n` units and cut, or apply the profile's no-autocutter feed behavior | Implemented | Implemented | No-cutter behavior validated on NT-5890K; autocutter pending |
| `ESC p` | Generate a cash-drawer pulse without printing | Implemented | Implemented | Pending |

Epson's no-autocutter baseline performs the documented
`n × vertical motion unit` feed and creates no sheet boundary for both
Function B modes. Profiles can mark either form ignored when compatible
firmware deviates. The connected NT-5890K feeds for full-cut mode 65 and
consumes partial-cut mode 66 without feeding. A cutter-equipped profile stores
the model-specific print-head-to-blade distance in dots. Function B adds that
distance to its explicit feed and then creates a full- or partial-cut sheet
boundary. Automated coverage fixes these semantics; physical autocutter
calibration remains pending.

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
