//! Logical GS1 DataBar module generation.
//!
//! DataBar encoders return one Boolean per narrow module. They deliberately do
//! not know about printer dots, HRI, placement, or paper movement.

const PAIR_MULTIPLIER: u64 = 4_537_077;
const CHARACTER_MULTIPLIER: i32 = 1597;
const LIMITED_PAIR_MULTIPLIER: u64 = 2_013_571;

const GROUP_SUMS: [i32; 9] = [0, 161, 961, 2015, 2715, 0, 336, 1036, 1516];
const EVEN_OR_ODD_DIVISORS: [i32; 9] = [1, 10, 34, 70, 126, 4, 20, 48, 81];
const MODULE_TOTALS: [i32; 18] = [12, 10, 8, 6, 4, 5, 7, 9, 11, 4, 6, 8, 10, 12, 10, 8, 6, 4];
const WIDEST_ODD_ELEMENTS: [i32; 9] = [8, 6, 4, 3, 1, 2, 4, 6, 8];
const FINDER_PATTERNS: [[i32; 5]; 9] = [
    [3, 8, 2, 1, 1],
    [3, 5, 5, 1, 1],
    [3, 3, 7, 1, 1],
    [3, 1, 9, 1, 1],
    [2, 7, 4, 1, 1],
    [2, 5, 6, 1, 1],
    [2, 3, 8, 1, 1],
    [1, 5, 7, 1, 1],
    [1, 3, 9, 1, 1],
];
const CHECKSUM_WEIGHTS: [[i32; 8]; 4] = [
    [1, 3, 9, 27, 2, 6, 18, 54],
    [4, 12, 36, 29, 8, 24, 72, 58],
    [16, 48, 65, 37, 32, 17, 51, 74],
    [64, 34, 23, 69, 49, 68, 46, 59],
];
const LIMITED_GROUP_SUMS: [i32; 7] = [
    0, 183_064, 820_064, 1_000_776, 1_491_021, 1_979_845, 1_996_939,
];
const LIMITED_EVEN_DIVISORS: [i32; 7] = [28, 728, 6454, 203, 2408, 1, 16_632];
const LIMITED_ODD_MODULES: [i32; 7] = [17, 13, 9, 15, 11, 19, 7];
const LIMITED_WIDEST_ODD_ELEMENTS: [i32; 7] = [6, 5, 3, 5, 4, 8, 1];
const LIMITED_CHECKSUM_WEIGHTS: [[i32; 14]; 2] = [
    [1, 3, 9, 27, 81, 65, 17, 51, 64, 14, 42, 37, 22, 66],
    [20, 60, 2, 6, 18, 54, 73, 41, 34, 13, 39, 28, 84, 74],
];
// ISO/IEC 24724 Annex C maps the modulo-89 checksum to one space/bar pair.
// Storing the 89 sequence values is smaller and easier to audit than storing
// every expanded 14-element finder pattern.
const LIMITED_CHECKSUM_SEQUENCE: [i32; 89] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 45, 52, 57, 63, 64, 65,
    66, 73, 74, 75, 76, 77, 78, 79, 82, 126, 127, 128, 129, 130, 132, 141, 142, 143, 144, 145, 146,
    210, 211, 212, 213, 214, 215, 216, 217, 220, 316, 317, 318, 319, 320, 322, 323, 326, 337,
];

pub(crate) fn encode_omnidirectional(gtin_body: &[u8]) -> Option<Vec<bool>> {
    if gtin_body.len() != 13 {
        return None;
    }
    let value = gtin_body.iter().try_fold(0_u64, |value, digit| {
        digit
            .is_ascii_digit()
            .then(|| value * 10 + u64::from(digit - b'0'))
    })?;
    let widths = symbol_widths(value);
    Some(expand_widths(&widths))
}

