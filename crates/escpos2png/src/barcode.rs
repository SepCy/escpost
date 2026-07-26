//! Logical one-dimensional barcode encoders.
//!
//! This module deliberately knows nothing about printer dots, justification,
//! or paper movement. It produces logical bars and HRI characters so the
//! ESC/POS layer remains authoritative for printer behavior.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarcodeError {
    Length,
    Character,
    Format,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncodedBarcode {
    pub(crate) bars: Vec<BarElement>,
    pub(crate) hri: Vec<char>,
    pub(crate) minimum_height_modules: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BarElement {
    pub(crate) dark: bool,
    pub(crate) width: BarWidth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarWidth {
    Modules(u8),
    Narrow,
    Wide,
}

const CODE93_CHARACTERS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";
const CODE93_PATTERNS: [u16; 48] = [
    0b100010100,
    0b101001000,
    0b101000100,
    0b101000010,
    0b100101000,
    0b100100100,
    0b100100010,
    0b101010000,
    0b100010010,
    0b100001010,
    0b110101000,
    0b110100100,
    0b110100010,
    0b110010100,
    0b110010010,
    0b110001010,
    0b101101000,
    0b101100100,
    0b101100010,
    0b100110100,
    0b100011010,
    0b101011000,
    0b101001100,
    0b101000110,
    0b100101100,
    0b100010110,
    0b110110100,
    0b110110010,
    0b110101100,
    0b110100110,
    0b110010110,
    0b110011010,
    0b101101100,
    0b101100110,
    0b100110110,
    0b100111010,
    0b100101110,
    0b111010100,
    0b111010010,
    0b111001010,
    0b101101110,
    0b101110110,
    0b110101110,
    0b100100110,
    0b111011010,
    0b111010110,
    0b100110010,
    0b101011110,
];
const CODE128_PATTERNS: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212",
    "221213", "221312", "231212", "112232", "122132", "122231", "113222", "123122", "123221",
    "223211", "221132", "221231", "213212", "223112", "312131", "311222", "321122", "321221",
    "312212", "322112", "322211", "212123", "212321", "232121", "111323", "131123", "131321",
    "112313", "132113", "132311", "211313", "231113", "231311", "112133", "112331", "132131",
    "113123", "113321", "133121", "313121", "211331", "231131", "213113", "213311", "213131",
    "311123", "311321", "331121", "312113", "312311", "332111", "314111", "221411", "431111",
    "111224", "111422", "121124", "121421", "141122", "141221", "112214", "112412", "122114",
    "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111", "111242",
    "121142", "121241", "114212", "124112", "124211", "411212", "421112", "421211", "212141",
    "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311",
    "113141", "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];

pub(crate) fn encode_ean13(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    const LEFT_ODD: [&str; 10] = [
        "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
        "0110111", "0001011",
    ];
    const LEFT_EVEN: [&str; 10] = [
        "0100111", "0110011", "0011011", "0100001", "0011101", "0111001", "0000101", "0010001",
        "0001001", "0010111",
    ];
    const RIGHT: [&str; 10] = [
        "1110010", "1100110", "1101100", "1000010", "1011100", "1001110", "1010000", "1000100",
        "1001000", "1110100",
    ];
    const PARITY: [&str; 10] = [
        "LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL",
        "LGGLGL",
    ];

    if !matches!(data.len(), 12 | 13) {
        return Err(BarcodeError::Length);
    }
    if !data.iter().all(u8::is_ascii_digit) {
        return Err(BarcodeError::Character);
    }

    let mut digits = data.iter().map(|digit| digit - b'0').collect::<Vec<_>>();
    if digits.len() == 12 {
        digits.push(ean_check_digit(&digits));
    }

    let mut modules = Vec::with_capacity(95);
    push_pattern(&mut modules, "101");
    for (digit, parity) in digits[1..7]
        .iter()
        .zip(PARITY[usize::from(digits[0])].bytes())
    {
        push_pattern(
            &mut modules,
            if parity == b'L' {
                LEFT_ODD[usize::from(*digit)]
            } else {
                LEFT_EVEN[usize::from(*digit)]
            },
        );
    }
    push_pattern(&mut modules, "01010");
    for digit in &digits[7..] {
        push_pattern(&mut modules, RIGHT[usize::from(*digit)]);
    }
    push_pattern(&mut modules, "101");
    let hri = digits
        .iter()
        .map(|digit| char::from(digit + b'0'))
        .collect();
    Ok(EncodedBarcode::from_modules(&modules, hri))
}

pub(crate) fn encode_upca(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    if !matches!(data.len(), 11 | 12) {
        return Err(BarcodeError::Length);
    }
    if !data.iter().all(u8::is_ascii_digit) {
        return Err(BarcodeError::Character);
    }

    // UPC-A uses the same 95-module pattern as an EAN-13 value whose leading
    // number-system digit is zero.
    let mut ean_data = Vec::with_capacity(data.len() + 1);
    ean_data.push(b'0');
    ean_data.extend_from_slice(data);
    let mut encoded = encode_ean13(&ean_data)?;
    encoded.hri.remove(0);
    Ok(encoded)
}

pub(crate) fn encode_ean8(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    const LEFT: [&str; 10] = [
        "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
        "0110111", "0001011",
    ];
    const RIGHT: [&str; 10] = [
        "1110010", "1100110", "1101100", "1000010", "1011100", "1001110", "1010000", "1000100",
        "1001000", "1110100",
    ];

    if !matches!(data.len(), 7 | 8) {
        return Err(BarcodeError::Length);
    }
    if !data.iter().all(u8::is_ascii_digit) {
        return Err(BarcodeError::Character);
    }

    let mut digits = data.iter().map(|digit| digit - b'0').collect::<Vec<_>>();
    if digits.len() == 7 {
        digits.push(ean_check_digit(&digits));
    }

    let mut modules = Vec::with_capacity(67);
    push_pattern(&mut modules, "101");
    for digit in &digits[..4] {
        push_pattern(&mut modules, LEFT[usize::from(*digit)]);
    }
    push_pattern(&mut modules, "01010");
    for digit in &digits[4..] {
        push_pattern(&mut modules, RIGHT[usize::from(*digit)]);
    }
    push_pattern(&mut modules, "101");
    let hri = digits
        .iter()
        .map(|digit| char::from(digit + b'0'))
        .collect();
    Ok(EncodedBarcode::from_modules(&modules, hri))
}

pub(crate) fn encode_upce(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    const LEFT_ODD: [&str; 10] = [
        "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
        "0110111", "0001011",
    ];
    const LEFT_EVEN: [&str; 10] = [
        "0100111", "0110011", "0011011", "0100001", "0011101", "0111001", "0000101", "0010001",
        "0001001", "0010111",
    ];
    const NUMBER_SYSTEM_ZERO_PARITY: [&str; 10] = [
        "GGGLLL", "GGLGLL", "GGLLGL", "GGLLLG", "GLGGLL", "GLLGGL", "GLLLGG", "GLGLGL", "GLGLLG",
        "GLLGLG",
    ];

    if !matches!(data.len(), 6..=8 | 11..=12) {
        return Err(BarcodeError::Length);
    }
    if !data.iter().all(u8::is_ascii_digit) {
        return Err(BarcodeError::Character);
    }

    let (number_system, compact, supplied_check) = match data.len() {
        6 => (0, data.to_vec(), None),
        7 => (data[0] - b'0', data[1..].to_vec(), None),
        8 => (data[0] - b'0', data[1..7].to_vec(), Some(data[7] - b'0')),
        11 => (
            data[0] - b'0',
            compress_upca_to_upce(&data[1..11])?.to_vec(),
            None,
        ),
        12 => (
            data[0] - b'0',
            compress_upca_to_upce(&data[1..11])?.to_vec(),
            Some(data[11] - b'0'),
        ),
        _ => unreachable!("the accepted UPC-E lengths were checked above"),
    };
    // Epson's documented seven- and eight-byte forms require number system 0.
    if number_system != 0 {
        return Err(BarcodeError::Format);
    }
    let check = supplied_check.unwrap_or_else(|| {
        let upca_payload = expand_upce_payload(number_system, &compact);
        ean_check_digit(&upca_payload)
    });
    let parity = NUMBER_SYSTEM_ZERO_PARITY[usize::from(check)];

    let mut modules = Vec::with_capacity(51);
    push_pattern(&mut modules, "101");
    for (digit, parity) in compact.iter().zip(parity.bytes()) {
        push_pattern(
            &mut modules,
            if parity == b'L' {
                LEFT_ODD[usize::from(*digit - b'0')]
            } else {
                LEFT_EVEN[usize::from(*digit - b'0')]
            },
        );
    }
    push_pattern(&mut modules, "010101");
    let mut hri = Vec::with_capacity(8);
    hri.push(char::from(number_system + b'0'));
    hri.extend(compact.iter().copied().map(char::from));
    hri.push(char::from(check + b'0'));
    Ok(EncodedBarcode::from_modules(&modules, hri))
}

pub(crate) fn encode_code39(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    if data.is_empty() {
        return Err(BarcodeError::Length);
    }
    if !data
        .iter()
        .all(|character| code39_pattern(*character).is_some())
    {
        return Err(BarcodeError::Character);
    }
    if data.len() > 2 && data[1..data.len() - 1].contains(&b'*') {
        return Err(BarcodeError::Format);
    }

    let mut characters = Vec::with_capacity(data.len() + 2);
    if data.first() != Some(&b'*') {
        characters.push(b'*');
    }
    characters.extend_from_slice(data);
    if data.last() != Some(&b'*') {
        characters.push(b'*');
    }

    let mut bars = Vec::new();
    for (index, character) in characters.iter().copied().enumerate() {
        let pattern = code39_pattern(character).expect("Code 39 data was validated above");
        push_binary_pattern(&mut bars, pattern);
        if index + 1 < characters.len() {
            bars.push(BarElement {
                dark: false,
                width: BarWidth::Narrow,
            });
        }
    }
    Ok(EncodedBarcode {
        bars,
        hri: ascii_hri(data),
        minimum_height_modules: 0,
    })
}

pub(crate) fn encode_itf(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    if data.len() < 2 || !data.len().is_multiple_of(2) {
        return Err(BarcodeError::Length);
    }
    if !data.iter().all(u8::is_ascii_digit) {
        return Err(BarcodeError::Character);
    }

    let mut bars = Vec::new();
    push_binary_pattern(&mut bars, "nnnn");
    for pair in data.chunks_exact(2) {
        let bar_pattern = itf_pattern(pair[0]);
        let space_pattern = itf_pattern(pair[1]);
        for (bar, space) in bar_pattern.bytes().zip(space_pattern.bytes()) {
            bars.push(BarElement {
                dark: true,
                width: binary_width(bar),
            });
            bars.push(BarElement {
                dark: false,
                width: binary_width(space),
            });
        }
    }
    push_binary_pattern(&mut bars, "wnn");
    Ok(EncodedBarcode {
        bars,
        hri: ascii_hri(data),
        minimum_height_modules: 0,
    })
}

pub(crate) fn encode_codabar(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    if data.len() < 2 {
        return Err(BarcodeError::Length);
    }
    if !is_codabar_guard(data[0]) || !is_codabar_guard(data[data.len() - 1]) {
        return Err(BarcodeError::Format);
    }
    if !data[1..data.len() - 1]
        .iter()
        .all(|character| codabar_data_pattern(*character).is_some())
    {
        return Err(BarcodeError::Character);
    }

    let mut bars = Vec::new();
    for (index, character) in data.iter().copied().enumerate() {
        let pattern = if is_codabar_guard(character) {
            codabar_guard_pattern(character)
        } else {
            codabar_data_pattern(character).expect("Codabar data was validated above")
        };
        push_binary_pattern(&mut bars, pattern);
        if index + 1 < data.len() {
            bars.push(BarElement {
                dark: false,
                width: BarWidth::Narrow,
            });
        }
    }
    Ok(EncodedBarcode {
        bars,
        hri: ascii_hri(data),
        minimum_height_modules: 0,
    })
}

pub(crate) fn encode_code93(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    if data.is_empty() {
        return Err(BarcodeError::Length);
    }
    let mut values = Vec::with_capacity(data.len());
    for character in data.iter().copied() {
        push_code93_ascii(&mut values, character)?;
    }
    let hri = code93_hri(&values);

    values.push(code93_check_value(&values, 20));
    values.push(code93_check_value(&values, 15));

    let mut modules = Vec::with_capacity((values.len() + 2) * 9 + 1);
    push_code93_pattern(&mut modules, 47);
    for value in values {
        push_code93_pattern(&mut modules, value);
    }
    push_code93_pattern(&mut modules, 47);
    modules.push(true);
    Ok(EncodedBarcode::from_modules(&modules, hri))
}

pub(crate) fn encode_code128(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    if data.len() < 2 || data[0] != b'{' {
        return Err(BarcodeError::Length);
    }
    let mut code_set = match data[1] {
        b'A' => Code128Set::A,
        b'B' => Code128Set::B,
        b'C' => Code128Set::C,
        _ => return Err(BarcodeError::Format),
    };

    let mut symbols = vec![code_set.start_value()];
    let mut hri = Vec::new();
    let mut index = 2;
    while index < data.len() {
        if data[index] == b'{' {
            let control = *data.get(index + 1).ok_or(BarcodeError::Format)?;
            match control {
                b'A' | b'B' | b'C' => {
                    code_set = match control {
                        b'A' => Code128Set::A,
                        b'B' => Code128Set::B,
                        b'C' => Code128Set::C,
                        _ => unreachable!(),
                    };
                    symbols.push(code_set.switch_value());
                }
                b'1' => {
                    symbols.push(102);
                    hri.push(' ');
                }
                b'2' => {
                    symbols.push(97);
                    hri.push(' ');
                }
                b'3' => {
                    symbols.push(96);
                    hri.push(' ');
                }
                b'4' => {
                    symbols.push(match code_set {
                        Code128Set::A => 101,
                        Code128Set::B => 100,
                        Code128Set::C => return Err(BarcodeError::Format),
                    });
                    hri.push(' ');
                }
                b'{' => {
                    symbols.push(code128_character_value(code_set, b'{')?);
                    hri.push('{');
                }
                b'S' => {
                    let shifted_set = match code_set {
                        Code128Set::A => Code128Set::B,
                        Code128Set::B => Code128Set::A,
                        Code128Set::C => return Err(BarcodeError::Format),
                    };
                    let character = *data.get(index + 2).ok_or(BarcodeError::Format)?;
                    symbols.push(98);
                    symbols.push(code128_character_value(shifted_set, character)?);
                    hri.push(if character.is_ascii_control() {
                        ' '
                    } else {
                        char::from(character)
                    });
                    // SHIFT consumes its marker and exactly one data byte.
                    // The following byte uses the original code set again.
                    index += 3;
                    continue;
                }
                _ => return Err(BarcodeError::Format),
            }
            index += 2;
            continue;
        }

        match code_set {
            Code128Set::A | Code128Set::B => {
                let character = data[index];
                symbols.push(code128_character_value(code_set, character)?);
                hri.push(if character.is_ascii_control() {
                    ' '
                } else {
                    char::from(character)
                });
                index += 1;
            }
            Code128Set::C => {
                let pair = data.get(index..index + 2).ok_or(BarcodeError::Format)?;
                if !pair.iter().all(u8::is_ascii_digit) {
                    return Err(BarcodeError::Format);
                }
                symbols.push((pair[0] - b'0') * 10 + pair[1] - b'0');
                hri.extend(pair.iter().copied().map(char::from));
                index += 2;
            }
        }
    }
    if symbols.len() == 1 {
        return Err(BarcodeError::Length);
    }

    let checksum = symbols
        .iter()
        .enumerate()
        .map(|(index, value)| usize::from(*value) * if index == 0 { 1 } else { index })
        .sum::<usize>()
        % 103;
    symbols.push(checksum as u8);
    symbols.push(106);

    let mut modules = Vec::new();
    for symbol in symbols {
        push_code128_pattern(&mut modules, symbol);
    }
    Ok(EncodedBarcode::from_modules(&modules, hri))
}

pub(crate) fn encode_code128_auto(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    if data.is_empty() {
        return Err(BarcodeError::Length);
    }

    let explicit = plan_code128_auto(data);
    let mut encoded = encode_code128(&explicit)?;
    encoded.hri = code128_auto_hri(data);
    Ok(encoded)
}

pub(crate) fn encode_gs1_128(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    if data.len() < 2 {
        return Err(BarcodeError::Length);
    }
    if data.iter().any(|byte| *byte > 0x7f) {
        return Err(BarcodeError::Character);
    }

    // GS1 controls are logical Code 128 symbols rather than bytes. Keeping
    // them as tokens lets the automatic code-set planner compact digits across
    // FNC1 without ever confusing the control with user data.
    let mut tokens = vec![Code128Token::Fnc1];
    let mut hri = Vec::with_capacity(data.len());
    let mut check_digit_data_start = None;
    let mut separator_seen = false;
    let mut index = 0;
    while index < data.len() {
        let byte = data[index];
        if byte == b'{' {
            let escaped = *data.get(index + 1).ok_or(BarcodeError::Format)?;
            match escaped {
                b'1' => {
                    tokens.push(Code128Token::Fnc1);
                    hri.push(' ');
                    separator_seen = false;
                    check_digit_data_start = None;
                }
                b'3' => {
                    tokens.push(Code128Token::Fnc3);
                    hri.push(' ');
                }
                b'(' | b')' | b'*' | b'{' => {
                    tokens.push(Code128Token::Data(escaped));
                    hri.push(char::from(escaped));
                }
                _ => return Err(BarcodeError::Format),
            }
            index += 2;
            continue;
        }

        match byte {
            b'(' => hri.push('('),
            b')' => {
                hri.push(')');
                if !separator_seen {
                    check_digit_data_start = Some(tokens.len());
                    separator_seen = true;
                }
            }
            b' ' => {
                hri.push(' ');
                if !separator_seen {
                    check_digit_data_start = Some(tokens.len());
                    separator_seen = true;
                }
            }
            b'*' => {
                let start = check_digit_data_start.ok_or(BarcodeError::Format)?;
                let check_digit = gs1_modulus_10_tokens(&tokens[start..])?;
                tokens.push(Code128Token::Data(check_digit + b'0'));
                hri.push(char::from(check_digit + b'0'));
            }
            byte => {
                tokens.push(Code128Token::Data(byte));
                hri.push(if byte.is_ascii_control() {
                    ' '
                } else {
                    char::from(byte)
                });
            }
        }
        index += 1;
    }
    if tokens.len() == 1 {
        return Err(BarcodeError::Length);
    }

    // Epson chooses the Code 128 sets automatically for GS1-128.
    let explicit = plan_code128_tokens(&tokens);
    let mut encoded = encode_code128(&explicit)?;
    encoded.hri = hri;
    Ok(encoded)
}

pub(crate) fn encode_gs1_databar_omnidirectional(
    data: &[u8],
) -> Result<EncodedBarcode, BarcodeError> {
    encode_gs1_databar_gtin(data, 33, crate::databar::encode_omnidirectional)
}

pub(crate) fn encode_gs1_databar_truncated(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    // Truncated uses the exact Omnidirectional module pattern. Only Epson's
    // minimum printed height changes from 33X to 13X.
    encode_gs1_databar_gtin(data, 13, crate::databar::encode_omnidirectional)
}

pub(crate) fn encode_gs1_databar_limited(data: &[u8]) -> Result<EncodedBarcode, BarcodeError> {
    encode_gs1_databar_gtin(data, 10, crate::databar::encode_limited)
}

fn encode_gs1_databar_gtin(
    data: &[u8],
    minimum_height_modules: u8,
    encode_modules: fn(&[u8]) -> Option<Vec<bool>>,
) -> Result<EncodedBarcode, BarcodeError> {
    if data.len() != 13 {
        return Err(BarcodeError::Length);
    }
    if !data.iter().all(u8::is_ascii_digit) {
        return Err(BarcodeError::Character);
    }

    let modules = encode_modules(data).ok_or(BarcodeError::Format)?;
    let digit_values = data.iter().map(|digit| digit - b'0').collect::<Vec<_>>();
    let check_digit = ean_check_digit(&digit_values) + b'0';
    let hri = "(01)"
        .chars()
        .chain(data.iter().copied().map(char::from))
        .chain([char::from(check_digit)])
        .collect();
    Ok(EncodedBarcode::from_modules(&modules, hri)
        .with_minimum_height_modules(minimum_height_modules))
}

fn gs1_modulus_10_tokens(tokens: &[Code128Token]) -> Result<u8, BarcodeError> {
    let digits = tokens
        .iter()
        .map(|token| match token {
            Code128Token::Data(byte) if byte.is_ascii_digit() => Ok(*byte),
            _ => Err(BarcodeError::Format),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if digits.is_empty() {
        return Err(BarcodeError::Format);
    }
    let sum = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(index, digit)| u32::from(digit - b'0') * if index.is_multiple_of(2) { 3 } else { 1 })
        .sum::<u32>();
    Ok(((10 - sum % 10) % 10) as u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Code128Set {
    A,
    B,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Code128Token {
    Data(u8),
    Fnc1,
    Fnc3,
}

#[derive(Debug, Clone, Copy)]
struct Code128AutoChoice {
    target: Code128Set,
    advance: usize,
    shift: bool,
    fnc4_count: u8,
    upper_after: bool,
}

impl Code128Set {
    fn start_value(self) -> u8 {
        match self {
            Self::A => 103,
            Self::B => 104,
            Self::C => 105,
        }
    }

    fn switch_value(self) -> u8 {
        match self {
            Self::A => 101,
            Self::B => 100,
            Self::C => 99,
        }
    }

    fn marker(self) -> u8 {
        match self {
            Self::A => b'A',
            Self::B => b'B',
            Self::C => b'C',
        }
    }
}

fn plan_code128_auto(data: &[u8]) -> Vec<u8> {
    let tokens = data
        .iter()
        .copied()
        .map(Code128Token::Data)
        .collect::<Vec<_>>();
    plan_code128_tokens(&tokens)
}

fn plan_code128_tokens(data: &[Code128Token]) -> Vec<u8> {
    // A state combines the current code set (including "not started") with
    // FNC4's lower/upper latch. Every Code 128 symbol has the same width apart
    // from the fixed stop symbol, so minimizing symbol count also minimizes
    // the complete barcode width.
    const STATE_COUNT: usize = 8;
    const UNREACHABLE: usize = usize::MAX / 4;

    // Work backwards so each candidate can reuse the cheapest known suffix.
    // The protocol limits input to 255 bytes, keeping this table small.
    let mut costs = vec![[UNREACHABLE; STATE_COUNT]; data.len() + 1];
    let mut choices = vec![[None; STATE_COUNT]; data.len()];
    costs[data.len()] = [0; STATE_COUNT];

    for position in (0..data.len()).rev() {
        for state_index in 0..STATE_COUNT {
            let (current, upper_mode) = code128_state(state_index);
            let mut best_cost = UNREACHABLE;
            let mut best_choice = None;

            if matches!(data[position], Code128Token::Fnc1 | Code128Token::Fnc3) {
                for target in [Code128Set::B, Code128Set::A, Code128Set::C] {
                    if data[position] == Code128Token::Fnc3 && target == Code128Set::C {
                        continue;
                    }
                    consider_code128_auto_choice(
                        Code128AutoChoice {
                            target,
                            advance: 1,
                            shift: false,
                            fnc4_count: 0,
                            upper_after: upper_mode,
                        },
                        1 + usize::from(current != Some(target))
                            + costs[position + 1][code128_state_index(Some(target), upper_mode)],
                        &mut best_cost,
                        &mut best_choice,
                    );
                }
                costs[position][state_index] = best_cost;
                choices[position][state_index] = best_choice;
                continue;
            }

            let Code128Token::Data(byte) = data[position] else {
                unreachable!("control tokens returned above")
            };
            let text_character = byte & 0x7f;
            let byte_uses_upper_half = !byte.is_ascii();
            let needs_upper_shift = byte_uses_upper_half != upper_mode;

            // Prefer B when A and B are equally narrow. Ordinary text then
            // follows the same stable choice as Epson's Code128 auto examples.
            for target in [Code128Set::B, Code128Set::A] {
                if !code128_set_encodes(target, text_character) {
                    continue;
                }
                consider_code128_auto_choice(
                    Code128AutoChoice {
                        target,
                        advance: 1,
                        shift: false,
                        fnc4_count: u8::from(needs_upper_shift),
                        upper_after: upper_mode,
                    },
                    1 + usize::from(needs_upper_shift)
                        + usize::from(current != Some(target))
                        + costs[position + 1][code128_state_index(Some(target), upper_mode)],
                    &mut best_cost,
                    &mut best_choice,
                );

                if !needs_upper_shift
                    && matches!(current, Some(Code128Set::A | Code128Set::B))
                    && current != Some(target)
                {
                    consider_code128_auto_choice(
                        Code128AutoChoice {
                            target,
                            advance: 1,
                            shift: true,
                            fnc4_count: 0,
                            upper_after: upper_mode,
                        },
                        2 + costs[position + 1][state_index],
                        &mut best_cost,
                        &mut best_choice,
                    );
                }

                if needs_upper_shift
                    && matches!(current, Some(Code128Set::A | Code128Set::B))
                    && current != Some(target)
                {
                    // FNC4 may be followed by SHIFT when one upper/lower-mode
                    // character belongs to the other text set. This keeps the
                    // surrounding A/B set latched.
                    consider_code128_auto_choice(
                        Code128AutoChoice {
                            target,
                            advance: 1,
                            shift: true,
                            fnc4_count: 1,
                            upper_after: upper_mode,
                        },
                        3 + costs[position + 1][state_index],
                        &mut best_cost,
                        &mut best_choice,
                    );
                }

                if needs_upper_shift {
                    let upper_after = !upper_mode;
                    consider_code128_auto_choice(
                        Code128AutoChoice {
                            target,
                            advance: 1,
                            shift: false,
                            fnc4_count: 2,
                            upper_after,
                        },
                        3 + usize::from(current != Some(target))
                            + costs[position + 1][code128_state_index(Some(target), upper_after)],
                        &mut best_cost,
                        &mut best_choice,
                    );
                }
            }

            let decimal_pair = data
                .get(position..position + 2)
                .and_then(|pair| match pair {
                    [Code128Token::Data(first), Code128Token::Data(second)]
                        if first.is_ascii_digit() && second.is_ascii_digit() =>
                    {
                        Some(())
                    }
                    _ => None,
                })
                .is_some();
            if decimal_pair {
                let target = Code128Set::C;
                consider_code128_auto_choice(
                    Code128AutoChoice {
                        target,
                        advance: 2,
                        shift: false,
                        fnc4_count: 0,
                        upper_after: upper_mode,
                    },
                    1 + usize::from(current != Some(target))
                        + costs[position + 2][code128_state_index(Some(target), upper_mode)],
                    &mut best_cost,
                    &mut best_choice,
                );
            }

            costs[position][state_index] = best_cost;
            choices[position][state_index] = best_choice;
        }
    }

    let mut explicit = Vec::with_capacity(data.len() + 2);
    let mut position = 0;
    let mut current = None;
    let mut upper_mode = false;
    while position < data.len() {
        let choice = choices[position][code128_state_index(current, upper_mode)]
            .expect("every byte can be represented by Code 128");
        if choice.shift {
            for _ in 0..choice.fnc4_count {
                explicit.extend_from_slice(b"{4");
            }
            explicit.extend_from_slice(b"{S");
        } else {
            if current != Some(choice.target) {
                explicit.extend_from_slice(&[b'{', choice.target.marker()]);
                current = Some(choice.target);
            }
            for _ in 0..choice.fnc4_count {
                explicit.extend_from_slice(b"{4");
            }
        }

        match data[position] {
            Code128Token::Fnc1 => explicit.extend_from_slice(b"{1"),
            Code128Token::Fnc3 => explicit.extend_from_slice(b"{3"),
            Code128Token::Data(_) => {
                for token in &data[position..position + choice.advance] {
                    let Code128Token::Data(character) = *token else {
                        unreachable!("a data choice cannot span a control token")
                    };
                    let character = if choice.target == Code128Set::C {
                        character
                    } else {
                        character & 0x7f
                    };
                    if character == b'{' {
                        explicit.extend_from_slice(b"{{");
                    } else {
                        explicit.push(character);
                    }
                }
            }
        }
        upper_mode = choice.upper_after;
        position += choice.advance;
    }
    explicit
}

fn consider_code128_auto_choice(
    choice: Code128AutoChoice,
    cost: usize,
    best_cost: &mut usize,
    best_choice: &mut Option<Code128AutoChoice>,
) {
    if cost < *best_cost {
        *best_cost = cost;
        *best_choice = Some(choice);
    }
}

fn code128_state_index(code_set: Option<Code128Set>, upper_mode: bool) -> usize {
    let code_set_index = match code_set {
        None => 0,
        Some(Code128Set::A) => 1,
        Some(Code128Set::B) => 2,
        Some(Code128Set::C) => 3,
    };
    code_set_index * 2 + usize::from(upper_mode)
}

fn code128_state(index: usize) -> (Option<Code128Set>, bool) {
    let code_set = match index / 2 {
        0 => None,
        1 => Some(Code128Set::A),
        2 => Some(Code128Set::B),
        3 => Some(Code128Set::C),
        _ => unreachable!("Code 128 auto state index is bounded"),
    };
    (code_set, index % 2 == 1)
}

fn code128_set_encodes(code_set: Code128Set, character: u8) -> bool {
    match code_set {
        Code128Set::A => character <= 0x5f,
        Code128Set::B => (0x20..=0x7f).contains(&character),
        Code128Set::C => false,
    }
}

fn code128_auto_hri(data: &[u8]) -> Vec<char> {
    data.iter()
        .copied()
        .map(|character| match character {
            // Code 128 represents the C0/C1 ranges, but printers show control
            // bytes as spaces in HRI rather than attempting visible glyphs.
            0x00..=0x1f | 0x7f..=0x9f => ' ',
            _ => char::from(character),
        })
        .collect()
}

impl EncodedBarcode {
    fn from_modules(modules: &[bool], hri: Vec<char>) -> Self {
        let mut bars = Vec::new();
        for dark in modules.iter().copied() {
            match bars.last_mut() {
                Some(BarElement {
                    dark: previous_dark,
                    width: BarWidth::Modules(width),
                }) if *previous_dark == dark && *width < u8::MAX => {
                    *width += 1;
                }
                _ => bars.push(BarElement {
                    dark,
                    width: BarWidth::Modules(1),
                }),
            }
        }
        Self {
            bars,
            hri,
            minimum_height_modules: 0,
        }
    }

    fn with_minimum_height_modules(mut self, minimum_height_modules: u8) -> Self {
        self.minimum_height_modules = minimum_height_modules;
        self
    }
}

fn ascii_hri(data: &[u8]) -> Vec<char> {
    data.iter().copied().map(char::from).collect()
}

fn code93_hri(values: &[usize]) -> Vec<char> {
    let mut hri = Vec::with_capacity(values.len() + 2);
    hri.push('□');
    for value in values.iter().copied() {
        hri.push(match value {
            0..=42 => char::from(CODE93_CHARACTERS[value]),
            43..=46 => '■',
            _ => unreachable!("Code 93 data values were validated"),
        });
    }
    hri.push('□');
    hri
}

fn code39_pattern(character: u8) -> Option<&'static str> {
    Some(match character {
        b'0' => "nnnwwnwnn",
        b'1' => "wnnwnnnnw",
        b'2' => "nnwwnnnnw",
        b'3' => "wnwwnnnnn",
        b'4' => "nnnwwnnnw",
        b'5' => "wnnwwnnnn",
        b'6' => "nnwwwnnnn",
        b'7' => "nnnwnnwnw",
        b'8' => "wnnwnnwnn",
        b'9' => "nnwwnnwnn",
        b'A' => "wnnnnwnnw",
        b'B' => "nnwnnwnnw",
        b'C' => "wnwnnwnnn",
        b'D' => "nnnnwwnnw",
        b'E' => "wnnnwwnnn",
        b'F' => "nnwnwwnnn",
        b'G' => "nnnnnwwnw",
        b'H' => "wnnnnwwnn",
        b'I' => "nnwnnwwnn",
        b'J' => "nnnnwwwnn",
        b'K' => "wnnnnnnww",
        b'L' => "nnwnnnnww",
        b'M' => "wnwnnnnwn",
        b'N' => "nnnnwnnww",
        b'O' => "wnnnwnnwn",
        b'P' => "nnwnwnnwn",
        b'Q' => "nnnnnnwww",
        b'R' => "wnnnnnwwn",
        b'S' => "nnwnnnwwn",
        b'T' => "nnnnwnwwn",
        b'U' => "wwnnnnnnw",
        b'V' => "nwwnnnnnw",
        b'W' => "wwwnnnnnn",
        b'X' => "nwnnwnnnw",
        b'Y' => "wwnnwnnnn",
        b'Z' => "nwwnwnnnn",
        b'-' => "nwnnnnwnw",
        b'.' => "wwnnnnwnn",
        b' ' => "nwwnnnwnn",
        b'$' => "nwnwnwnnn",
        b'/' => "nwnwnnnwn",
        b'+' => "nwnnnwnwn",
        b'%' => "nnnwnwnwn",
        b'*' => "nwnnwnwnn",
        _ => return None,
    })
}

fn push_binary_pattern(bars: &mut Vec<BarElement>, pattern: &str) {
    bars.extend(
        pattern
            .bytes()
            .enumerate()
            .map(|(index, element)| BarElement {
                dark: index % 2 == 0,
                width: if element == b'w' {
                    BarWidth::Wide
                } else {
                    BarWidth::Narrow
                },
            }),
    );
}

fn itf_pattern(digit: u8) -> &'static str {
    match digit {
        b'0' => "nnwwn",
        b'1' => "wnnnw",
        b'2' => "nwnnw",
        b'3' => "wwnnn",
        b'4' => "nnwnw",
        b'5' => "wnwnn",
        b'6' => "nwwnn",
        b'7' => "nnnww",
        b'8' => "wnnwn",
        b'9' => "nwnwn",
        _ => unreachable!("ITF data was validated as decimal"),
    }
}

fn binary_width(element: u8) -> BarWidth {
    if element == b'w' {
        BarWidth::Wide
    } else {
        BarWidth::Narrow
    }
}

fn is_codabar_guard(character: u8) -> bool {
    matches!(character, b'A'..=b'D' | b'a'..=b'd')
}

fn codabar_guard_pattern(character: u8) -> &'static str {
    match character.to_ascii_uppercase() {
        b'A' => "nnwwnwn",
        b'B' => "nnnwnww",
        b'C' => "nwnwnnw",
        b'D' => "nnnwwwn",
        _ => unreachable!("Codabar guard was validated above"),
    }
}

fn codabar_data_pattern(character: u8) -> Option<&'static str> {
    Some(match character {
        b'0' => "nnnnnww",
        b'1' => "nnnnwwn",
        b'2' => "nnnwnnw",
        b'3' => "wwnnnnn",
        b'4' => "nnwnnwn",
        b'5' => "wnnnnwn",
        b'6' => "nwnnnnw",
        b'7' => "nwnnwnn",
        b'8' => "nwwnnnn",
        b'9' => "wnnwnnn",
        b'-' => "nnnwwnn",
        b'$' => "nnwwnnn",
        b':' => "wnnnwnw",
        b'/' => "wnwnnnw",
        b'.' => "wnwnwnn",
        b'+' => "nnwnwnw",
        _ => return None,
    })
}

fn code93_check_value(values: &[usize], maximum_weight: usize) -> usize {
    let mut weight = 1;
    let mut sum = 0;
    for value in values.iter().rev() {
        sum += weight * value;
        weight = if weight == maximum_weight {
            1
        } else {
            weight + 1
        };
    }
    sum % 47
}

fn push_code93_ascii(values: &mut Vec<usize>, character: u8) -> Result<(), BarcodeError> {
    if let Some(value) = CODE93_CHARACTERS
        .iter()
        .position(|candidate| *candidate == character)
    {
        values.push(value);
        return Ok(());
    }

    // Code 93 represents the rest of ASCII with one of four shift symbols
    // followed by a normal alphabetic symbol. Values 43 through 46 are the
    // ($), (%), (/), and (+) shift symbols in that order.
    let pair = match character {
        0 => (44, code93_letter_value(b'U')),
        1..=26 => (43, code93_letter_value(b'A' + character - 1)),
        27..=31 => (44, code93_letter_value(b'A' + character - 27)),
        33..=35 => (45, code93_letter_value(b'A' + character - 33)),
        38..=42 => (45, code93_letter_value(b'F' + character - 38)),
        44 => (45, code93_letter_value(b'L')),
        47 => (45, code93_letter_value(b'O')),
        58 => (45, code93_letter_value(b'Z')),
        59..=63 => (44, code93_letter_value(b'F' + character - 59)),
        64 => (44, code93_letter_value(b'V')),
        91..=95 => (44, code93_letter_value(b'K' + character - 91)),
        96 => (44, code93_letter_value(b'W')),
        b'a'..=b'z' => (46, code93_letter_value(b'A' + character - b'a')),
        123..=127 => (44, code93_letter_value(b'P' + character - 123)),
        // Printable characters that have a direct representation were
        // returned above. No byte outside ASCII reaches GS k Function B.
        _ => return Err(BarcodeError::Character),
    };
    values.extend_from_slice(&[pair.0, pair.1]);
    Ok(())
}

fn code93_letter_value(letter: u8) -> usize {
    usize::from(letter - b'A') + 10
}

fn push_code93_pattern(modules: &mut Vec<bool>, value: usize) {
    let pattern = CODE93_PATTERNS[value];
    modules.extend((0..9).rev().map(|bit| pattern & (1 << bit) != 0));
}

fn code128_character_value(code_set: Code128Set, character: u8) -> Result<u8, BarcodeError> {
    match code_set {
        Code128Set::A if character <= 0x1f => Ok(character + 64),
        Code128Set::A if (0x20..=0x5f).contains(&character) => Ok(character - 0x20),
        Code128Set::B if (0x20..=0x7f).contains(&character) => Ok(character - 0x20),
        _ => Err(BarcodeError::Character),
    }
}

fn push_code128_pattern(modules: &mut Vec<bool>, value: u8) {
    for (index, width) in CODE128_PATTERNS[usize::from(value)].bytes().enumerate() {
        modules.extend(std::iter::repeat_n(
            index % 2 == 0,
            usize::from(width - b'0'),
        ));
    }
}

fn expand_upce_payload(number_system: u8, compact: &[u8]) -> [u8; 11] {
    let digits: [u8; 6] = std::array::from_fn(|index| compact[index] - b'0');
    let mut payload = [0; 11];
    payload[0] = number_system;

    match digits[5] {
        suffix @ 0..=2 => {
            payload[1..6].copy_from_slice(&[digits[0], digits[1], suffix, 0, 0]);
            payload[6..].copy_from_slice(&[0, 0, digits[2], digits[3], digits[4]]);
        }
        3 => {
            payload[1..6].copy_from_slice(&[digits[0], digits[1], digits[2], 0, 0]);
            payload[6..].copy_from_slice(&[0, 0, 0, digits[3], digits[4]]);
        }
        4 => {
            payload[1..6].copy_from_slice(&[digits[0], digits[1], digits[2], digits[3], 0]);
            payload[6..].copy_from_slice(&[0, 0, 0, 0, digits[4]]);
        }
        suffix => {
            payload[1..6].copy_from_slice(&[digits[0], digits[1], digits[2], digits[3], digits[4]]);
            payload[6..].copy_from_slice(&[0, 0, 0, 0, suffix]);
        }
    }
    payload
}

fn compress_upca_to_upce(data: &[u8]) -> Result<[u8; 6], BarcodeError> {
    let d = data;
    let compact = if matches!(d[2], b'0'..=b'2') && d[3..=6].iter().all(|digit| *digit == b'0') {
        [d[0], d[1], d[7], d[8], d[9], d[2]]
    } else if matches!(d[2], b'3'..=b'9') && d[3..=7].iter().all(|digit| *digit == b'0') {
        [d[0], d[1], d[2], d[8], d[9], b'3']
    } else if matches!(d[3], b'1'..=b'9') && d[4..=8].iter().all(|digit| *digit == b'0') {
        [d[0], d[1], d[2], d[3], d[9], b'4']
    } else if matches!(d[4], b'1'..=b'9')
        && d[5..=8].iter().all(|digit| *digit == b'0')
        && matches!(d[9], b'5'..=b'9')
    {
        [d[0], d[1], d[2], d[3], d[4], d[9]]
    } else {
        return Err(BarcodeError::Format);
    };
    Ok(compact)
}

fn ean_check_digit(digits: &[u8]) -> u8 {
    // Weight the rightmost payload digit by three, then alternate 1 and 3
    // while moving left. This rule is shared by EAN-13 and UPC-A.
    let sum = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(index, digit)| u32::from(*digit) * if index % 2 == 0 { 3 } else { 1 })
        .sum::<u32>();
    ((10 - sum % 10) % 10) as u8
}

fn push_pattern(modules: &mut Vec<bool>, pattern: &str) {
    modules.extend(pattern.bytes().map(|module| module == b'1'));
}

#[cfg(test)]
mod tests {
    use super::encode_code93;

    #[test]
    fn code93_hri_names_its_start_and_stop_characters() {
        let encoded = encode_code93(b"A").expect("A is valid Code 93 data");

        assert_eq!(encoded.hri, ['□', 'A', '□']);
    }

    #[test]
    fn code93_hri_names_a_shifted_control_character() {
        let encoded = encode_code93(&[0]).expect("NUL has a full-ASCII Code 93 encoding");

        // NUL is encoded as the Code 93 shift pair %U. Epson displays the
        // shift symbol as a black square followed by the pair's letter.
        assert_eq!(encoded.hri, ['□', '■', 'U', '□']);
    }
}
