//! Foundry scraps with their exact amount (`items/machine/ItemScraps.java`).
//!
//! The Java item is `scraps` with the material as damage and the amount in
//! the tag `amount` (`create`:140-149); without a tag it is an ingot's worth
//! (`getMats`:118-132). Here it is `ntm:scraps__<material>` with the amount in
//! `minecraft:custom_data` as `{ntm_amount:N}`; the data is left out at 72
//! (the same value, and it piles with the plain item).

use crate::server::container_bridge::Stack;

pub const PREFIX: &str = "ntm:scraps__";
/// The amount of a scrap with no data: `MaterialShapes.INGOT.q(1)`.
pub const INGOT: u32 = 72;
const KEY: &str = "ntm_amount";

pub fn is_scrap(item: &str) -> bool {
    item.starts_with(PREFIX)
}

/// The material a scrap item is of.
pub fn material(item: &str) -> Option<&str> {
    item.strip_prefix(PREFIX)
}

/// The scrap of `mat` with `quanta`, one item (`ItemScraps.create`).
pub fn stack(mat: &str, quanta: u32) -> Stack {
    Stack { custom: data(quanta), ..Stack::new(&format!("{PREFIX}{mat}"), 1) }
}

fn data(quanta: u32) -> Option<String> {
    (quanta != INGOT).then(|| format!("{{{KEY}:{quanta}}}"))
}

/// The material and the exact amount of a scrap stack (`getMats`), `None` for another item. Data without
/// `ntm_amount` reads as an ingot's worth (the Java reads 0 there, but nothing here writes such data); an
/// amount that does not read as a number is the Java's missing integer: 0.
pub fn amount_of(item: &str, custom: Option<&str>) -> Option<(String, u32)> {
    let mat = material(item)?;
    let quanta = match custom {
        None => INGOT,
        Some(c) => crate::snbt::fields(c)
            .and_then(|f| f.get(KEY).cloned())
            .map_or(INGOT, |v| v.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse().unwrap_or(0)),
    };
    Some((mat.to_string(), quanta))
}

/// The scraps of molten stacks `(material, quanta)`, one per stack with something in it.
pub fn stacks<'a>(molten: impl IntoIterator<Item = &'a (String, u32)>) -> Vec<Stack> {
    molten.into_iter().filter(|(_, q)| *q > 0).map(|(m, q)| stack(m, *q)).collect()
}

/// A drop's item as the ledger keeps it (`item_to_nbt`): `{components:{...},count:1,id:'...'}`.
pub fn snbt(s: &Stack) -> String {
    match &s.custom {
        Some(c) => format!("{{components:{{'minecraft:custom_data':{c}}},count:{},id:'{}'}}", s.count, s.id),
        None => format!("{{count:{},id:'{}'}}", s.count, s.id),
    }
}

/// The inverse of [`snbt`] (for the hosts that keep the ledger form and the tests).
pub fn from_snbt(text: &str) -> Option<Stack> {
    let f = crate::snbt::fields(text)?;
    let unquote = |v: &String| v.trim_matches(|c| c == '\'' || c == '"').to_string();
    let id = unquote(f.get("id")?);
    let count = f.get("count")?.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse().ok()?;
    let custom = match f.get("components") {
        Some(c) => crate::snbt::fields(c)?.iter().find(|(k, _)| k.trim_matches('\'') == "minecraft:custom_data").map(|(_, v)| v.clone()),
        None => None,
    };
    Some(Stack { id, count, damage: 0, custom })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ingots_worth_has_no_data_and_any_other_amount_is_exact() {
        assert_eq!(stack("iron", 72), Stack::new("ntm:scraps__iron", 1));
        let s = stack("iron", 100);
        assert_eq!(s.custom.as_deref(), Some("{ntm_amount:100}"));
        assert_eq!(amount_of(&s.id, s.custom.as_deref()), Some(("iron".into(), 100)));
        assert_eq!(amount_of("ntm:scraps__iron", None), Some(("iron".into(), 72)));
        assert_eq!(amount_of("minecraft:iron_ingot", None), None);
    }

    #[test]
    fn a_tag_without_a_readable_amount_is_the_java_default() {
        // A tag without the amount is an ingot's worth (declared deviation); a bad amount reads 0.
        assert_eq!(amount_of("ntm:scraps__iron", Some("{other:1}")), Some(("iron".into(), 72)));
        assert_eq!(amount_of("ntm:scraps__iron", Some("{ntm_amount:5}")), Some(("iron".into(), 5)));
        assert_eq!(amount_of("ntm:scraps__iron", Some("{ntm_amount:5s}")), Some(("iron".into(), 5)));
        assert_eq!(amount_of("ntm:scraps__iron", Some("{ntm_amount:\"x\"}")), Some(("iron".into(), 0)));
    }

    #[test]
    fn the_ledger_form_of_a_drop() {
        assert_eq!(snbt(&stack("iron", 72)), "{count:1,id:'ntm:scraps__iron'}");
        assert_eq!(snbt(&stack("iron", 5)), "{components:{'minecraft:custom_data':{ntm_amount:5}},count:1,id:'ntm:scraps__iron'}");
    }

    #[test]
    fn the_ledger_form_reads_back() {
        for q in [1, 5, 72, 100, 10_368] {
            let s = stack("iron", q);
            assert_eq!(from_snbt(&snbt(&s)), Some(s), "{q}");
        }
    }

    #[test]
    fn every_molten_stack_is_one_scrap_with_all_its_quanta() {
        let molten = vec![("iron".to_string(), 150), ("lead".to_string(), 0), ("steel".to_string(), 72)];
        let s = stacks(&molten);
        let sum: u32 = s.iter().map(|x| amount_of(&x.id, x.custom.as_deref()).unwrap().1).sum();
        assert_eq!(s.len(), 2);
        assert_eq!(sum, 222);
    }
}
