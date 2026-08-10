//! Experimental command tracing model and crate-private collection seam.

use std::ops::Range;

use crate::RenderError;
use crate::state::Justification as StateJustification;
use crate::state::PrinterState;
use crate::surface::RenderSurface;
use crate::surface::tracing::TracingSurface;

/// Experimental semantic interpretation of a traced command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedCommand {
    SetJustification(Justification),
    TextByte(u8),
    LineFeed,
    RasterImage,
    QrCode(Vec<u8>),
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

    fn begin_command(&mut self, sheet_index: usize, offset: usize);
    fn describe_command(&mut self, command: DecodedCommand, effects: Vec<Effect>);
    fn finish_command(&mut self, end_offset: usize);
}

#[inline]
pub(crate) fn execute_line_feed<S: RenderSurface, C: CommandSink>(
    state: &mut PrinterState<S>,
    command_sink: &mut C,
) -> Result<(), RenderError> {
    if C::ENABLED {
        let before = state.trace_line_feed_start_position();
        state.line_feed()?;
        let after = state.trace_position();
        command_sink.describe_command(
            DecodedCommand::LineFeed,
            (before != after)
                .then_some(Effect::Motion {
                    before: position(before),
                    after: position(after),
                })
                .into_iter()
                .collect(),
        );
    } else {
        state.line_feed()?;
    }
    Ok(())
}

#[inline]
pub(crate) fn execute_text_byte<S: RenderSurface, C: CommandSink>(
    state: &mut PrinterState<S>,
    command_sink: &mut C,
    byte: u8,
    offset: usize,
) -> Result<(), RenderError> {
    state.print_byte(byte, offset)?;
    if C::ENABLED {
        command_sink.describe_command(DecodedCommand::TextByte(byte), vec![]);
    }
    Ok(())
}

fn position((x, y): (u32, u32)) -> Position {
    Position { x, y }
}

pub(crate) struct NoTrace;

impl CommandSink for NoTrace {
    const ENABLED: bool = false;

    #[inline]
    fn begin_command(&mut self, _sheet_index: usize, _offset: usize) {
        unreachable!("NoTrace commands are guarded by CommandSink::ENABLED")
    }

    #[inline]
    fn describe_command(&mut self, _command: DecodedCommand, _effects: Vec<Effect>) {
        unreachable!("NoTrace commands are guarded by CommandSink::ENABLED")
    }

    #[inline]
    fn finish_command(&mut self, _end_offset: usize) {
        unreachable!("NoTrace commands are guarded by CommandSink::ENABLED")
    }
}

#[derive(Debug)]
struct PendingCommand {
    sheet_index: usize,
    start_offset: usize,
    description: Option<(DecodedCommand, Vec<Effect>)>,
}

#[derive(Debug, Default)]
pub(crate) struct TraceCollector {
    commands: Vec<(usize, CommandTrace)>,
    pending: Option<PendingCommand>,
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

    fn begin_command(&mut self, sheet_index: usize, offset: usize) {
        debug_assert!(self.pending.is_none());
        self.pending = Some(PendingCommand {
            sheet_index,
            start_offset: offset,
            description: None,
        });
    }

    fn describe_command(&mut self, command: DecodedCommand, effects: Vec<Effect>) {
        let pending = self
            .pending
            .as_mut()
            .expect("traced command descriptions require an active command");
        debug_assert!(pending.description.is_none());
        pending.description = Some((command, effects));
    }

    fn finish_command(&mut self, end_offset: usize) {
        let pending = self
            .pending
            .take()
            .expect("traced command finalization requires an active command");
        let Some((command, effects)) = pending.description else {
            return;
        };
        self.commands.push((
            pending.sheet_index,
            CommandTrace {
                byte_range: pending.start_offset..end_offset,
                command,
                effects,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use escpost_profiles::compile_profile;

    use super::{
        CommandTrace, DecodedCommand, Effect, Justification, Position, StateChange, TraceCollector,
    };
    use crate::surface::tracing::TracingSurface;
    use crate::{RenderOptions, render, render_surfaces_with_sink};

    const CAPABILITIES_JSON: &[u8] =
        include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
    const REFERENCE_PROFILE: &str = include_str!("../../../profiles/REFERENCE/profile.toml");

    #[test]
    fn traced_render_attributes_centered_text_to_its_input_byte() {
        let profile = compile_profile(CAPABILITIES_JSON, REFERENCE_PROFILE)
            .expect("the reference profile should compile");
        let input = [0x1b, b'a', 1, b'A', 0x0a];

        let ordinary = render(&input, &profile).expect("ordinary rendering should succeed");
        let mut commands = TraceCollector::default();
        let traced = render_surfaces_with_sink::<TracingSurface, _>(
            &input,
            &profile,
            &RenderOptions::default(),
            &mut commands,
        )
        .expect("traced rendering should succeed");
        let traced_sheet = &traced.surfaces[0];
        let trace = commands.finish(&traced.surfaces);
        let commands = &trace.sheets[0].commands;

        assert_eq!(
            commands[0],
            CommandTrace {
                byte_range: 0..3,
                command: DecodedCommand::SetJustification(Justification::Center),
                effects: vec![Effect::StateChange(StateChange::Justification {
                    before: Justification::Left,
                    after: Justification::Center,
                })],
            }
        );
        assert_eq!(commands[1].byte_range, 3..4);
        assert_eq!(commands[1].command, DecodedCommand::TextByte(b'A'));
        let [Effect::Paint { bounds }] = commands[1].effects.as_slice() else {
            panic!("the printable byte should have exactly one paint effect");
        };
        assert_eq!(
            (bounds.x, bounds.y, bounds.width, bounds.height),
            (282, 0, 12, 24)
        );
        assert_eq!(
            commands[2],
            CommandTrace {
                byte_range: 4..5,
                command: DecodedCommand::LineFeed,
                effects: vec![Effect::Motion {
                    before: Position { x: 294, y: 0 },
                    after: Position { x: 0, y: 30 },
                }],
            }
        );

        assert_eq!(traced_sheet.inner, ordinary.sheets[0].surface);
        let text_bounds = traced_sheet
            .logical_regions
            .iter()
            .filter(|region| region.command_offset == 3)
            .collect::<Vec<_>>();
        assert!(!text_bounds.is_empty());
        assert!(
            text_bounds
                .iter()
                .all(|region| region.x >= 282 && region.x < 294)
        );
        assert!(
            traced_sheet
                .logical_regions
                .iter()
                .all(|region| region.command_offset != 4),
            "LF must move the text without taking ownership of its pixels"
        );
    }

    #[test]
    fn unsupported_paint_commands_do_not_retain_trace_provenance() {
        let profile = compile_profile(CAPABILITIES_JSON, REFERENCE_PROFILE)
            .expect("the reference profile should compile");
        let input = [0x1b, b'*', 1, 1, 0, 0xff];
        let mut commands = TraceCollector::default();

        let traced = render_surfaces_with_sink::<TracingSurface, _>(
            &input,
            &profile,
            &RenderOptions::default(),
            &mut commands,
        )
        .expect("traced rendering should succeed");

        assert!(
            commands
                .finish(&traced.surfaces)
                .sheets
                .iter()
                .all(|sheet| sheet.commands.is_empty())
        );
        assert!(
            traced
                .surfaces
                .iter()
                .all(|surface| surface.logical_regions.is_empty()),
            "unsupported paint commands must not retain logical regions"
        );
    }
}
