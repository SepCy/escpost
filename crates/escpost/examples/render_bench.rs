//! Renders the shared calibration receipt in a loop and reports latency
//! percentiles, for comparing renderer changes across branches.

use std::time::Instant;

use escpost::render;
use escpost_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const CALIBRATION_HEX: &str = include_str!("../../../calibration/input.hex");

const ITERATIONS: usize = 10_000;

fn main() {
    let profile =
        compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML).expect("the profile should compile");
    let input: Vec<u8> = CALIBRATION_HEX
        .split_whitespace()
        .map(|token| u8::from_str_radix(token, 16).expect("the input should be hex bytes"))
        .collect();

    // The warm-up render also proves the input renders at all and shows what
    // is being measured.
    let rendered = render(&input, &profile).expect("the calibration receipt should render");
    let sheet = &rendered.sheets[0];
    println!(
        "input {} bytes, {} sheet(s), first sheet {}x{} dots, png {} bytes",
        input.len(),
        rendered.sheets.len(),
        sheet.surface.width(),
        sheet.surface.height(),
        sheet.png.len(),
    );

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
