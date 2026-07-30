//! Bit-packed monochrome dot surface and its PNG encoding.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonoSurface {
    pub(crate) width: u32,
    pub(crate) height: u32,
    // Row-major, eight dots per byte, most-significant bit first, rows padded
    // to a whole byte: the same layout as PNG 1-bit grayscale, with inverted
    // polarity (1 = ink here, 0 = black in PNG). Padding bits stay zero so
    // derived equality cannot be confused by unused bits.
    rows: Vec<u8>,
}

impl MonoSurface {
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn is_printed(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        self.rows[(y * self.row_bytes() + x / 8) as usize] & (0x80 >> (x % 8)) != 0
    }

    fn row_bytes(&self) -> u32 {
        self.width.div_ceil(8)
    }
}

impl MonoSurface {
    pub(crate) fn new(width: u32) -> Self {
        Self {
            width,
            height: 0,
            rows: Vec::new(),
        }
    }

    pub(crate) fn print_dot(&mut self, x: u32, y: u32) {
        if x >= self.width {
            return;
        }

        self.ensure_height(y + 1);
        let index = (y * self.row_bytes() + x / 8) as usize;
        self.rows[index] |= 0x80 >> (x % 8);
    }

    pub(crate) fn clear_dot(&mut self, x: u32, y: u32) {
        if x >= self.width || y >= self.height {
            return;
        }

        let index = (y * self.row_bytes() + x / 8) as usize;
        self.rows[index] &= !(0x80 >> (x % 8));
    }

    pub(crate) fn composite_at(&mut self, source: &Self, left: u32, top: u32) {
        if source.height == 0 {
            return;
        }

        self.ensure_height(top.saturating_add(source.height));
        for y in 0..source.height {
            for x in 0..source.width {
                if source.is_printed(x, y) {
                    self.print_dot(left.saturating_add(x), top + y);
                }
            }
        }
    }

    pub(crate) fn ensure_height(&mut self, height: u32) {
        if height <= self.height {
            return;
        }

        self.rows.resize((height * self.row_bytes()) as usize, 0);
        self.height = height;
    }

    pub(crate) fn clear(&mut self) {
        self.height = 0;
        self.rows.clear();
    }
}

pub(crate) fn encode_png(surface: &MonoSurface) -> Result<Vec<u8>, png::EncodingError> {
    // MonoSurface already stores rows in PNG's 1-bit layout; only the polarity
    // differs (1 = ink on the surface, 0 = black in PNG), so encoding inverts
    // each byte. Inverted row padding becomes ones, which PNG ignores.
    let pixels: Vec<u8> = surface.rows.iter().map(|byte| !byte).collect();

    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, surface.width, surface.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::One);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&pixels)?;
    }

    Ok(encoded)
}
