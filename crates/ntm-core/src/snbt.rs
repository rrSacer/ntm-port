//! A canonical form of SNBT (the text of an item's `minecraft:custom_data`):
//! the engine writes it back in its own order and number format
//! (`{ntm_ore:{rare:0.0d,light:1.5d}}` for `{ntm_ore:{light:1.5d,rare:0d}}`),
//! so the port compares and writes it canonical: compound keys sorted,
//! doubles and floats in their shortest form, whitespace dropped.

/// The canonical form of `s`, or `None` if it isn't SNBT this reads.
pub fn canonical(s: &str) -> Option<String> {
    let mut p = Parser { s: s.as_bytes(), i: 0 };
    let mut out = String::new();
    p.value(&mut out)?;
    p.ws();
    (p.i == p.s.len()).then_some(out)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn value(&mut self, out: &mut String) -> Option<()> {
        match self.peek()? {
            b'{' => self.compound(out),
            b'[' => self.list(out),
            b'"' | b'\'' => {
                let t = self.quoted()?;
                out.push_str(&quote(&t));
                Some(())
            }
            _ => {
                let w = self.word()?;
                out.push_str(&number(&w).unwrap_or(w));
                Some(())
            }
        }
    }

    fn compound(&mut self, out: &mut String) -> Option<()> {
        self.eat(b'{');
        let mut entries: Vec<(String, String)> = Vec::new();
        if !self.eat(b'}') {
            loop {
                let key = if matches!(self.peek()?, b'"' | b'\'') { self.quoted()? } else { self.word()? };
                if !self.eat(b':') {
                    return None;
                }
                let mut v = String::new();
                self.value(&mut v)?;
                entries.push((key, v));
                if self.eat(b'}') {
                    break;
                }
                if !self.eat(b',') {
                    return None;
                }
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        out.push('{');
        for (k, (key, v)) in entries.iter().enumerate() {
            if k > 0 {
                out.push(',');
            }
            if key.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b'+')) && !key.is_empty() {
                out.push_str(key);
            } else {
                out.push_str(&quote(key));
            }
            out.push(':');
            out.push_str(v);
        }
        out.push('}');
        Some(())
    }

    fn list(&mut self, out: &mut String) -> Option<()> {
        self.eat(b'[');
        out.push('[');
        // Typed arrays: [B;...], [I;...], [L;...].
        if let Some(t) = self.s.get(self.i..self.i + 2)
            && matches!(t[0], b'B' | b'I' | b'L')
            && t[1] == b';'
        {
            out.push(t[0] as char);
            out.push(';');
            self.i += 2;
        }
        let mut first = true;
        if !self.eat(b']') {
            loop {
                if !first {
                    out.push(',');
                }
                first = false;
                self.value(out)?;
                if self.eat(b']') {
                    break;
                }
                if !self.eat(b',') {
                    return None;
                }
            }
        }
        out.push(']');
        Some(())
    }

    fn quoted(&mut self) -> Option<String> {
        let q = self.s[self.i];
        self.i += 1;
        let mut t = Vec::new();
        while self.i < self.s.len() {
            let c = self.s[self.i];
            self.i += 1;
            if c == b'\\' {
                t.push(*self.s.get(self.i)?);
                self.i += 1;
            } else if c == q {
                return String::from_utf8(t).ok();
            } else {
                t.push(c);
            }
        }
        None
    }

    fn word(&mut self) -> Option<String> {
        self.ws();
        let start = self.i;
        while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || matches!(self.s[self.i], b'_' | b'-' | b'.' | b'+')) {
            self.i += 1;
        }
        (self.i > start).then(|| String::from_utf8_lossy(&self.s[start..self.i]).into_owned())
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A number word in its canonical form: doubles (`d`, or a point without a
/// suffix) and floats in their shortest form, integers as written.
fn number(w: &str) -> Option<String> {
    let (body, suffix) = match w.as_bytes().last()? {
        c if c.is_ascii_alphabetic() => (&w[..w.len() - 1], c.to_ascii_lowercase() as char),
        _ => (w, if w.contains('.') || w.contains('e') || w.contains('E') { 'd' } else { 'i' }),
    };
    match suffix {
        'd' => Some(format!("{:?}d", body.parse::<f64>().ok()?)),
        'f' => Some(format!("{:?}f", body.parse::<f32>().ok()?)),
        'b' | 's' | 'l' | 'i' => {
            let v: i64 = body.parse().ok()?;
            Some(if suffix == 'i' { v.to_string() } else { format!("{v}{suffix}") })
        }
        _ => None,
    }
}

/// Top-level compound fields, using the same parser as canonicalisation.
/// Nested tags and quoted delimiters are preserved.
pub fn fields(s: &str) -> Option<std::collections::BTreeMap<String, String>> {
    let mut p = Parser { s: s.as_bytes(), i: 0 };
    if !p.eat(b'{') { return None; }
    let mut fields = std::collections::BTreeMap::new();
    if !p.eat(b'}') {
        loop {
            let key = if matches!(p.peek()?, b'"' | b'\'') { p.quoted()? } else { p.word()? };
            if !p.eat(b':') { return None; }
            let mut value = String::new();
            p.value(&mut value)?;
            fields.insert(key, value);
            if p.eat(b'}') { break; }
            if !p.eat(b',') { return None; }
        }
    }
    p.ws();
    (p.i == p.s.len()).then_some(fields)
}

/// Replace one field, preserving the rest of a valid custom-data compound.
pub fn with_field(s: Option<&str>, key: &str, value: &str) -> String {
    let mut fields = s.and_then(fields).unwrap_or_default();
    fields.insert(key.into(), value.into());
    let body = fields.iter().map(|(k,v)| format!("{}:{v}",quote(k))).collect::<Vec<_>>().join(",");
    canonical(&format!("{{{body}}}")).expect("valid compound field")
}

#[cfg(test)]
mod tests {
    use super::canonical;

    #[test]
    fn the_engines_order_and_numbers_are_the_same_data() {
        let mine = "{ntm_ore:{light:0.229536d,heavy:0.405059d,rare:0d,actinide:2.2d}}";
        let engine = "{ntm_ore: {rare: 0.0d, actinide: 2.2d, light: 0.229536d, heavy: 0.405059d}}";
        assert_eq!(canonical(mine), canonical(engine));
        assert_eq!(canonical(mine).as_deref(), Some("{ntm_ore:{actinide:2.2d,heavy:0.405059d,light:0.229536d,rare:0.0d}}"));
        assert_eq!(canonical("{a:1b,b:\"x y\",c:[I;1,2],d:1.5f}").as_deref(), Some("{a:1b,b:\"x y\",c:[I;1,2],d:1.5f}"));
        assert_eq!(canonical("{a:1"), None);
    }
}

