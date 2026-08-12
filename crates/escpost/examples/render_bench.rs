//! Renders a selected workload in a loop and reports latency percentiles, for
//! comparing renderer changes across branches.

use std::env;
use std::time::Instant;

use escpost::render;
use escpost_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const CALIBRATION_HEX: &str = include_str!("../../../calibration/input.hex");

const ITERATIONS: usize = 10_000;
const COMMAND_REPETITIONS: usize = 1_000;

fn main() {
    let workload = env::args()
        .nth(1)
        .unwrap_or_else(|| "calibration".to_owned());
    let profile =
        compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML).expect("the profile should compile");
    let input = match workload.as_str() {
        "calibration" => calibration_input(),
        "commands" => command_heavy_input(),
        workload => panic!("unknown workload {workload:?}; use calibration or commands"),
    };

    // The warm-up render also proves the input renders at all and shows what
    // is being measured.
    let rendered = render(&input, &profile).expect("the calibration receipt should render");
    println!(
        "workload {workload}, input {} bytes, {} sheet(s)",
        input.len(),
        rendered.sheets.len(),
    );
    if let Some(sheet) = rendered.sheets.first() {
        println!(
            "first sheet {}x{} dots, png {} bytes",
            sheet.surface.width(),
            sheet.surface.height(),
            sheet.png.len(),
        );
    }

    let mut nanos: Vec<u128> = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let result = render(&input, &profile).expect("the calibration receipt should render");
        nanos.push(start.elapsed().as_nanos());
        std::hint::black_box(result);
    }
    nanos.sort_unstable();

    let micros_at =
        |fraction: f64| nanos[((nanos.len() - 1) as f64 * fraction) as usize] as f64 / 1_000.0;
    println!("renders {ITERATIONS}");
    println!("min    {:>10.1} us", micros_at(0.0));
    println!("median {:>10.1} us", micros_at(0.5));
    println!("p90    {:>10.1} us", micros_at(0.9));
    println!("max    {:>10.1} us", micros_at(1.0));
}

fn calibration_input() -> Vec<u8> {
    CALIBRATION_HEX
        .split_whitespace()
        .map(|token| u8::from_str_radix(token, 16).expect("the input should be hex bytes"))
        .collect()
}

fn command_heavy_input() -> Vec<u8> {
    // State-only commands isolate parser/dispatch overhead from glyph
    // rasterization, surface growth, and PNG encoding.
    const COMMANDS: &[u8] = &[
        0x1b, b'@', 0x1b, b'a', 0, 0x1b, b' ', 0, 0x1b, b'E', 0, 0x1b, b'-', 0, 0x1b, b'2', 0x1b,
        b'M', 0, 0x1b, b'R', 0, 0x1b, b't', 0, 0x1d, b'!', 0, 0x1d, b'B', 0,
    ];
    COMMANDS.repeat(COMMAND_REPETITIONS)
}
