//! The numbers an HBM 1.7.10 screen reads from its text fields (RBMK T06): commons-lang3 3.3.2's
//! `NumberUtils.isNumber`, `Double.parseDouble` and the save rule of `GUIRBMKControlAuto.mouseClicked:104-118`
//! (`(int) MathHelper.clamp_double(Double.parseDouble(text), 0, clamp)`, `"0"` for text that is not a number). Checked
//! against the Java over every string of up to four characters of the relevant alphabet
//! (`tools/screens-oracle/NumberOracle.java`).

/// `NumberUtils.isNumber` (commons-lang3 3.3.2): Java's number literals, `0x` hexadecimals, octals (a leading `0`
/// before a digit: `08` is not a number) and the `d`/`f`/`l` qualifiers included. It passes text `Double.parseDouble` throws on (`1L`, `0x1`).
pub fn is_number(s: &str) -> bool {
    let chars: Vec<u16> = s.encode_utf16().collect();
    if chars.is_empty() {
        return false;
    }
    let is = |c: u16, b: u8| c == u16::from(b);
    let digit = |c: u16| (u16::from(b'0')..=u16::from(b'9')).contains(&c);
    let mut sz = chars.len();
    let mut has_exp = false;
    let mut has_dec_point = false;
    let mut allow_signs = false;
    let mut found_digit = false;
    let start = usize::from(is(chars[0], b'-'));
    if sz > start + 1 && is(chars[start], b'0') && (is(chars[start + 1], b'x') || is(chars[start + 1], b'X')) {
        let i = start + 2;
        if i == sz {
            return false;
        }
        return chars[i..].iter().all(|&c| {
            digit(c) || (u16::from(b'a')..=u16::from(b'f')).contains(&c) || (u16::from(b'A')..=u16::from(b'F')).contains(&c)
        });
    }
    // A leading zero before a digit: octal, every following character 0 to 7.
    if sz > start + 1 && is(chars[start], b'0') && digit(chars[start + 1]) {
        return chars[start + 1..].iter().all(|&c| (u16::from(b'0')..=u16::from(b'7')).contains(&c));
    }
    sz -= 1;
    let mut i = start;
    while i < sz || (i < sz + 1 && allow_signs && !found_digit) {
        let c = chars[i];
        if digit(c) {
            found_digit = true;
            allow_signs = false;
        } else if is(c, b'.') {
            if has_dec_point || has_exp {
                return false;
            }
            has_dec_point = true;
        } else if is(c, b'e') || is(c, b'E') {
            if has_exp || !found_digit {
                return false;
            }
            has_exp = true;
            allow_signs = true;
        } else if is(c, b'+') || is(c, b'-') {
            if !allow_signs {
                return false;
            }
            allow_signs = false;
            found_digit = false;
        } else {
            return false;
        }
        i += 1;
    }
    if i < chars.len() {
        let c = chars[i];
        if digit(c) {
            return true;
        }
        if is(c, b'e') || is(c, b'E') {
            return false;
        }
        if is(c, b'.') {
            return !(has_dec_point || has_exp) && found_digit;
        }
        if !allow_signs && (is(c, b'd') || is(c, b'D') || is(c, b'f') || is(c, b'F')) {
            return found_digit;
        }
        if is(c, b'l') || is(c, b'L') {
            return found_digit && !has_exp && !has_dec_point;
        }
        return false;
    }
    !allow_signs && found_digit
}

/// `Double.parseDouble` (`FloatingDecimal.readJavaFormatString`): the text trimmed of characters up to the space, a
/// sign, `NaN` or `Infinity`, or a decimal literal (digits with at most one point, at least one digit, an optional
/// exponent) with an optional `f`/`F`/`d`/`D`, correctly rounded. None where the Java throws. Hexadecimal literals
/// need a `p` exponent and five characters at least, past any field of the RBMK's: they read as None.
pub fn parse_double(s: &str) -> Option<f64> {
    let t = s.trim_matches(|c: char| c <= ' ');
    let (negative, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let sign = |v: f64| if negative { -v } else { v };
    match body {
        "NaN" => return Some(f64::NAN),
        "Infinity" => return Some(sign(f64::INFINITY)),
        _ => {}
    }
    let b = body.as_bytes();
    let mut i = 0;
    let mut mantissa_digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        mantissa_digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            mantissa_digits += 1;
        }
    }
    if mantissa_digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let digits_from = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == digits_from {
            return None;
        }
    }
    let end_of_number = i;
    if i < b.len() && matches!(b[i], b'f' | b'F' | b'd' | b'D') {
        i += 1;
    }
    if i != b.len() {
        return None;
    }
    body[..end_of_number].parse::<f64>().ok().map(sign)
}