pub(crate) fn encode_limited(gtin_body: &[u8]) -> Option<Vec<bool>> {
    if gtin_body.len() != 13 || !gtin_body.iter().all(u8::is_ascii_digit) || gtin_body[0] > b'1' {
        return None;
    }
    let value = gtin_body
        .iter()
        .fold(0_u64, |value, digit| value * 10 + u64::from(digit - b'0'));
    let pair_values = [
        (value / LIMITED_PAIR_MULTIPLIER) as i32,
        (value % LIMITED_PAIR_MULTIPLIER) as i32,
    ];
    let pair_widths = pair_values.map(limited_pair_widths);

    let checksum = pair_widths
        .iter()
        .zip(LIMITED_CHECKSUM_WEIGHTS)
        .flat_map(|(widths, weights)| widths.iter().zip(weights))
        .map(|(width, weight)| width * weight)
        .sum::<i32>()
        % 89;
    let finder_widths = limited_finder_widths(checksum as usize);

    let mut widths = [0_i32; 47];
    widths[0..2].fill(1);
    widths[2..16].copy_from_slice(&pair_widths[0]);
    widths[16..30].copy_from_slice(&finder_widths);
    widths[30..44].copy_from_slice(&pair_widths[1]);
    widths[44..46].fill(1);
    // The 2011 symbol revision reserves five light modules on the right.
    // Keeping them here makes the complete printed width 79 modules.
    widths[46] = 5;
    Some(expand_widths(&widths))
}

fn symbol_widths(value: u64) -> [i32; 46] {
    let left_pair = (value / PAIR_MULTIPLIER) as i32;
    let right_pair = (value % PAIR_MULTIPLIER) as i32;
    let character_values = [
        left_pair / CHARACTER_MULTIPLIER,
        left_pair % CHARACTER_MULTIPLIER,
        right_pair / CHARACTER_MULTIPLIER,
        right_pair % CHARACTER_MULTIPLIER,
    ];

    let mut character_widths = [[0_i32; 8]; 4];
    for (index, widths) in character_widths.iter_mut().enumerate() {
        let outside = index.is_multiple_of(2);
        let group = character_group(character_values[index], outside);
        let value = character_values[index] - GROUP_SUMS[group];
        let quotient = value / EVEN_OR_ODD_DIVISORS[group];
        let remainder = value % EVEN_OR_ODD_DIVISORS[group];
        let (odd_value, even_value) = if outside {
            (quotient, remainder)
        } else {
            (remainder, quotient)
        };
        widths.copy_from_slice(&interleaved_widths(
            i64::from(odd_value),
            i64::from(even_value),
            MODULE_TOTALS[group],
            MODULE_TOTALS[group + 9],
            WIDEST_ODD_ELEMENTS[group],
            !outside,
        ));
    }

    let mut checksum = 0_i32;
    for (character, widths) in character_widths.iter().enumerate() {
        for (element, width) in widths.iter().enumerate() {
            checksum += CHECKSUM_WEIGHTS[character][element] * width;
        }
    }
    checksum %= 79;
    // Values 8 and 72 are intentionally skipped by the DataBar finder table.
    if checksum >= 8 {
        checksum += 1;
    }
    if checksum >= 72 {
        checksum += 1;
    }
    let left_finder = (checksum / 9) as usize;
    let right_finder = (checksum % 9) as usize;

    let mut widths = [0_i32; 46];
    widths[0..2].fill(1);
    widths[44..46].fill(1);
    for index in 0..8 {
        widths[index + 2] = character_widths[0][index];
        widths[index + 15] = character_widths[1][7 - index];
        widths[index + 23] = character_widths[3][index];
        widths[index + 36] = character_widths[2][7 - index];
    }
    for index in 0..5 {
        widths[index + 10] = FINDER_PATTERNS[left_finder][index];
        widths[index + 31] = FINDER_PATTERNS[right_finder][4 - index];
    }
    widths
}

fn limited_pair_widths(mut value: i32) -> [i32; 14] {
    let group = LIMITED_GROUP_SUMS
        .iter()
        .rposition(|group_sum| value >= *group_sum)
        .unwrap_or(0);
    value -= LIMITED_GROUP_SUMS[group];
    let odd_value = value / LIMITED_EVEN_DIVISORS[group];
    let even_value = value % LIMITED_EVEN_DIVISORS[group];
    let odd = element_widths::<7>(
        i64::from(odd_value),
        LIMITED_ODD_MODULES[group],
        LIMITED_WIDEST_ODD_ELEMENTS[group],
        false,
    );
    let even = element_widths::<7>(
        i64::from(even_value),
        26 - LIMITED_ODD_MODULES[group],
        9 - LIMITED_WIDEST_ODD_ELEMENTS[group],
        true,
    );
    let mut widths = [0_i32; 14];
    interleave_widths(&mut widths, &odd, &even);
    widths
}

