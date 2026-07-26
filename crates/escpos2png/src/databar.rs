//! Logical GS1 DataBar module generation.
//!
//! DataBar encoders return one Boolean per narrow module. They deliberately do
//! not know about printer dots, HRI, placement, or paper movement.

const PAIR_MULTIPLIER: u64 = 4_537_077;
const CHARACTER_MULTIPLIER: i32 = 1597;

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
    let odd = element_widths(odd_value, odd_modules, widest_odd, no_narrow_odd);
    let even = element_widths(even_value, even_modules, 9 - widest_odd, !no_narrow_odd);
    let mut widths = [0_i32; 8];
    for index in 0..4 {
        widths[index * 2] = odd[index];
        widths[index * 2 + 1] = even[index];
    }
    widths
}

/// Map one value to four element widths using ISO/IEC 24724 Annex B.
fn element_widths(mut value: i64, mut modules: i32, widest: i32, no_narrow: bool) -> [i32; 4] {
    const ELEMENTS: i32 = 4;

    let mut widths = [0_i32; ELEMENTS as usize];
    let mut narrow_mask = 0_u32;
    let mut element = 0_i32;
    while element < ELEMENTS - 1 {
        let mut width = 1_i32;
        narrow_mask |= 1 << element;
        let mut combinations;
        loop {
            combinations = binomial(
                i64::from(modules - width - 1),
                i64::from(ELEMENTS - element - 2),
            );
            if no_narrow
                && narrow_mask == 0
                && modules - width - (ELEMENTS - element - 1) >= ELEMENTS - element - 1
            {
                combinations -= binomial(
                    i64::from(modules - width - (ELEMENTS - element)),
                    i64::from(ELEMENTS - element - 2),
                );
            }
            if ELEMENTS - element - 1 > 1 {
                let mut too_wide = 0_i64;
                let mut candidate = modules - width - (ELEMENTS - element - 2);
                while candidate > widest {
                    too_wide += binomial(
                        i64::from(modules - width - candidate - 1),
                        i64::from(ELEMENTS - element - 3),
                    );
                    candidate -= 1;
                }
                combinations -= too_wide * i64::from(ELEMENTS - 1 - element);
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
