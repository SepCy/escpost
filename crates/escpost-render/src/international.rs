//! Epson international character substitutions selected by `ESC R`.

/// Returns the replacement for one documented ASCII position.
///
/// A missing replacement means the byte keeps its meaning from the active
/// `ESC t` code page. Listing only differences keeps each country row easy to
/// audit against Epson's international-character table.
pub(crate) fn substitution(character_set: u8, byte: u8) -> Option<char> {
    match character_set {
        0 => None, // U.S.A.
        // France
        1 => match byte {
            b'@' => Some('à'),
            b'[' => Some('°'),
            b'\\' => Some('ç'),
            b']' => Some('§'),
            b'{' => Some('é'),
            b'|' => Some('ù'),
            b'}' => Some('è'),
            _ => None,
        },
        // Germany
        2 => match byte {
            b'@' => Some('§'),
            b'[' => Some('Ä'),
            b'\\' => Some('Ö'),
            b']' => Some('Ü'),
            b'{' => Some('ä'),
            b'|' => Some('ö'),
            b'}' => Some('ü'),
            b'~' => Some('ß'),
            _ => None,
        },
        3 => (byte == b'#').then_some('£'), // U.K.
        // Denmark I
        4 => match byte {
            b'[' => Some('Æ'),
            b'\\' => Some('Ø'),
            b']' => Some('Å'),
            b'{' => Some('æ'),
            b'|' => Some('ø'),
            b'}' => Some('å'),
            _ => None,
        },
        // Sweden
        5 => match byte {
            b'$' => Some('¤'),
            b'@' => Some('É'),
            b'[' => Some('Ä'),
            b'\\' => Some('Ö'),
            b']' => Some('Å'),
            b'^' => Some('Ü'),
            b'`' => Some('é'),
            b'{' => Some('ä'),
            b'|' => Some('ö'),
            b'}' => Some('å'),
            b'~' => Some('ü'),
            _ => None,
        },
        // Italy
        6 => match byte {
            b'[' => Some('°'),
            b']' => Some('é'),
            b'`' => Some('ù'),
            b'{' => Some('à'),
            b'|' => Some('ò'),
            b'}' => Some('è'),
            b'~' => Some('ì'),
            _ => None,
        },
        // Spain I
        7 => match byte {
            b'#' => Some('₧'),
            b'[' => Some('¡'),
            b'\\' => Some('Ñ'),
            b']' => Some('¿'),
            b'{' => Some('¨'),
            b'|' => Some('ñ'),
            _ => None,
        },
        8 => (byte == b'\\').then_some('¥'), // Japan
        // Norway
        9 => match byte {
            b'$' => Some('¤'),
            b'@' => Some('É'),
            b'[' => Some('Æ'),
            b'\\' => Some('Ø'),
            b']' => Some('Å'),
            b'^' => Some('Ü'),
            b'`' => Some('é'),
            b'{' => Some('æ'),
            b'|' => Some('ø'),
            b'}' => Some('å'),
            b'~' => Some('ü'),
            _ => None,
        },
        // Denmark II
        10 => match byte {
            b'@' => Some('É'),
            b'[' => Some('Æ'),
            b'\\' => Some('Ø'),
            b']' => Some('Å'),
            b'^' => Some('Ü'),
            b'`' => Some('é'),
            b'{' => Some('æ'),
            b'|' => Some('ø'),
            b'}' => Some('å'),
            b'~' => Some('ü'),
            _ => None,
        },
        // Spain II
        11 => match byte {
            b'@' => Some('á'),
            b'[' => Some('¡'),
            b'\\' => Some('Ñ'),
            b']' => Some('¿'),
            b'^' => Some('é'),
            b'{' => Some('í'),
            b'|' => Some('ñ'),
            b'}' => Some('ó'),
            b'~' => Some('ú'),
            _ => None,
        },
        // Latin America
        12 => match byte {
            b'@' => Some('á'),
            b'[' => Some('¡'),
            b'\\' => Some('Ñ'),
            b']' => Some('¿'),
            b'^' => Some('é'),
            b'`' => Some('ü'),
            b'{' => Some('í'),
            b'|' => Some('ñ'),
            b'}' => Some('ó'),
            b'~' => Some('ú'),
            _ => None,
        },
        13 => (byte == b'\\').then_some('₩'), // Korea
        // Slovenia/Croatia
        14 => match byte {
            b'@' => Some('Ž'),
            b'[' => Some('Š'),
            b'\\' => Some('Đ'),
            b']' => Some('Ć'),
            b'^' => Some('Č'),
            b'`' => Some('ž'),
            b'{' => Some('š'),
            b'|' => Some('đ'),
            b'}' => Some('ć'),
            b'~' => Some('č'),
            _ => None,
        },
        15 => (byte == b'$').then_some('¥'), // China
        16 => (byte == b'#').then_some('₫'), // Vietnam
        17 => (byte == b'%').then_some('٪'), // Arabia
        _ => None,
    }
}
