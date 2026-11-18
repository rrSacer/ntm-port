//! `BrokenItem`: what a faulty precision-assembler run leaves instead of its
//! product, and what the recycling recipe takes back.
//!
//! The Java item (`BrokenItem.make`) keeps the product in the stack's NBT as
//! `itemID` and `itemMeta` and takes the product's stack size as its own;
//! the product's own NBT is dropped. Here a subtype is part of the item id
//! (no metadata), so the item keeps the product id in its `custom_data`:
//! `{ntm_broken:{id:"ntm:upgrade_speed_2"}}`. The count is the product's.
use crate::{server::container_bridge::Stack, snbt};

pub const ID: &str = "ntm:broken_item";
const KEY: &str = "ntm_broken";

/// `BrokenItem.make(stack)`: the broken stack for `count` of `product`.
pub fn make(product: &str, count: u32) -> Option<Stack> {
    if product.is_empty() || product.contains(['"', '\\']) {
        return None;
    }
    let custom = snbt::canonical(&format!("{{{KEY}:{{id:\"{product}\"}}}}"))?;
    Some(Stack { custom: Some(custom), ..Stack::new(ID, count) })
}

/// The product a broken stack stands for. `None` for any other item, and for
/// a broken item without data (`getItemStackDisplayName` falls back to the
/// bare name when the tag or the product is missing).
pub fn product(stack: &Stack) -> Option<String> {
    if stack.id != ID {
        return None;
    }
    let inner = snbt::fields(stack.custom.as_deref()?)?.remove(KEY)?;
    let id = snbt::fields(&inner)?.remove("id")?;
    let id = id.strip_prefix('"')?.strip_suffix('"')?;
    // An escaped quote or backslash is no id `make` writes (it refuses them) nor a real one: not a product.
    (!id.is_empty() && !id.contains('\\')).then(|| id.to_string())
}

/// `NBTStack.matchesRecipe` for the recycling input `BrokenItem.make(product)`:
/// the item is the broken one with meta 0 (`ComparableStack` compares it), its
/// tag holds the product's id (extra tags are ignored) and the stack is at
/// least as big as the product's count.
pub fn matches(stack: &Stack, product_id: &str, count: u32) -> bool {
    stack.damage == 0 && stack.count >= count && product(stack).as_deref() == Some(product_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_item_remembers_its_product_and_its_count() {
        let s = make("ntm:upgrade_speed_2", 1).unwrap();
        assert_eq!((s.id.as_str(), s.count, s.damage), (ID, 1, 0));
        assert_eq!(s.custom.as_deref(), Some("{ntm_broken:{id:\"ntm:upgrade_speed_2\"}}"));
        assert_eq!(product(&s).as_deref(), Some("ntm:upgrade_speed_2"));
        assert_eq!(make("ntm:crystalcircuit", 3).unwrap().count, 3);
    }

    #[test]
    fn the_engines_spelling_of_the_tag_reads_the_same() {
        let mut s = make("ntm:a", 1).unwrap();
        s.custom = Some("{ntm_broken: {id: \"ntm:a\"}}".into());
        assert_eq!(product(&s).as_deref(), Some("ntm:a"));
        assert!(matches(&s, "ntm:a", 1));
    }

    #[test]
    fn only_the_same_product_is_recycled() {
        let s = make("ntm:a", 2).unwrap();
        assert!(matches(&s, "ntm:a", 2));
        assert!(matches(&s, "ntm:a", 1));
        assert!(!matches(&s, "ntm:a", 3), "the recipe needs the product's stack size");
        assert!(!matches(&s, "ntm:b", 1));
    }

    #[test]
    fn what_is_not_a_broken_product_is_refused() {
        assert!(product(&Stack::new("ntm:a", 1)).is_none());
        let mut bare = Stack::new(ID, 1);
        assert!(product(&bare).is_none(), "no tag: the bare item");
        bare.custom = Some("{ntm_broken:{id:\"\"}}".into());
        assert!(product(&bare).is_none());
        bare.custom = Some("{other:{id:\"ntm:a\"}}".into());
        assert!(product(&bare).is_none());
        assert!(!matches(&bare, "ntm:a", 1));
        assert!(make("", 1).is_none() && make("a\"b", 1).is_none());
        // Another item carrying the tag is no broken item (the item check of `product`).
        let mut other = make("ntm:a", 1).unwrap();
        other.id = "ntm:upgrade_speed_2".into();
        assert!(product(&other).is_none());
        assert!(!matches(&other, "ntm:a", 1));
        // An escaped id (someone's command with a malformed tag) is no product.
        let mut escaped = make("ntm:a", 1).unwrap();
        escaped.custom = Some("{ntm_broken:{id:\"ntm:a\\\"b\"}}".into());
        assert!(product(&escaped).is_none());
    }

    #[test]
    fn the_recycling_wants_meta_zero() {
        let mut s = make("ntm:a", 1).unwrap();
        assert!(matches(&s, "ntm:a", 1));
        s.damage = 7;
        assert!(!matches(&s, "ntm:a", 1), "the NBTStack's meta is 0");
    }

    #[test]
    fn products_do_not_pile_in_one_stack() {
        assert_ne!(make("ntm:a", 1).unwrap().custom, make("ntm:b", 1).unwrap().custom);
    }
}
