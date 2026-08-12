//! Rendering-surface abstraction and implementations.

mod mono;
pub(crate) mod tracing;

pub use mono::MonoSurface;
pub(crate) use mono::encode_png;

/// Drawing operations required by the ESC/POS interpreter.
///
/// Keeping this interface private lets alternate surfaces decorate the raster
/// surface without making rendering backends part of the public API.
pub(crate) trait RenderSurface: Sized {
    fn new(width: u32, scale: u32, antialias: bool) -> Self;
    /// Create a related surface, preserving decorator context such as an active
    /// command while starting with empty pixels and metadata.
    fn fork(&self, width: u32) -> Self;
    fn begin_command(&mut self, _offset: usize) {}
    fn end_command(&mut self) {}
    fn mark_region(&mut self, _x: u32, _y: u32, _width: u32, _height: u32) {}
    fn has_command_region(&self, _offset: usize) -> bool {
        false
    }
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn print_dot(&mut self, x: u32, y: u32);
    fn blend_subpixel(&mut self, sx: u32, sy: u32, value: u8, add: bool);
    fn composite_at(&mut self, source: &Self, left: u32, top: u32);
    fn ensure_height(&mut self, height: u32);
    fn clear(&mut self);
}
