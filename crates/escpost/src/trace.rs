//! Experimental command tracing model and crate-private collection seam.

use std::ops::Range;

use crate::state::Justification as StateJustification;
use crate::surface::tracing::TracingSurface;

/// Experimental semantic interpretation of a traced command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedCommand {
    SetJustification(Justification),
    TextByte(u8),
    LineFeed,
}

/// Experimental justification value used by command traces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justification {
    Left,
    Center,
    Right,
}

impl From<StateJustification> for Justification {
    fn from(value: StateJustification) -> Self {
        match value {
            StateJustification::Left => Self::Left,
            StateJustification::Center => Self::Center,
            StateJustification::Right => Self::Right,
        }
    }
}

/// Experimental typed state transition caused by a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateChange {
    Justification {
        before: Justification,
        after: Justification,
    },
}

/// Experimental logical printer position in printer-dot coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: u32,
    pub y: u32,
}

/// Experimental logical drawing bounds in printer-dot coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Experimental typed effect of a traced command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    StateChange(StateChange),
    Motion { before: Position, after: Position },
    Paint { bounds: PaintRegion },
}

/// Experimental trace entry for one safely decoded command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTrace {
    pub byte_range: Range<usize>,
    pub command: DecodedCommand,
    pub effects: Vec<Effect>,
}

/// Experimental commands associated with one rendered sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetTrace {
    pub commands: Vec<CommandTrace>,
}

/// Experimental ordered command trace grouped by rendered sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub sheets: Vec<SheetTrace>,
}

pub(crate) trait CommandSink {
    const ENABLED: bool;

    fn record(&mut self, sheet_index: usize, command: CommandTrace);
}

pub(crate) struct NoTrace;

impl CommandSink for NoTrace {
    const ENABLED: bool = false;

    #[inline]
    fn record(&mut self, _sheet_index: usize, _command: CommandTrace) {
        unreachable!("NoTrace commands are guarded by CommandSink::ENABLED")
    }
}

#[derive(Debug, Default)]
pub(crate) struct TraceCollector {
    commands: Vec<(usize, CommandTrace)>,
}

impl TraceCollector {
    pub(crate) fn finish(self, surfaces: &[TracingSurface]) -> Trace {
        let mut sheets = (0..surfaces.len())
            .map(|_| SheetTrace {
                commands: Vec::new(),
            })
            .collect::<Vec<_>>();

        for (sheet_index, mut command) in self.commands {
            if let Some(surface) = surfaces.get(sheet_index)
                && let Some(bounds) = command_bounds(surface, command.byte_range.start)
            {
                command.effects.push(Effect::Paint { bounds });
            }
            if let Some(sheet) = sheets.get_mut(sheet_index) {
                sheet.commands.push(command);
            }
        }

        Trace { sheets }
    }
}

fn command_bounds(surface: &TracingSurface, command_offset: usize) -> Option<PaintRegion> {
    let mut regions = surface
        .logical_regions
        .iter()
        .filter(|region| region.command_offset == command_offset);
    let first = regions.next()?;
    let mut left = first.x;
    let mut top = first.y;
    let mut right = first.x.saturating_add(first.width);
    let mut bottom = first.y.saturating_add(first.height);
    for region in regions {
        left = left.min(region.x);
        top = top.min(region.y);
        right = right.max(region.x.saturating_add(region.width));
        bottom = bottom.max(region.y.saturating_add(region.height));
    }
    Some(PaintRegion {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    })
}

impl CommandSink for TraceCollector {
    const ENABLED: bool = true;

    fn record(&mut self, sheet_index: usize, command: CommandTrace) {
        self.commands.push((sheet_index, command));
    }
}
