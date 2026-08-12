# Positioning and print area

This case makes positioning behavior visible with solid reversed-space cells:

1. `ESC $` places a 12 × 24-dot marker at the absolute 30-dot position.
2. `ESC SP` adds five dots after a 12-dot Font A cell, placing the next marker
   at x=17.
3. `GS L` and `GS W` select a 24-dot left margin and a 120-dot print area.
   `ESC a` centers a visible 12-dot reversed-space cell at physical x=78.
4. Epson behavior would use positive and negative `ESC \` movements to place
   markers at x=40 and x=20.
5. The calibrated NT-5890K applies the positive movement, then ignores both
   the second `ESC $` after printable data and the negative `ESC \`. Its two
   markers therefore occupy x=40 and x=52 and form one 24 × 24-dot block.

Using visible text cells keeps this positioning case independent of
model-specific `ESC *` painting artifacts. Column-image positioning remains
covered by automated interaction tests and the dedicated column-graphics
hardware section.

The NT-5890K profile maps one horizontal motion unit to one printer dot by
default. `GS P` is covered by the automated dot test because changing the
motion-unit pitch is harder to label clearly on this compact receipt.

This case is rendered during development but will only be sent to the printer
as part of the consolidated v1 hardware-conformance receipt.
