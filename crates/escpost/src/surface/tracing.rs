//! Provenance-decorating surface for experimental command tracing.
//!
//! The types here validate command provenance through surface composition. They
//! are private implementation details rather than the public trace schema.

use super::{MonoSurface, RenderSurface};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LogicalRegion {
    pub(crate) command_offset: usize,
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug)]
pub(crate) struct TracingSurface {
    pub(crate) inner: MonoSurface,
    pub(crate) logical_regions: Vec<LogicalRegion>,
    active_command: Option<usize>,
}

impl RenderSurface for TracingSurface {
    fn new(width: u32, scale: u32, antialias: bool) -> Self {
        Self {
            inner: MonoSurface::new(width, scale, antialias),
            logical_regions: Vec::new(),
            active_command: None,
        }
    }

    fn fork(&self, width: u32) -> Self {
        Self {
            inner: self.inner.fork(width),
            logical_regions: Vec::new(),
            active_command: self.active_command,
        }
    }

    fn begin_command(&mut self, offset: usize) {
        self.active_command = Some(offset);
    }

    fn end_command(&mut self) {
        self.active_command = None;
    }

    fn mark_region(&mut self, x: u32, y: u32, width: u32, height: u32) {
        if let Some(command_offset) = self.active_command {
            self.logical_regions.push(LogicalRegion {
                command_offset,
                x,
                y,
                width,
                height,
            });
        }
    }

    fn has_command_region(&self, offset: usize) -> bool {
        self.logical_regions
            .iter()
            .any(|region| region.command_offset == offset)
    }

    fn width(&self) -> u32 {
        self.inner.width()
    }

    fn height(&self) -> u32 {
        self.inner.height()
    }

    fn print_dot(&mut self, x: u32, y: u32) {
        self.inner.print_dot(x, y);
    }

    fn blend_subpixel(&mut self, sx: u32, sy: u32, value: u8, add: bool) {
        self.inner.blend_subpixel(sx, sy, value, add);
    }

    fn composite_at(&mut self, source: &Self, left: u32, top: u32) {
        self.inner.composite_at(&source.inner, left, top);
        self.logical_regions
            .extend(source.logical_regions.iter().map(|region| LogicalRegion {
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
        self.logical_regions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{LogicalRegion, RenderSurface, TracingSurface};

    #[test]
    fn surface_decorator_preserves_logical_regions_through_composition() {
        let mut line = TracingSurface::new(32, 1, false);
        line.begin_command(7);
        line.mark_region(3, 4, 12, 24);
        let mut roll = TracingSurface::new(80, 1, false);

        roll.composite_at(&line, 20, 30);

        assert_eq!(roll.inner.height(), 0);
        assert_eq!(
            roll.logical_regions,
            vec![LogicalRegion {
                command_offset: 7,
                x: 23,
                y: 34,
                width: 12,
                height: 24,
            }]
        );
    }
}
