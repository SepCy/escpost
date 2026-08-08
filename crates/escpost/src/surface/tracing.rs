//! Test-only tracing-surface proof.
//!
//! The types here validate command provenance through surface composition. They
//! intentionally do not define the eventual public trace schema.

use super::{MonoSurface, RenderSurface};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PaintedRegion {
    pub(crate) command_offset: usize,
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug)]
pub(crate) struct TracingSurface {
    pub(crate) inner: MonoSurface,
    pub(crate) painted_regions: Vec<PaintedRegion>,
    active_command: Option<usize>,
}

impl RenderSurface for TracingSurface {
    fn new(width: u32, scale: u32, antialias: bool) -> Self {
        Self {
            inner: MonoSurface::new(width, scale, antialias),
            painted_regions: Vec::new(),
            active_command: None,
        }
    }

    fn fork(&self, width: u32) -> Self {
        Self {
            inner: self.inner.fork(width),
            painted_regions: Vec::new(),
            active_command: self.active_command,
        }
    }

    fn begin_command(&mut self, offset: usize) {
        self.active_command = Some(offset);
    }

    fn width(&self) -> u32 {
        self.inner.width()
    }

    fn height(&self) -> u32 {
        self.inner.height()
    }

    fn print_dot(&mut self, x: u32, y: u32) {
        let was_printed = self.inner.is_printed(x, y);
        self.inner.print_dot(x, y);
        if !was_printed
            && self.inner.is_printed(x, y)
            && let Some(command_offset) = self.active_command
        {
            self.painted_regions.push(PaintedRegion {
                command_offset,
                x,
                y,
                width: 1,
                height: 1,
            });
        }
    }

    fn blend_subpixel(&mut self, sx: u32, sy: u32, value: u8, add: bool) {
        let before = self.inner.subpixel_coverage(sx, sy);
        self.inner.blend_subpixel(sx, sy, value, add);
        let after = self.inner.subpixel_coverage(sx, sy);
        if before != after
            && let Some(command_offset) = self.active_command
        {
            self.painted_regions.push(PaintedRegion {
                command_offset,
                x: sx / self.inner.scale(),
                y: sy / self.inner.scale(),
                width: 1,
                height: 1,
            });
        }
    }

    fn composite_at(&mut self, source: &Self, left: u32, top: u32) {
        self.inner.composite_at(&source.inner, left, top);
        self.painted_regions
            .extend(source.painted_regions.iter().map(|region| PaintedRegion {
                command_offset: region.command_offset,
                x: region.x + left,
                y: region.y + top,
                width: region.width,
                height: region.height,
            }));
    }

    fn ensure_height(&mut self, height: u32) {
        self.inner.ensure_height(height);
    }

    fn clear(&mut self) {
        self.inner.clear();
        self.painted_regions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{PaintedRegion, RenderSurface, TracingSurface};

    #[test]
    fn surface_decorator_preserves_painted_regions_through_composition() {
        let mut line = TracingSurface::new(32, 1, false);
        line.begin_command(7);
        line.print_dot(3, 4);
        line.ensure_height(9);
        line.blend_subpixel(7, 8, 255, true);
        let mut roll = TracingSurface::new(80, 1, false);

        roll.composite_at(&line, 20, 30);

        assert!(roll.inner.is_printed(23, 34));
        assert!(roll.inner.is_printed(27, 38));
        assert_eq!(
            roll.painted_regions,
            vec![
                PaintedRegion {
                    command_offset: 7,
                    x: 23,
                    y: 34,
                    width: 1,
                    height: 1,
                },
                PaintedRegion {
                    command_offset: 7,
                    x: 27,
                    y: 38,
                    width: 1,
                    height: 1,
                },
            ]
        );
    }
}
