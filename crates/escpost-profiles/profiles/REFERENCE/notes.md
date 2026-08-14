# REFERENCE virtual profile

`REFERENCE` is not a physical printer and is not imported from
escpos-printer-db. It enables every capability represented by the current
ESCPost profile schema and uses the documented baseline behavior instead of
firmware quirks.

The profile gives commands concrete 203 DPI, 80 mm geometry because raster
output cannot be dimensionless. Its 576-dot width and 80-dot
print-head-to-cutter distance are deterministic virtual values, not claims
that ESC/POS standardizes those mechanism dimensions.

This profile removes printer-specific capability limitations. It does not turn
post-v1 command families that the renderer has not implemented into supported
commands. As new standard command handlers and profile capabilities are added,
`REFERENCE` must expose them without a printer-specific restriction.

The focused
`crates/escpost-render/tests/cases/mechanism/reference-full-and-partial-cuts` case creates three
ordered PNG sheets from two `GS V` Function B cuts. Virtual profiles need
automated golden evidence, but no physical `verification.toml`.
