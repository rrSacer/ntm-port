//! Block states as the `world` capability writes them:
//! `ntm:machine_battery[facing=north,honey_level=3]`.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockState {
    pub id: String,
    /// In the order given; `with` replaces in place.
    pub props: Vec<(String, String)>,
}

impl BlockState {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            props: Vec::new(),
        }
    }

    /// `id` or `id[k=v,...]`; `None` for a malformed string.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let Some(open) = s.find('[') else {
            return (!s.is_empty()).then(|| Self::new(s));
        };
        let inner = s[open + 1..].strip_suffix(']')?;
        let mut props = Vec::new();
        for kv in inner.split(',').filter(|kv| !kv.is_empty()) {
            let (k, v) = kv.split_once('=')?;
            props.push((k.trim().into(), v.trim().into()));
        }
        Some(Self {
            id: s[..open].into(),
            props,
        })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.props
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn with(mut self, key: &str, value: impl ToString) -> Self {
        let value = value.to_string();
        match self.props.iter_mut().find(|(k, _)| k == key) {
            Some(p) => p.1 = value,
            None => self.props.push((key.into(), value)),
        }
        self
    }
}

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)?;
        if self.props.is_empty() {
            return Ok(());
        }
        f.write_str("[")?;
        for (i, (k, v)) in self.props.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "{k}={v}")?;
        }
        f.write_str("]")
    }
}

/// Whether the state `actual` (as the world reports it, every property)
/// shows `want` (only some properties): the same id, and each `k=v` of `want`
/// in `actual`. The same answer as comparing two [`BlockState::parse`]s, with
/// no allocation: the server asks this for every loaded machine (measured in
/// docs/bombas.md, "Pool"; ~3 k fuel per parse in WebAssembly).
pub fn shows(actual: &str, want: &str) -> bool {
    // O caso comum: o mundo mostra exatamente o que a lógica quer (mesma
    // ordem de propriedades). Bytes, sem `trim` nem iteradores.
    if actual.len() == want.len() && actual.as_bytes() == want.as_bytes() && plain(want) {
        return true;
    }
    shows_slow(actual, want)
}

/// `want` sem espaços nas pontas, e bem formado: `id` ou `id[k=v,…]` com
/// todo pedaço não vazio tendo `=` (o que `BlockState::parse` aceita).
fn plain(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b[0].is_ascii_whitespace() || b[b.len() - 1].is_ascii_whitespace() || !s.is_ascii() {
        return false;
    }
    let Some(open) = b.iter().position(|&c| c == b'[') else {
        return true;
    };
    if open == 0 || b[b.len() - 1] != b']' {
        return false;
    }
    let mut eq = false;
    let mut empty = true;
    for &c in &b[open + 1..b.len() - 1] {
        match c {
            b',' => {
                if !empty && !eq {
                    return false;
                }
                eq = false;
                empty = true;
            }
            b'=' => {
                eq = true;
                empty = false;
            }
            b' ' | b'[' | b']' => return false,
            _ => empty = false,
        }
    }
    empty || eq
}

fn shows_slow(actual: &str, want: &str) -> bool {
    fn split(s: &str) -> Option<(&str, &str)> {
        let s = s.trim();
        match s.find('[') {
            None => (!s.is_empty()).then_some((s, "")),
            Some(open) => Some((&s[..open], s[open + 1..].strip_suffix(']')?)),
        }
    }
    fn props(p: &str) -> impl Iterator<Item = &str> {
        p.split(',').filter(|kv| !kv.is_empty())
    }
    let (Some((aid, ap)), Some((wid, wp))) = (split(actual), split(want)) else {
        return false;
    };
    if aid != wid || !props(ap).chain(props(wp)).all(|kv| kv.contains('=')) {
        return false;
    }
    // The world and the logic write properties in the same order: equal
    // strings answer at once.
    if ap == wp {
        return true;
    }
    props(wp).all(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        props(ap)
            .filter_map(|x| x.split_once('='))
            .find(|(ak, _)| ak.trim() == k.trim())
            .is_some_and(|(_, av)| av.trim() == v.trim())
    })
}

/// The id of a state string, without parsing the properties.
pub fn id_of(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s = BlockState::parse("ntm:machine_battery[facing=north,honey_level=3]").unwrap();
        assert_eq!(s.id, "ntm:machine_battery");
        assert_eq!(s.get("honey_level"), Some("3"));
        let s = s.with("honey_level", 5);
        assert_eq!(s.to_string(), "ntm:machine_battery[facing=north,honey_level=5]");
        assert_eq!(BlockState::parse("minecraft:stone").unwrap().to_string(), "minecraft:stone");
        assert!(BlockState::parse("a[b]").is_none());
        assert_eq!(id_of("a:b[c=d]"), "a:b");
    }

    /// `shows` answers as the two parses did, on states of every shape.
    #[test]
    fn shows_matches_the_parsed_comparison() {
        fn parsed(actual: &str, want: &str) -> bool {
            let (Some(a), Some(w)) = (BlockState::parse(actual), BlockState::parse(want)) else {
                return false;
            };
            a.id == w.id && w.props.iter().all(|(k, v)| a.get(k) == Some(v.as_str()))
        }
        let states = [
            "ntm:machine_battery[facing=north,honey_level=3]",
            "ntm:machine_battery[honey_level=3,facing=north]",
            "ntm:machine_battery[facing=north,honey_level=0]",
            "ntm:machine_battery[facing=south,honey_level=3]",
            "ntm:machine_battery[facing=north]",
            "ntm:machine_battery[honey_level=3]",
            "ntm:machine_battery",
            "ntm:machine_press[facing=north,honey_level=3]",
            "ntm:machine_battery[facing=north,honey_level=3",
            "ntm:machine_battery[facing,honey_level=3]",
            "ntm:machine_battery[]",
            " ntm:machine_battery[facing=north, honey_level=3] ",
            "ntm:machine_battery[facing=north,,honey_level=3]",
            "minecraft:stone",
            "",
            "a[b]",
        ];
        for a in states {
            for w in states {
                assert_eq!(shows(a, w), parsed(a, w), "{a:?} mostra {w:?}");
            }
        }
    }
}
