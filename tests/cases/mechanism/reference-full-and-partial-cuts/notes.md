# REFERENCE full and partial cuts

This focused virtual case makes multi-sheet preview behavior easy to inspect.
It uses the self-contained `REFERENCE` profile rather than claiming cutter
behavior for a physical printer.

The first `GS V` Function B command uses mode 65 (full cut). The second uses
mode 66 (partial cut). Both use an explicit feed operand of zero, so each
preceding sheet includes only the profile's fixed 80-dot
print-head-to-cutter distance. Content after each cut starts a new PNG:

1. `expected-001.png` ends at the full cut.
2. `expected-002.png` ends at the partial cut.
3. `expected-003.png` contains the content after both cuts.

The physical bridge left by a partial cut is not printable content. As defined
by DD-011, it still creates a new preview sheet.
