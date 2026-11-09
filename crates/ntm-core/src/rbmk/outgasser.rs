//! The outgasser's recipes (`OutgasserRecipes.registerDefaults`, executed by
//! the oracle) and its fluid/item bookkeeping.

use super::tables_data::OUTGASSER;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutgasserInput {
    /// A `ComparableStack`: item name and metadata.
    Stack(&'static str, i32),
    /// An `OreDictStack`.
    Ore(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct OutgasserRecipe {
    pub input: OutgasserInput,
    /// `(item, count, metadata)`.
    pub solid: Option<(&'static str, i32, i32)>,
    /// `(fluid, amount)`.
    pub fluid: Option<(&'static str, i32)>,
    pub fusion_only: bool,
}

/// An item stack as the outgasser sees it. `max_stack` is the item's stack limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: i32,
    pub meta: i32,
    pub max_stack: i32,
}

/// `OutgasserRecipes.getOutput`: the exact item first, then its ore-dictionary
/// names in the order the host gives them.
pub fn recipe_for(item: &str, meta: i32, ore_names: &[String]) -> Option<&'static OutgasserRecipe> {
    if let Some(r) = OUTGASSER
        .iter()
        .find(|r| matches!(r.input, OutgasserInput::Stack(i, m) if i == item && m == meta))
    {
        return Some(r);
    }
    for key in ore_names {
        if let Some(r) = OUTGASSER
            .iter()
            .find(|r| matches!(r.input, OutgasserInput::Ore(k) if k == key))
        {
            return Some(r);
        }
    }
    None
}

/// `TileEntityRBMKOutgasser.duration`.
pub const DURATION: f64 = 10_000.0;
/// The gas tank's size.
pub const GAS_MAX: i32 = 64_000;