fn limited_finder_widths(checksum: usize) -> [i32; 14] {
    let sequence = LIMITED_CHECKSUM_SEQUENCE[checksum];
    let spaces = element_widths::<6>(i64::from(sequence / 21), 8, 3, false);
    let bars = element_widths::<6>(i64::from(sequence % 21), 8, 3, false);
    let mut widths = [0_i32; 14];
    interleave_widths(&mut widths[..12], &spaces, &bars);
    widths[12..14].fill(1);
    widths
}

fn character_group(value: i32, outside: bool) -> usize {
    let mut group = if outside { 0 } else { 5 };
    let last_group = if outside { 4 } else { 8 };
    while group < last_group {
        if value < GROUP_SUMS[group + 1] {
            return group;
        }
        group += 1;
    }
    group
}

fn interleaved_widths(
    odd_value: i64,
    even_value: i64,
    odd_modules: i32,
    even_modules: i32,
    widest_odd: i32,
    no_narrow_odd: bool,
) -> [i32; 8] {
    let odd = element_widths::<4>(odd_value, odd_modules, widest_odd, no_narrow_odd);
    let even = element_widths::<4>(even_value, even_modules, 9 - widest_odd, !no_narrow_odd);
    let mut widths = [0_i32; 8];
    interleave_widths(&mut widths, &odd, &even);
    widths
}

fn interleave_widths(output: &mut [i32], odd: &[i32], even: &[i32]) {
    for (index, (odd, even)) in odd.iter().zip(even).enumerate() {
        output[index * 2] = *odd;
        output[index * 2 + 1] = *even;
    }
}

/// Map one value to a fixed number of element widths using ISO/IEC 24724 Annex B.
fn element_widths<const ELEMENTS: usize>(
    mut value: i64,
    mut modules: i32,
    widest: i32,
    no_narrow: bool,
) -> [i32; ELEMENTS] {
    let element_count = ELEMENTS as i32;
    let mut widths = [0_i32; ELEMENTS];
    let mut narrow_mask = 0_u32;
    let mut element = 0_i32;
    while element < element_count - 1 {
        let mut width = 1_i32;
        narrow_mask |= 1 << element;
        let mut combinations;
        loop {
            combinations = binomial(
                i64::from(modules - width - 1),
                i64::from(element_count - element - 2),
            );
            if no_narrow
                && narrow_mask == 0
                && modules - width - (element_count - element - 1) >= element_count - element - 1
            {
                combinations -= binomial(
                    i64::from(modules - width - (element_count - element)),
                    i64::from(element_count - element - 2),
                );
            }
            if element_count - element - 1 > 1 {
                let mut too_wide = 0_i64;
                let mut candidate = modules - width - (element_count - element - 2);
                while candidate > widest {
                    too_wide += binomial(
                        i64::from(modules - width - candidate - 1),
                        i64::from(element_count - element - 3),
                    );
                    candidate -= 1;
                }
                combinations -= too_wide * i64::from(element_count - 1 - element);
            } else if modules - width > widest {
                combinations -= 1;
            }
            value -= combinations;
            if value < 0 {
                break;
            }
            width += 1;
            narrow_mask &= !(1 << element);
        }
        value += combinations;
        modules -= width;
        widths[element as usize] = width;
        element += 1;
    }
    widths[element as usize] = modules;
    widths
}

fn binomial(n: i64, r: i64) -> i64 {
    if n < 0 || r < 0 || r > n {
        return 0;
    }
    let r = r.min(n - r);
    (1..=r).fold(1_i64, |value, divisor| value * (n - r + divisor) / divisor)
}

fn expand_widths(widths: &[i32]) -> Vec<bool> {
    let mut modules = Vec::with_capacity(96);
    let mut dark = false;
    for width in widths {
        modules.extend(std::iter::repeat_n(dark, *width as usize));
        dark = !dark;
    }
    modules
}