/// `MathHelper.clamp_double`: NaN passes through (both comparisons false).
pub fn clamp_double(v: f64, min: f64, max: f64) -> f64 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// Java's `(int)` of a double: toward zero, NaN to 0, saturating.
pub fn java_int(v: f64) -> i32 {
    v as i32
}

/// What saving a field does (`GUIRBMKControlAuto.mouseClicked:104-118`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Saved {
    /// A number: the field shows `value` and the screen sends it.
    Number(i32),
    /// Not a number: the field shows `0` and the screen sends 0.
    NotANumber,
    /// `isNumber` passes and `parseDouble` throws: the Java's click handler dies there (a crash of the 1.7.10
    /// client), before any packet.
    Throws,
}

/// The save rule of a field clamped to [0, `clamp`].
pub fn save(text: &str, clamp: f64) -> Saved {
    if !is_number(text) {
        return Saved::NotANumber;
    }
    match parse_double(text) {
        Some(d) => Saved::Number(java_int(clamp_double(d, 0.0, clamp))),
        None => Saved::Throws,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALPHABET: &str = "018.-+eExXdDfFlLa ";

    /// The oracle's lines for the strings `isNumber` passes, as the Java printed them.
    fn oracle() -> std::collections::HashMap<String, (String, String, String)> {
        let text = include_str!("../../../../tools/screens-oracle/fixtures/numbers.tsv");
        text.lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                let s: String = f[0]
                    .split("\\u")
                    .filter(|h| !h.is_empty())
                    .map(|h| char::from_u32(u32::from_str_radix(h, 16).unwrap()).unwrap())
                    .collect();
                assert_eq!(f[1], "1", "the fixture keeps the strings isNumber passes");
                (s, (f[2].to_string(), f[3].to_string(), f[4].to_string()))
            })
            .collect()
    }

    fn all_strings() -> Vec<String> {
        let a: Vec<char> = ALPHABET.chars().collect();
        let mut out = vec![String::new()];
        let mut level = vec![String::new()];
        for _ in 0..4 {
            level = level.iter().flat_map(|s| a.iter().map(move |c| format!("{s}{c}"))).collect();
            out.extend(level.iter().cloned());
        }
        out
    }

    #[test]
    fn is_number_parse_and_save_match_the_java_on_every_short_string() {
        let java = oracle();
        let all = all_strings();
        assert_eq!(all.len(), 111_151, "as many strings as the oracle ran");
        for s in &all {
            let mine = is_number(s);
            assert_eq!(mine, java.contains_key(s), "isNumber({s:?})");
            if let Some((bits, c100, c9999)) = java.get(s) {
                match parse_double(s) {
                    // `doubleToRawLongBits`, a signed long.
                    Some(d) => assert_eq!(Ok(d.to_bits() as i64), bits.parse::<i64>(), "parseDouble({s:?})"),
                    None => assert_eq!(bits, "EXC", "parseDouble({s:?}) throws"),
                }
                let expect = |c: &str| if c == "EXC" { Saved::Throws } else { Saved::Number(c.parse().unwrap()) };
                assert_eq!(save(s, 100.0), expect(c100), "save({s:?}, 100)");
                assert_eq!(save(s, 9999.0), expect(c9999), "save({s:?}, 9999)");
            } else {
                assert_eq!(save(s, 100.0), Saved::NotANumber, "save({s:?})");
            }
        }
    }

    #[test]
    fn parse_double_reads_what_isnumber_refuses_too() {
        // Strings isNumber refuses still parse (the save rule never asks): Java's grammar, not isNumber's.
        assert_eq!(parse_double(" 1 "), Some(1.0));
        assert_eq!(parse_double("+1"), Some(1.0));
        assert!(parse_double("NaN").unwrap().is_nan());
        assert_eq!(parse_double("-Infinity"), Some(f64::NEG_INFINITY));
        assert_eq!(parse_double("1e400"), Some(f64::INFINITY));
        assert_eq!(parse_double("inf"), None, "Rust's words are not Java's");
        assert_eq!(parse_double("0x1p3"), None, "hexadecimals read as None (past any RBMK field)");
    }

    #[test]
    fn clamp_and_cast_are_javas() {
        assert!(clamp_double(f64::NAN, 0.0, 100.0).is_nan());
        assert_eq!(java_int(f64::NAN), 0);
        assert_eq!(java_int(99.99), 99);
        assert_eq!(java_int(-0.5), 0);
        assert_eq!(java_int(1e10), i32::MAX);
    }
}
