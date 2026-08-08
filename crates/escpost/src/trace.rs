//! Crate-private command tracing model and compile-time collection seam.

use std::ops::Range;

#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};

use crate::state::Justification;
#[cfg(test)]
use crate::surface::tracing::TracingSurface;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecodedCommand {
    SetJustification(Justification),
    TextByte(u8),
    LineFeed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StateChange {
    Justification {
        before: Justification,
        after: Justification,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Position {
    pub(crate) x: u32,
    pub(crate) y: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaintRegion {
    pub(crate) sheet_index: usize,
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum Effect {
    StateChange(StateChange),
    Motion { before: Position, after: Position },
    Flush { commands: Vec<Range<usize>> },
    Paint { regions: Vec<PaintRegion> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandTrace {
    pub(crate) byte_range: Range<usize>,
    pub(crate) command: DecodedCommand,
    pub(crate) effects: Vec<Effect>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Trace {
    pub(crate) commands: Vec<CommandTrace>,
}

pub(crate) trait CommandSink {
    const ENABLED: bool;

    fn record(&mut self, command: CommandTrace);
}

pub(crate) struct NoTrace;

impl CommandSink for NoTrace {
    const ENABLED: bool = false;

    #[inline]
    fn record(&mut self, _command: CommandTrace) {
        unreachable!("NoTrace commands are guarded by CommandSink::ENABLED")
    }
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct TraceCollector {
    commands: Vec<CommandTrace>,
    pending_printable_commands: Vec<Range<usize>>,
}

#[cfg(test)]
impl TraceCollector {
    pub(crate) fn finish(mut self, surfaces: &[TracingSurface]) -> Trace {
        let mut dots_by_command = BTreeMap::<usize, BTreeMap<usize, BTreeSet<(u32, u32)>>>::new();
        for (sheet_index, surface) in surfaces.iter().enumerate() {
            for region in &surface.painted_regions {
                let dots = dots_by_command
                    .entry(region.command_offset)
                    .or_default()
                    .entry(sheet_index)
                    .or_default();
                for y in region.y..region.y + region.height {
                    for x in region.x..region.x + region.width {
                        dots.insert((x, y));
                    }
                }
            }
        }

        for command in &mut self.commands {
            let Some(sheets) = dots_by_command.remove(&command.byte_range.start) else {
                continue;
            };
            let mut regions = Vec::new();
            for (sheet_index, dots) in sheets {
                regions.extend(coalesce_dots(sheet_index, &dots));
            }
            if !regions.is_empty() {
                command.effects.push(Effect::Paint { regions });
            }
        }

        Trace {
            commands: self.commands,
        }
    }
}

#[cfg(test)]
fn coalesce_dots(sheet_index: usize, dots: &BTreeSet<(u32, u32)>) -> Vec<PaintRegion> {
    let mut rows = BTreeMap::<u32, Vec<u32>>::new();
    for &(x, y) in dots {
        rows.entry(y).or_default().push(x);
    }

    let mut rectangles: Vec<PaintRegion> = Vec::new();
    for (y, xs) in rows {
        let mut run_start = xs[0];
        let mut previous = xs[0];
        for x in xs.into_iter().skip(1).chain(std::iter::once(u32::MAX)) {
            if x == previous.saturating_add(1) {
                previous = x;
                continue;
            }

            let width = previous - run_start + 1;
            if let Some(rectangle) = rectangles.iter_mut().rev().find(|rectangle| {
                rectangle.x == run_start
                    && rectangle.width == width
                    && rectangle.y + rectangle.height == y
            }) {
                rectangle.height += 1;
            } else {
                rectangles.push(PaintRegion {
                    sheet_index,
                    x: run_start,
                    y,
                    width,
                    height: 1,
                });
            }

            run_start = x;
            previous = x;
        }
    }
    rectangles
}

#[cfg(test)]
impl CommandSink for TraceCollector {
    const ENABLED: bool = true;

    fn record(&mut self, mut command: CommandTrace) {
        if matches!(&command.command, DecodedCommand::LineFeed)
            && !self.pending_printable_commands.is_empty()
        {
            command.effects.push(Effect::Flush {
                commands: std::mem::take(&mut self.pending_printable_commands),
            });
        }
        if matches!(&command.command, DecodedCommand::TextByte(_)) {
            self.pending_printable_commands
                .push(command.byte_range.clone());
        }
        self.commands.push(command);
    }
}
