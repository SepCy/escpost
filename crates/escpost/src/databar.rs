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
const EXPANDED_GROUP_SUMS: [i32; 5] = [0, 348, 1388, 2948, 3988];
const EXPANDED_EVEN_DIVISORS: [i32; 5] = [4, 20, 52, 104, 204];
const EXPANDED_ODD_MODULES: [i32; 5] = [12, 10, 8, 6, 4];
const EXPANDED_WIDEST_ODD_ELEMENTS: [i32; 5] = [7, 5, 4, 3, 1];
const EXPANDED_CHECKSUM_WEIGHTS: [[i32; 8]; 23] = [
    [1, 3, 9, 27, 81, 32, 96, 77],
    [20, 60, 180, 118, 143, 7, 21, 63],
    [189, 145, 13, 39, 117, 140, 209, 205],
    [193, 157, 49, 147, 19, 57, 171, 91],
    [62, 186, 136, 197, 169, 85, 44, 132],
    [185, 133, 188, 142, 4, 12, 36, 108],
    [113, 128, 173, 97, 80, 29, 87, 50],
    [150, 28, 84, 41, 123, 158, 52, 156],
    [46, 138, 203, 187, 139, 206, 196, 166],
    [76, 17, 51, 153, 37, 111, 122, 155],
    [43, 129, 176, 106, 107, 110, 119, 146],
    [16, 48, 144, 10, 30, 90, 59, 177],
    [109, 116, 137, 200, 178, 112, 125, 164],
    [70, 210, 208, 202, 184, 130, 179, 115],
    [134, 191, 151, 31, 93, 68, 204, 190],
    [148, 22, 66, 198, 172, 94, 71, 2],
    [6, 18, 54, 162, 64, 192, 154, 40],
    [120, 149, 25, 75, 14, 42, 126, 167],
    [79, 26, 78, 23, 69, 207, 199, 175],
    [103, 98, 83, 38, 114, 131, 182, 124],
    [161, 61, 183, 127, 170, 88, 53, 159],
    [55, 165, 73, 8, 24, 72, 5, 15],
    [45, 135, 194, 160, 58, 174, 100, 89],
];
const EXPANDED_FINDER_PATTERNS: [[i32; 5]; 12] = [
    [1, 8, 4, 1, 1],
    [1, 1, 4, 8, 1],
    [3, 6, 4, 1, 1],
    [1, 1, 4, 6, 3],
    [3, 4, 6, 1, 1],
    [1, 1, 6, 4, 3],
    [3, 2, 8, 1, 1],
    [1, 1, 8, 2, 3],
    [2, 6, 5, 1, 1],
    [1, 1, 5, 6, 2],
    [2, 2, 9, 1, 1],
    [1, 1, 9, 2, 2],
];
const EXPANDED_FINDER_SEQUENCES: [[usize; 11]; 10] = [
    [1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 4, 3, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 6, 3, 8, 0, 0, 0, 0, 0, 0, 0],
    [1, 10, 3, 8, 5, 0, 0, 0, 0, 0, 0],
    [1, 10, 3, 8, 7, 12, 0, 0, 0, 0, 0],
    [1, 10, 3, 8, 9, 12, 11, 0, 0, 0, 0],
    [1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0],
    [1, 2, 3, 4, 5, 6, 7, 10, 9, 0, 0],
    [1, 2, 3, 4, 5, 6, 7, 10, 11, 12, 0],
    [1, 2, 3, 4, 5, 8, 7, 10, 9, 12, 11],
];
const EXPANDED_WEIGHT_ROWS: [[usize; 21]; 10] = [
    [
        0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    [
        0, 5, 6, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    [
        0, 9, 10, 3, 4, 13, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    [
        0, 17, 18, 3, 4, 13, 14, 7, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    [
        0, 17, 18, 3, 4, 13, 14, 11, 12, 21, 22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    [
        0, 17, 18, 3, 4, 13, 14, 15, 16, 21, 22, 19, 20, 0, 0, 0, 0, 0, 0, 0, 0,
    ],
    [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 0, 0, 0, 0, 0, 0,
    ],
    [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 17, 18, 15, 16, 0, 0, 0, 0,
    ],
    [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 17, 18, 19, 20, 21, 22, 0, 0,
    ],
    [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 13, 14, 11, 12, 17, 18, 15, 16, 21, 22, 19, 20,
    ],
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum GeneralFieldMode {
    Numeric,
    Alphanumeric,
    IsoIec,
}

struct GeneralFieldEncoding {
    mode: GeneralFieldMode,
    last_digit: Option<u8>,
}

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

pub(crate) fn encode_expanded(data: &[u8]) -> Option<Vec<bool>> {
    let bits = expanded_binary(data)?;
    let data_widths = bits
        .chunks_exact(12)
        .map(|character| expanded_character_widths(bits_to_value(character)))
        .collect::<Vec<_>>();
    let symbol_characters = data_widths.len() + 1;
    let weight_row = EXPANDED_WEIGHT_ROWS.get((data_widths.len() - 2) / 2)?;
    let checksum = data_widths
        .iter()
        .enumerate()
        .flat_map(|(character, widths)| {
            widths
                .iter()
                .zip(EXPANDED_CHECKSUM_WEIGHTS[weight_row[character]])
        })
        .map(|(width, weight)| width * weight)
        .sum::<i32>()
        % 211;
    let check_value = 211 * (symbol_characters as i32 - 4) + checksum;
    let check_widths = expanded_character_widths(check_value);

    let codeblocks = symbol_characters.div_ceil(2);
    let pattern_width = codeblocks * 5 + symbol_characters * 8 + 4;
    let mut widths = vec![0_i32; pattern_width];
    let finder_sequence = EXPANDED_FINDER_SEQUENCES.get((symbol_characters - 1) / 2 - 1)?;
    for (codeblock, finder) in finder_sequence.iter().take(codeblocks).enumerate() {
        let finder = EXPANDED_FINDER_PATTERNS.get(finder.checked_sub(1)?)?;
        widths[codeblock * 21 + 10..codeblock * 21 + 15].copy_from_slice(finder);
    }
    widths[2..10].copy_from_slice(&check_widths);
    for character in (1..data_widths.len()).step_by(2) {
        let start = (character - 1) / 2 * 21 + 23;
        widths[start..start + 8].copy_from_slice(&data_widths[character]);
    }
    for character in (0..data_widths.len()).step_by(2) {
        let start = character / 2 * 21 + 15;
        for (target, source) in widths[start..start + 8]
            .iter_mut()
            .zip(data_widths[character].iter().rev())
        {
            *target = *source;
        }
    }
    widths[0..2].fill(1);
    widths[pattern_width - 2..].fill(1);
    Some(expand_widths(&widths))
}

fn expanded_binary(data: &[u8]) -> Option<Vec<bool>> {
    // ISO/IEC 24724 limits the reduced GS1 data, before compaction, to 77
    // bytes. The later bit-length check catches some shorter inputs whose
    // characters still need too much space.
    if data.len() > 77 {
        return None;
    }

    let encoding_method = select_expanded_encoding_method(data);
    let read_position = expanded_compressed_field_end(encoding_method, data.len());
    let mut bits = vec![false]; // No composite-component linkage.
    append_expanded_method(&mut bits, encoding_method);

    if !data
        .get(..read_position)?
        .iter()
        .all(|byte| byte.is_ascii_digit() || *byte == 0x1d)
    {
        return None;
    }

    append_expanded_compressed_field(&mut bits, data, encoding_method)?;
    let general_field = encode_general_field(&mut bits, data.get(read_position..)?)?;
    pad_expanded_binary(
        &mut bits,
        encoding_method,
        general_field.mode,
        general_field.last_digit,
    )?;
    Some(bits)
}

fn select_expanded_encoding_method(data: &[u8]) -> u8 {
    let mut method = if data.len() >= 16 && data.starts_with(b"01") {
        1
    } else {
        2
    };
    if data.len() < 20 || method != 1 || data[2] != b'9' || data[16] != b'3' {
        return method;
    }

    if data.len() >= 26 && data.get(17..19) == Some(b"10") {
        let Some(weight) = parse_decimal(&data[20..26]) else {
            return method;
        };
        if weight > 99_999 {
            return method;
        }
        if data.len() == 26 {
            return if data[19] == b'3' && weight <= 32_767 {
                3
            } else {
                7
            };
        }
        if data.len() == 34
            && data[26] == b'1'
            && matches!(data[27], b'1' | b'3' | b'5' | b'7')
            && expanded_date_value(&data[28..34]).is_some()
        {
            method = 6 + (data[27] - b'0');
        }
    } else if data.len() >= 26 && data.get(17..19) == Some(b"20") {
        let Some(weight) = parse_decimal(&data[20..26]) else {
            return method;
        };
        if weight > 99_999 {
            return method;
        }
        if data.len() == 26 {
            return if (data[19] == b'2' && weight <= 9_999)
                || (data[19] == b'3' && weight <= 22_767)
            {
                4
            } else {
                8
            };
        }
        if data.len() == 34
            && data[26] == b'1'
            && matches!(data[27], b'1' | b'3' | b'5' | b'7')
            && expanded_date_value(&data[28..34]).is_some()
        {
            method = 7 + (data[27] - b'0');
        }
    } else if data[17] == b'9' && matches!(data[19], b'0'..=b'3') {
        if data[18] == b'2' {
            method = 5;
        } else if data.len() >= 23 && data[18] == b'3' && parse_decimal(&data[20..23]).is_some() {
            method = 6;
        }
    }
    method
}

fn expanded_compressed_field_end(method: u8, data_length: usize) -> usize {
    match method {
        1 => 16,
        2 => 0,
        3 | 4 => 26,
        5 => 20,
        6 => 23,
        7..=14 => data_length,
        _ => unreachable!("the encoding method is selected above"),
    }
}

fn append_expanded_method(bits: &mut Vec<bool>, method: u8) {
    match method {
        1 => append_bits(bits, 4, 3),
        2 => append_bits(bits, 0, 4),
        3 | 4 => append_bits(bits, u32::from(4 + method - 3), 4),
        5 => append_bits(bits, 0x30, 7),
        6 => append_bits(bits, 0x34, 7),
        7..=14 => append_bits(bits, u32::from(56 + method - 7), 7),
        _ => unreachable!("the encoding method is selected above"),
    }
}

fn append_expanded_compressed_field(bits: &mut Vec<bool>, data: &[u8], method: u8) -> Option<()> {
    match method {
        1 => {
            append_bits(bits, u32::from(*data.get(2)? - b'0'), 4);
            append_expanded_gtin(bits, data)?;
        }
        2 => {}
        3 | 4 => {
            append_expanded_gtin(bits, data)?;
            let mut weight = parse_decimal(data.get(20..26)?)?;
            if method == 4 && data[19] == b'3' {
                weight += 10_000;
            }
            append_bits(bits, weight, 15);
        }
        5 | 6 => {
            append_expanded_gtin(bits, data)?;
            append_bits(bits, u32::from(*data.get(19)? - b'0'), 2);
            if method == 6 {
                append_bits(bits, parse_decimal(data.get(20..23)?)?, 10);
            }
        }
        7..=14 => {
            append_expanded_gtin(bits, data)?;
            let mut weight = [0_u8; 6];
            weight[0] = data[19];
            weight[1..].copy_from_slice(data.get(21..26)?);
            append_bits(bits, parse_decimal(&weight)?, 20);
            let date = if data.len() == 34 {
                expanded_date_value(data.get(28..34)?)?
            } else {
                38_400
            };
            append_bits(bits, date, 16);
        }
        _ => unreachable!("the encoding method is selected above"),
    }
    Some(())
}

fn append_expanded_gtin(bits: &mut Vec<bool>, data: &[u8]) -> Option<()> {
    for start in (3..15).step_by(3) {
        append_bits(bits, parse_decimal(data.get(start..start + 3)?)?, 10);
    }
    Some(())
}

fn encode_general_field(bits: &mut Vec<bool>, data: &[u8]) -> Option<GeneralFieldEncoding> {
    let mut mode = GeneralFieldMode::Numeric;
    let mut position = 0;
    let mut last_digit = None;

    while position < data.len() {
        let character_type = general_field_type(data[position])?;
        match mode {
            GeneralFieldMode::Numeric => {
                if position + 1 < data.len() {
                    if character_type != GeneralFieldMode::Numeric
                        || general_field_type(data[position + 1])? != GeneralFieldMode::Numeric
                    {
                        append_bits(bits, 0, 4);
                        mode = GeneralFieldMode::Alphanumeric;
                    } else {
                        let first = general_field_numeric_value(data[position])?;
                        let second = general_field_numeric_value(data[position + 1])?;
                        append_bits(bits, 11 * first + second + 8, 7);
                        position += 2;
                    }
                } else if character_type != GeneralFieldMode::Numeric {
                    append_bits(bits, 0, 4);
                    mode = GeneralFieldMode::Alphanumeric;
                } else {
                    last_digit = Some(data[position]);
                    position += 1;
                }
            }
            GeneralFieldMode::Alphanumeric => {
                if data[position] == 0x1d {
                    append_bits(bits, 15, 5);
                    mode = GeneralFieldMode::Numeric;
                    position += 1;
                } else if character_type == GeneralFieldMode::IsoIec {
                    append_bits(bits, 4, 5);
                    mode = GeneralFieldMode::IsoIec;
                } else if general_field_next(data, position, 6, GeneralFieldMode::Numeric, None)
                    || general_field_next_terminates(
                        data,
                        position,
                        4,
                        5,
                        GeneralFieldMode::Numeric,
                    )
                {
                    append_bits(bits, 0, 3);
                    mode = GeneralFieldMode::Numeric;
                } else {
                    append_general_field_alphanumeric(bits, data[position])?;
                    position += 1;
                }
            }
            GeneralFieldMode::IsoIec => {
                if data[position] == 0x1d {
                    append_bits(bits, 15, 5);
                    mode = GeneralFieldMode::Numeric;
                    position += 1;
                } else {
                    let next_ten_have_no_iso =
                        general_field_next_none(data, position, 10, GeneralFieldMode::IsoIec);
                    if next_ten_have_no_iso
                        && general_field_next(data, position, 4, GeneralFieldMode::Numeric, None)
                    {
                        append_bits(bits, 0, 3);
                        mode = GeneralFieldMode::Numeric;
                    } else if next_ten_have_no_iso
                        && general_field_next(
                            data,
                            position,
                            5,
                            GeneralFieldMode::Alphanumeric,
                            Some(GeneralFieldMode::Numeric),
                        )
                    {
                        append_bits(bits, 4, 5);
                        mode = GeneralFieldMode::Alphanumeric;
                    } else {
                        append_general_field_iso_iec(bits, data[position])?;
                        position += 1;
                    }
                }
            }
        }
    }

    Some(GeneralFieldEncoding { mode, last_digit })
}

fn append_general_field_alphanumeric(bits: &mut Vec<bool>, byte: u8) -> Option<()> {
    let value = if byte.is_ascii_digit() {
        u32::from(byte - 43)
    } else if byte.is_ascii_uppercase() {
        u32::from(byte - 33)
    } else {
        let position = b"*,-./".iter().position(|candidate| *candidate == byte)?;
        u32::try_from(position).ok()? + 58
    };
    append_bits(bits, value, if byte.is_ascii_digit() { 5 } else { 6 });
    Some(())
}

fn append_general_field_iso_iec(bits: &mut Vec<bool>, byte: u8) -> Option<()> {
    let (value, count) = if byte.is_ascii_digit() {
        (u32::from(byte - 43), 5)
    } else if byte.is_ascii_uppercase() {
        (u32::from(byte - 1), 7)
    } else if byte.is_ascii_lowercase() {
        (u32::from(byte - 7), 7)
    } else if byte == b'$' {
        // '$' occupies the value immediately before the punctuation table.
        (231, 8)
    } else {
        let position = b"!\"%&'()*+,-./:;<=>?_ "
            .iter()
            .position(|candidate| *candidate == byte)?;
        (u32::try_from(position).ok()? + 232, 8)
    };
    append_bits(bits, value, count);
    Some(())
}

fn general_field_type(byte: u8) -> Option<GeneralFieldMode> {
    if byte == 0x1d || byte.is_ascii_digit() {
        Some(GeneralFieldMode::Numeric)
    } else if byte.is_ascii_uppercase() || b"*,-./".contains(&byte) {
        Some(GeneralFieldMode::Alphanumeric)
    } else if byte.is_ascii_lowercase() || b"!\"%&'$()+:;<=>?_ ".contains(&byte) {
        Some(GeneralFieldMode::IsoIec)
    } else {
        None
    }
}

fn general_field_numeric_value(byte: u8) -> Option<u32> {
    if byte == 0x1d {
        Some(10)
    } else {
        byte.is_ascii_digit().then(|| u32::from(byte - b'0'))
    }
}

fn general_field_next(
    data: &[u8],
    position: usize,
    count: usize,
    first_type: GeneralFieldMode,
    second_type: Option<GeneralFieldMode>,
) -> bool {
    let Some(characters) = data.get(position..position + count) else {
        return false;
    };
    characters.iter().all(|byte| {
        let Some(character_type) = general_field_type(*byte) else {
            return false;
        };
        character_type == first_type || second_type == Some(character_type)
    })
}

fn general_field_next_terminates(
    data: &[u8],
    position: usize,
    minimum: usize,
    maximum: usize,
    character_type: GeneralFieldMode,
) -> bool {
    let remaining = &data[position..];
    remaining.len() >= minimum
        && remaining.len() <= maximum
        && remaining
            .iter()
            .all(|byte| general_field_type(*byte) == Some(character_type))
}

fn general_field_next_none(
    data: &[u8],
    position: usize,
    count: usize,
    character_type: GeneralFieldMode,
) -> bool {
    data[position..]
        .iter()
        .take(count)
        .all(|byte| general_field_type(*byte) != Some(character_type))
}

fn pad_expanded_binary(
    bits: &mut Vec<bool>,
    method: u8,
    mode: GeneralFieldMode,
    last_digit: Option<u8>,
) -> Option<()> {
    let mut symbol_characters = expanded_symbol_character_count(bits.len());
    let mut remainder = 12 * (symbol_characters - 1) - bits.len();

    if let Some(last_digit) = last_digit {
        let digit = general_field_numeric_value(last_digit)?;
        if (4..=6).contains(&remainder) {
            append_bits(bits, digit + 1, 4);
        } else {
            append_bits(bits, digit * 11 + 18, 7);
        }
        symbol_characters = expanded_symbol_character_count(bits.len());
        remainder = 12 * (symbol_characters - 1) - bits.len();
    }

    // A linear Expanded symbol has at most 21 data characters. We test the
    // bits rather than the input length because compaction efficiency varies.
    if bits.len() > 252 {
        return None;
    }

    let mut padding = i32::try_from(remainder).ok()?;
    if mode == GeneralFieldMode::Numeric {
        append_bits(bits, 0, 4);
        padding -= 4;
    }
    while padding > 0 {
        append_bits(bits, 4, 5);
        padding -= 5;
    }

    let odd_symbol_length = !symbol_characters.is_multiple_of(2);
    let long_symbol = symbol_characters > 14;
    match method {
        1 => {
            bits[2] = odd_symbol_length;
            bits[3] = long_symbol;
        }
        2 => {
            bits[3] = odd_symbol_length;
            bits[4] = long_symbol;
        }
        5 | 6 => {
            bits[6] = odd_symbol_length;
            bits[7] = long_symbol;
        }
        _ => {}
    }

    // Padding is a repeating five-bit pattern, so its final repetition can
    // cross the last 12-bit character boundary. Only complete data
    // characters belong to the symbol.
    bits.truncate(12 * (symbol_characters - 1));
    Some(())
}

fn expanded_symbol_character_count(bit_length: usize) -> usize {
    let remainder = (12 - bit_length % 12) % 12;
    ((bit_length + remainder) / 12 + 1).max(4)
}

fn expanded_date_value(date: &[u8]) -> Option<u32> {
    let year = parse_decimal(date.get(0..2)?)?;
    let month = parse_decimal(date.get(2..4)?)?;
    let day = parse_decimal(date.get(4..6)?)?;
    (matches!(month, 1..=12) && day <= 31).then(|| year * 384 + (month - 1) * 32 + day)
}

fn parse_decimal(digits: &[u8]) -> Option<u32> {
    digits.iter().try_fold(0_u32, |value, digit| {
        digit
            .is_ascii_digit()
            .then(|| value * 10 + u32::from(digit - b'0'))
    })
}

fn append_bits(bits: &mut Vec<bool>, value: u32, count: u32) {
    bits.extend((0..count).rev().map(|shift| value & (1 << shift) != 0));
}

fn bits_to_value(bits: &[bool]) -> i32 {
    bits.iter()
        .fold(0_i32, |value, bit| value * 2 + i32::from(*bit))
}

fn expanded_character_widths(value: i32) -> [i32; 8] {
    let group = EXPANDED_GROUP_SUMS
        .iter()
        .rposition(|group_sum| value >= *group_sum)
        .unwrap_or(0);
    let value = value - EXPANDED_GROUP_SUMS[group];
    interleaved_widths(
        i64::from(value / EXPANDED_EVEN_DIVISORS[group]),
        i64::from(value % EXPANDED_EVEN_DIVISORS[group]),
        EXPANDED_ODD_MODULES[group],
        17 - EXPANDED_ODD_MODULES[group],
        EXPANDED_WIDEST_ODD_ELEMENTS[group],
        true,
    )
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
