# NT-5890K native symbols

This case uses the exact 176-byte stream sent to the connected 58 mm Netum
printer. It prints:

1. EAN-13 through NUL-terminated `GS k` Function A;
2. EAN-13 through length-prefixed `GS k` Function B; and
3. a Model 2 QR code through `GS ( k`.

Both EAN-13 commands receive the 12 digits `590123412345`. The printer and
renderer add check digit `7`, so the HRI text is `5901234123457`. The QR
payload is `NETUM-QR-TEST`.

The QR data fits a 21-module Version 1 symbol. Function 167 selects four dots
per module, producing an 84 × 84-dot symbol: approximately 10.5 mm at 203 DPI
and 21.9% of the 384-dot printable width. The observed paper symbol occupied
about one fifth of the paper width, consistent with this geometry.

## Physical observation

```text
date: 2026-07-26
input SHA-256: 665cd5dc465f6fa0a3f0994465943d4d97c3446177828b42b8dce24a16b1e3d2
renderer commit before profile correction: 7920ee6
printer profile: NT-5890K revision 5
printer USB identity: 0416:5011
manufacturer/product descriptor: YICHIP3121 / USB Portable Printer
serial: B120300001
connection: USB interface 0, OUT endpoint 0x01
result: both EAN-13 symbols and the Model 2 QR symbol printed
```

The physical result corrects the conservative `barcodeA`, `barcodeB`, and
`qrCode` flags inherited by the upstream profile from `simple`. It establishes
command availability. It does not by itself claim that the renderer's
representative HRI glyphs or QR mask are pixel-identical to this firmware.

The stream contains no cut or drawer command. Symbols are centered so their
surrounding paper supplies a quiet zone; ESC/POS does not add one to the
logical symbol dimensions.
