use escpost::{DecodedCommand, Effect, Justification, Position, StateChange, render_with_trace};
use escpost_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const REFERENCE_PROFILE: &str = include_str!("../../../profiles/REFERENCE/profile.toml");
const NT_5890K_PROFILE: &str = include_str!("../../../profiles/NT-5890K/profile.toml");

#[test]
fn experimental_trace_exposes_sheet_commands_and_logical_bounds() {
    let profile = compile_profile(CAPABILITIES_JSON, REFERENCE_PROFILE)
        .expect("the reference profile should compile");
    let traced = render_with_trace(&[0x1b, b'a', 1, b'A', 0x0a], &profile)
        .expect("traced rendering should succeed");

    assert_eq!(traced.render.sheets.len(), 1);
    assert_eq!(traced.trace.sheets.len(), traced.render.sheets.len());
    let commands = &traced.trace.sheets[0].commands;
    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].byte_range, 0..3);
    assert_eq!(
        commands[0].command,
        DecodedCommand::SetJustification(Justification::Center)
    );
    assert_eq!(
        commands[0].effects,
        [Effect::StateChange(StateChange::Justification {
            before: Justification::Left,
            after: Justification::Center,
        })]
    );
    assert_eq!(commands[1].byte_range, 3..4);
    assert_eq!(commands[1].command, DecodedCommand::TextByte(b'A'));
    let [Effect::Paint { bounds }] = commands[1].effects.as_slice() else {
        panic!("the printable byte should expose its logical bounds");
    };
    assert_eq!(
        (bounds.x, bounds.y, bounds.width, bounds.height),
        (282, 0, 12, 24)
    );
    assert_eq!(commands[2].byte_range, 4..5);
    assert_eq!(commands[2].command, DecodedCommand::LineFeed);
    assert_eq!(
        commands[2].effects,
        [Effect::Motion {
            before: Position { x: 12, y: 0 },
            after: Position { x: 0, y: 30 },
        }]
    );
}

#[test]
fn ignored_justification_has_no_state_change_effect() {
    let profile = compile_profile(CAPABILITIES_JSON, REFERENCE_PROFILE)
        .expect("the reference profile should compile");
    let traced = render_with_trace(&[b'A', 0x1b, b'a', 1, 0x0a], &profile)
        .expect("traced rendering should succeed");

    let justification = traced.trace.sheets[0]
        .commands
        .iter()
        .find(|command| matches!(command.command, DecodedCommand::SetJustification(_)))
        .expect("ESC a should still have a command entry");
    assert!(
        justification.effects.is_empty(),
        "an ignored ESC a must not claim a state transition"
    );
}

#[test]
fn a_space_has_logical_bounds_without_ink() {
    let profile = compile_profile(CAPABILITIES_JSON, REFERENCE_PROFILE)
        .expect("the reference profile should compile");
    let traced =
        render_with_trace(&[b' ', 0x0a], &profile).expect("traced rendering should succeed");

    let [Effect::Paint { bounds }] = traced.trace.sheets[0].commands[0].effects.as_slice() else {
        panic!("a space should expose its logical character cell");
    };
    assert_eq!(
        (bounds.x, bounds.y, bounds.width, bounds.height),
        (0, 0, 12, 24)
    );
    assert!(!traced.render.sheets[0].surface.is_printed(0, 0));
}

#[test]
fn commands_are_grouped_under_the_sheet_active_when_they_execute() {
    let profile = compile_profile(CAPABILITIES_JSON, REFERENCE_PROFILE)
        .expect("the reference profile should compile");
    let traced = render_with_trace(&[b'A', 0x0a, 0x1d, b'V', 0, b'B', 0x0a], &profile)
        .expect("traced rendering should succeed");

    assert_eq!(traced.render.sheets.len(), 2);
    assert_eq!(traced.trace.sheets.len(), 2);
    assert_eq!(traced.trace.sheets[0].commands.len(), 2);
    assert_eq!(
        traced.trace.sheets[0].commands[0].command,
        DecodedCommand::TextByte(b'A')
    );
    assert_eq!(traced.trace.sheets[1].commands.len(), 2);
    assert_eq!(
        traced.trace.sheets[1].commands[0].command,
        DecodedCommand::TextByte(b'B')
    );
}

#[test]
fn a_profile_suppressed_line_feed_has_no_motion_effect() {
    let profile = compile_profile(CAPABILITIES_JSON, NT_5890K_PROFILE)
        .expect("the NT-5890K profile should compile");
    let traced = render_with_trace(&[0x1d, b'v', b'0', 0, 1, 0, 1, 0, 0x80, 0x0a], &profile)
        .expect("traced rendering should succeed");

    let line_feed = traced.trace.sheets[0]
        .commands
        .iter()
        .find(|command| command.command == DecodedCommand::LineFeed)
        .expect("LF should retain its command entry");
    assert!(
        line_feed.effects.is_empty(),
        "a profile-suppressed LF must not claim motion"
    );
}
