# escpost-render

`escpost-render` converts ESC/POS byte streams into deterministic, ordered PNG
sheets using an `escpost-profiles` printer profile. Render results include
warnings, device events, and reproducible profile information.

The renderer supports profile-driven text and layout, common single-byte code
pages, raster and column graphics, native one-dimensional barcodes, GS1-128,
Code 128, Model 2 QR codes, feeds, and cuts. See the [command coverage](https://github.com/receiptful/escpost/blob/main/docs/COMMAND_COVERAGE.md)
for the detailed support and validation matrix.

Licensed under the Apache License, Version 2.0. The bundled Noto Sans Mono font
is licensed separately under the SIL Open Font License 1.1.
