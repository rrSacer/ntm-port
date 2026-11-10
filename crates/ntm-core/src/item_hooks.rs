//! Item-family contracts, independent of the native host. The use table is
//! `consumables`; the hazard and gas-mask rules (HazmatRegistry, ArmorUtil)
//! are here.
use std::{collections::HashMap, sync::OnceLock};

pub use crate::consumables::{
    Animation, Consume, Cure, Effect, Gate, Give, Hurt, Overflow, UseSpec, use_spec,
};

/// All stack hazards use one endpoint interpolation. Reject non-finite state
/// rather than allowing NaN to erase a hazard. Values outside the life range
/// clamp to their corresponding endpoint.
pub fn interpolate(fresh: f64, spent: f64, remaining: f64, maximum: f64) -> Option<f64> {
    if ![fresh, spent, remaining, maximum]
        .iter()
        .all(|x| x.is_finite())
        || maximum <= 0.0
        || fresh < 0.0
        || spent < 0.0
    {
        return None;
    }
    Some(spent + (fresh - spent) * (remaining / maximum).clamp(0.0, 1.0))
}

/// Shared Java HazardModifier interpolation: the endpoint difference is
/// evaluated in float, interpolation in double, result quantized in float.
/// The caller chooses its life curve/clamping (RTG and ZIRNOX differ).
pub fn hazard_rate(base: f32, target: f32, fraction: f64) -> Option<u32> {
    if !base.is_finite() || !target.is_finite() || !fraction.is_finite() {
        return None;
    }
    let level = (f64::from(base) + f64::from(target - base) * fraction) as f32;
    Some((level * 1000.0).round() as u32)
}

#[derive(serde::Deserialize)]
struct Row {
    value: f64,
}
#[derive(serde::Deserialize)]
struct Classes {
    classes: Vec<String>,
}
#[derive(serde::Deserialize)]
struct Hazmat {
    resistance: HashMap<String, Row>,
    cladding: HashMap<String, Row>,
    classes: HashMap<String, Classes>,
    filters: Vec<String>,
}
/// `HazmatRegistry.initDefault` and the `ItemModCladding` values, generated
/// from the Java by `tools/content-gen/hazmat.py`.
fn hazmat() -> &'static Hazmat {
    static T: OnceLock<Hazmat> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("../data/hazmat.json")).expect("checked-in hazmat table"))
}

/// `HazmatRegistry.entries.get(item)`: the registered resistance, 0 when the
/// item has none.
pub fn resistance(id: &str) -> f64 {
    hazmat().resistance.get(id).map_or(0.0, |r| r.value)
}
/// `ItemModCladding.rad`, `None` when the item is no radiation cladding.
pub fn cladding_rad(id: &str) -> Option<f64> {
    hazmat().cladding.get(id).map(|r| r.value)
}
/// `ArmorModHandler.cladding`, the mod slot `getCladding` reads.
pub const CLADDING_SLOT: usize = 5;

/// An `ItemArmorMod`: the mod slot it goes in (`type`) and the armor types
/// it applies to (helmet, chestplate, leggings, boots).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArmorMod {
    pub slot: usize,
    pub applies: [bool; 4],
}
/// The port's `ItemArmorMod`s: `ItemModGasmask` (helmet only, slot 0) and the
/// radiation claddings (`ItemModCladding`, slot 5, every armor type).
pub fn armor_mod(id: &str) -> Option<ArmorMod> {
    if id == "ntm:attachment_mask" {
        return Some(ArmorMod { slot: 0, applies: [true, false, false, false] });
    }
    cladding_rad(id).map(|_| ArmorMod { slot: CLADDING_SLOT, applies: [true; 4] })
}
/// `ContainerArmorTable` mod slot `isItemValid`: an armor piece is in the
/// table, `ArmorModHandler.isApplicable(armor, mod)` and `type == slot`.
pub fn mod_fits(armor_type: Option<usize>, slot: usize, id: &str) -> bool {
    match (armor_type, armor_mod(id)) {
        (Some(t), Some(m)) => m.slot == slot && m.applies.get(t).copied().unwrap_or(false),
        _ => false,
    }
}

/// `ModEventHandlerClient` "CLADDING (LEGACY)": every item with a positive
/// `getResistance(stack)`, truncated to three decimals (en_US text).
pub fn resistance_line(rad: f64) -> Option<String> {
    let rad = ((rad * 1000.0) as i64) as f64 / 1000.0;
    (rad > 0.0).then(|| format!("§eRadiation resistance: {}", crate::instruments::java_f64(rad)))
}
/// `ItemModCladding.addInformation` and the `ItemArmorMod` lines after it.
pub fn cladding_lines(id: &str) -> Option<Vec<String>> {
    let rad = cladding_rad(id)?;
    Some(vec![
        format!("§e+{} rad-resistance", crate::instruments::java_f64(rad)),
        String::new(),
        "§5Applicable To:".into(),
        "  All".into(),
        "§5Slot:".into(),
        "  Cladding".into(),
    ])
}

/// One worn piece as `HazmatRegistry.getResistance(ItemStack)` sees it.
pub struct Piece<'a> {
    pub id: &'a str,
    /// `stackTagCompound.getFloat("hfr_cladding")` (0 when absent).
    pub hfr_cladding: f32,
    /// The item in the cladding mod slot, if any.
    pub cladding_mod: Option<&'a str>,
}
/// `getCladding`: the `hfr_cladding` float wins when positive, else the
/// `ItemModCladding` in the cladding slot; `entries.get(item) + cladding`.
pub fn piece_resistance(p: &Piece) -> f64 {
    let cladding = if p.hfr_cladding > 0.0 {
        f64::from(p.hfr_cladding)
    } else {
        p.cladding_mod.and_then(cladding_rad).unwrap_or(0.0)
    };
    resistance(p.id) + cladding
}
/// `HazmatRegistry.getResistance(EntityPlayer)`: a `float` accumulates each
/// piece's `double` (`res += d` narrows after every addition), then `+0.2F`
/// while Rad-X is active.
pub fn player_resistance(pieces: impl IntoIterator<Item = f64>, radx: bool) -> f32 {
    let mut res = 0f32;
    for piece in pieces {
        res = (f64::from(res) + piece) as f32;
    }
    if radx {
        res += 0.2f32;
    }
    res
}
/// `ContaminationUtil.calculateRadiationMod`: `(float) Math.pow(10.0F, -res)`.
pub fn radiation_mod(res: f32) -> f32 {
    10f64.powf(-f64::from(res)) as f32
}
/// The factor of bare pieces (no cladding): `radiation_mod(player_resistance)`.
pub fn dose_factor<'a>(equipment: impl IntoIterator<Item = &'a str>, radx: bool) -> f64 {
    f64::from(radiation_mod(player_resistance(equipment.into_iter().map(resistance), radx)))
}

/// `ArmorRegistry.HazardClass`, plus `Gamma` for the dose path (resistance,
/// not a class).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hazard {
    Gamma,
    CoarseParticles,
    FineParticles,
    LungGas,
    BlisteringGas,
    Bacteria,
    CarbonMonoxide,
    InertGas,
    Sand,
    Light,
}
impl Hazard {
    /// The `HazardClass` constant's Java name.
    pub fn of_java(name: &str) -> Option<Self> {
        use Hazard::*;
        Some(match name {
            "PARTICLE_COARSE" => CoarseParticles,
            "PARTICLE_FINE" => FineParticles,
            "GAS_LUNG" => LungGas,
            "GAS_BLISTERING" => BlisteringGas,
            "BACTERIA" => Bacteria,
            "GAS_MONOXIDE" => CarbonMonoxide,
            "GAS_INERT" => InertGas,
            "SAND" => Sand,
            "LIGHT" => Light,
            _ => return None,
        })
    }
}
/// `ArmorRegistry.hazardClasses.get(item)`: what the item stops by itself
/// (`ArmorUtil.register` and `setHazardClass`, generated in hazmat.json).
pub fn registered_classes(id: &str) -> Vec<Hazard> {
    hazmat()
        .classes
        .get(id)
        .map(|r| r.classes.iter().filter_map(|c| Hazard::of_java(c)).collect())
        .unwrap_or_default()
}
/// The hazard classes an `ItemFilter` stops (empty for anything else).
pub fn filter_classes(filter: &str) -> Vec<Hazard> {
    if is_filter(filter) { registered_classes(filter) } else { Vec::new() }
}

/// Whether an item is an `ItemFilter` (20000 wear, stack size 1).
pub fn is_filter(id: &str) -> bool {
    hazmat().filters.iter().any(|f| f == id)
}

/// The `IGasMask`s of the port and the hazards each one never protects
/// against (`getBlacklist`). `ArmorGasMask.getBlacklist`: the mono mask
/// refuses lung gas, blistering gas and bacteria, the others blistering gas;
/// `ItemModGasmask` refuses blistering gas; the full hoods (`ArmorHazmatMask`,
/// `ArmorLiquidatorMask`) have "no restrictions". `None` for no gas mask.
pub fn mask_blacklist(mask: &str) -> Option<&'static [Hazard]> {
    use Hazard::*;
    match mask {
        "ntm:gas_mask_mono" => Some(&[LungGas, BlisteringGas, Bacteria]),
        "ntm:gas_mask" | "ntm:gas_mask_m65" | "ntm:gas_mask_olde" | "ntm:attachment_mask" => Some(&[BlisteringGas]),
        "ntm:hazmat_helmet" | "ntm:hazmat_helmet_red" | "ntm:hazmat_helmet_grey" | "ntm:hazmat_paa_helmet" | "ntm:liquidator_helmet" => Some(&[]),
        _ => None,
    }
}

/// Whether an item is an `IGasMask` that takes a filter.
pub fn is_gas_mask(id: &str) -> bool {
    mask_blacklist(id).is_some()
}

/// The Java's filter-copy bug, reproduced on purpose (coordinator, onda 1):
/// `ArmorUtil.damageGasMaskFilter(ItemStack mask, int)` (ArmorUtil.java:332-352)
/// takes the filter of the `helmet_only` mod when the helmet `IGasMask` has
/// none of its own, wears it, and then `installGasMaskFilter(mask, filter)`
/// writes the worn copy into the *helmet*, while the mod keeps its filter:
/// a sneak-use of the helmet then takes the copy out, a second filter. This
/// is a deliberate integrity divergence from the "no duplication" rule
/// (docs/onda-1-protecao.md); `false` wears the mod's own filter instead.
pub const JAVA_FILTER_COPY_BUG: bool = true;

/// Which filter `ArmorUtil.damageGasMaskFilter(entity, damage)` wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterWear {
    /// The helmet's own filter, written back into the helmet.
    Helmet,
    /// The filter of the `IGasMask` in the helmet's `helmet_only` slot,
    /// written back into that mod.
    Mod,
    /// The mod's filter, worn, written into the helmet (the bug above); the
    /// mod keeps its filter unchanged.
    CopyIntoHelmet,
    /// Nothing: no `IGasMask` or no filter.
    None,
}
/// `damageGasMaskFilter(entity)` picks the helmet if it is an `IGasMask`,
/// else the `IGasMask` mod; `damageGasMaskFilter(mask)` then reads the mask's
/// filter, or, when it has none, the filter of the mask's own mod.
pub fn filter_wear(helmet_is_mask: bool, helmet_filter: bool, mod_is_mask: bool, mod_filter: bool) -> FilterWear {
    if helmet_is_mask {
        if helmet_filter {
            FilterWear::Helmet
        } else if mod_is_mask && mod_filter {
            if JAVA_FILTER_COPY_BUG { FilterWear::CopyIntoHelmet } else { FilterWear::Mod }
        } else {
            FilterWear::None
        }
    } else if mod_is_mask {
        // The mod is the mask now: its own filter (an attachment mask has no mods).
        if mod_filter { FilterWear::Mod } else { FilterWear::None }
    } else if helmet_filter {
        // No IGasMask anywhere: the mask stays the helmet, and a filter tag on it
        // (only a command puts one there) is still read and worn.
        FilterWear::Helmet
    } else {
        FilterWear::None
    }
}

/// A worn helmet (or a mod inside it) as `getProtectionFromItem` reads it:
/// the item, its installed filter and the mods in its `ntm_armor_mods`.
#[derive(Clone, Debug, Default)]
pub struct Worn<'a> {
    pub id: &'a str,
    pub filter: Option<&'a str>,
    pub mods: Vec<Worn<'a>>,
}
/// `ArmorRegistry.getProtectionFromItem`: the item's own classes, the
/// filter's classes minus the mask's blacklist when the item is an
/// `IGasMask`, and the same, recursively, for every mod.
pub fn protection(w: &Worn) -> Vec<Hazard> {
    let mut out = registered_classes(w.id);
    if let (Some(blacklist), Some(filter)) = (mask_blacklist(w.id), w.filter) {
        // `hazardClasses.get(filter)` then `remove` each blacklisted class:
        // only a registered filter item contributes.
        out.extend(registered_classes(filter).into_iter().filter(|c| !blacklist.contains(c)));
    }
    for m in &w.mods {
        out.extend(protection(m));
    }
    out
}
/// `ArmorRegistry.hasProtection(entity, 3, clazz)` (also `hasAllProtection`
/// with one class): false without a helmet.
pub fn has_protection(helmet: Option<&Worn>, hazard: Hazard) -> bool {
    helmet.is_some_and(|w| protection(w).contains(&hazard))
}
/// `ArmorRegistry.hasAnyProtection`.
pub fn has_any_protection(helmet: Option<&Worn>, hazards: &[Hazard]) -> bool {
    helmet.is_some_and(|w| {
        let p = protection(w);
        hazards.iter().any(|h| p.contains(h))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A worn mask with only a filter installed.
    fn filter_protects(mask: &str, filter: &str, hazard: Hazard) -> bool {
        has_protection(Some(&Worn { id: mask, filter: Some(filter), mods: vec![] }), hazard)
    }
    #[test]
    fn life_endpoints_and_invalid_state() {
        assert_eq!(interpolate(20., 4., 100., 100.), Some(20.));
        assert_eq!(interpolate(20., 4., 25., 100.), Some(8.));
        assert_eq!(interpolate(20., 4., -1., 100.), Some(4.));
        assert_eq!(interpolate(20., 4., f64::NAN, 100.), None);
    }
    /// ArmorUtil.register, filter by filter.
    #[test]
    fn each_filter_stops_the_classes_the_java_registers() {
        use Hazard::*;
        let m65 = "ntm:gas_mask_m65";
        assert!(filter_protects(m65, "ntm:gas_mask_filter_mono", CarbonMonoxide));
        assert!(!filter_protects(m65, "ntm:gas_mask_filter_mono", LungGas));
        assert!(filter_protects(m65, "ntm:gas_mask_filter_combo", CarbonMonoxide));
        assert!(filter_protects(m65, "ntm:gas_mask_filter_combo", Bacteria));
        assert!(!filter_protects(m65, "ntm:gas_mask_filter_combo", BlisteringGas), "the M65 blacklists blistering gas");
        assert!(filter_protects(m65, "ntm:gas_mask_filter_rag", CoarseParticles));
        assert!(!filter_protects(m65, "ntm:gas_mask_filter_rag", FineParticles));
        assert!(filter_protects(m65, "ntm:gas_mask_filter_piss", LungGas));
        assert!(!filter_protects(m65, "ntm:gas_mask_filter_piss", FineParticles));
        assert!(!filter_protects(m65, "ntm:gas_mask_filter", CarbonMonoxide));
        assert!(!filter_protects(m65, "ntm:stone", CoarseParticles));
        assert!(is_filter("ntm:gas_mask_filter_rag") && !is_filter("ntm:gas_mask_m65"));
    }

    /// `ArmorHazmatMask implements IGasMask` with an empty blacklist: the
    /// yellow, red and grey hoods take filters and, unlike the M65 and the
    /// attachment mask, let a filter stop blistering gas.
    #[test]
    fn the_hazmat_hoods_are_gas_masks_without_a_blacklist() {
        use Hazard::*;
        for hood in ["ntm:hazmat_helmet", "ntm:hazmat_helmet_red", "ntm:hazmat_helmet_grey"] {
            assert!(is_gas_mask(hood), "{hood}");
            assert!(filter_protects(hood, "ntm:gas_mask_filter", BlisteringGas), "{hood}");
            assert!(filter_protects(hood, "ntm:gas_mask_filter_combo", BlisteringGas), "{hood}");
            assert!(filter_protects(hood, "ntm:gas_mask_filter_rag", CoarseParticles), "{hood}");
            assert!(!filter_protects(hood, "ntm:gas_mask_filter_rag", LungGas), "{hood}");
        }
        for mask in ["ntm:gas_mask_m65", "ntm:attachment_mask"] {
            assert!(is_gas_mask(mask) && !filter_protects(mask, "ntm:gas_mask_filter", BlisteringGas));
        }
        assert!(!is_gas_mask("ntm:hazmat_plate") && !is_gas_mask("ntm:stone"));
        assert!(!filter_protects("ntm:hazmat_plate", "ntm:gas_mask_filter", CoarseParticles));
    }

    #[test]
    fn java_protection_has_distinct_gamma_and_filter_paths() {
        // (float) Math.pow(10, -0.24F): the exponent is the float 0.23999999.
        assert_eq!(dose_factor(["ntm:hazmat_plate"], false), f64::from(0.575_439_93_f32));
        assert_eq!(dose_factor(["ntm:gas_mask_m65"], false), f64::from(0.803_526_1_f32));
        assert!(filter_protects(
            "ntm:gas_mask_m65",
            "ntm:gas_mask_filter",
            Hazard::LungGas
        ));
        assert!(!filter_protects(
            "ntm:gas_mask_m65",
            "ntm:gas_mask_filter",
            Hazard::Gamma
        ));
        assert!(!filter_protects(
            "ntm:gas_mask_m65",
            "ntm:gas_mask_filter",
            Hazard::BlisteringGas
        ));
    }

    /// HazmatRegistry.initDefault, products evaluated in double as the JVM does.
    #[test]
    fn the_table_is_every_registered_piece_of_the_catalog() {
        assert_eq!(resistance("ntm:hazmat_helmet"), 0.6 * 0.2);
        assert_eq!(resistance("ntm:hazmat_plate_grey"), 2.0 * 0.4);
        assert_eq!(resistance("ntm:steel_plate"), 0.045 * 0.4);
        assert_eq!(resistance("ntm:titanium_legs"), 0.045 * 0.3);
        assert_eq!(resistance("ntm:alloy_helmet"), 0.07 * 0.2);
        assert_eq!(resistance("ntm:cobalt_boots"), 0.125 * 0.1);
        assert_eq!(resistance("ntm:starmetal_plate"), 1.0 * 0.4);
        assert_eq!(resistance("minecraft:iron_chestplate"), 0.0225 * 0.4);
        assert_eq!(resistance("minecraft:golden_boots"), 0.0225 * 0.1);
        assert_eq!(resistance("ntm:gas_mask_m65"), 0.095);
        assert_eq!(resistance("minecraft:diamond_helmet"), 0.0);
        assert_eq!(resistance("ntm:attachment_mask"), 0.0);
    }

    /// `res += getResistance(stack)` narrows to float after each piece.
    #[test]
    fn the_player_sum_is_a_float_like_the_java() {
        let grey = ["ntm:hazmat_helmet_grey", "ntm:hazmat_plate_grey", "ntm:hazmat_legs_grey", "ntm:hazmat_boots_grey"];
        assert_eq!(player_resistance(grey.map(resistance), false), 2.0);
        // 10^-2 as a float is 0.00999999977, the float nearest 0.01.
        assert_eq!(radiation_mod(2.0), 0.01_f32);
        let steel = ["ntm:steel_helmet", "ntm:steel_plate", "ntm:steel_legs", "ntm:steel_boots"];
        // The float nearest 0.045 is 0.045000002: four double pieces narrowed one by one land on it.
        assert_eq!(player_resistance(steel.map(resistance), false), 0.045_f32);
        assert_eq!(radiation_mod(0.045), 0.901_571_15_f32);
        let yellow = ["ntm:hazmat_helmet", "ntm:hazmat_plate", "ntm:hazmat_legs", "ntm:hazmat_boots"];
        let r = player_resistance(yellow.map(resistance), true);
        assert_eq!(r, 0.799_999_95_f32);
        assert_eq!(radiation_mod(r), 0.158_489_33_f32);
    }

    /// `getCladding`: a positive `hfr_cladding` wins over the mod slot; the
    /// slot counts only an `ItemModCladding`.
    #[test]
    fn cladding_adds_to_the_piece() {
        let lead = "ntm:cladding_lead";
        let base = Piece { id: "ntm:hazmat_helmet_grey", hfr_cladding: 0.0, cladding_mod: None };
        assert_eq!(piece_resistance(&base), 0.4);
        assert_eq!(piece_resistance(&Piece { cladding_mod: Some(lead), ..base }), 0.4 + 0.1);
        assert_eq!(piece_resistance(&Piece { hfr_cladding: 0.25, cladding_mod: Some(lead), ..base }), 0.4 + 0.25);
        assert_eq!(piece_resistance(&Piece { cladding_mod: Some("ntm:attachment_mask"), ..base }), 0.4);
        assert_eq!(piece_resistance(&Piece { hfr_cladding: -1.0, ..base }), 0.4);
        // Cladding counts on a piece the registry does not know.
        assert_eq!(piece_resistance(&Piece { id: "minecraft:diamond_helmet", hfr_cladding: 0.5, cladding_mod: None }), 0.5);
    }

    #[test]
    fn mods_go_in_their_slot_on_the_armor_types_they_name() {
        assert!(mod_fits(Some(0), 0, "ntm:attachment_mask"));
        assert!(!mod_fits(Some(1), 0, "ntm:attachment_mask"));
        assert!(!mod_fits(Some(0), 5, "ntm:attachment_mask"));
        assert!(!mod_fits(None, 0, "ntm:attachment_mask"));
        assert!(!mod_fits(Some(0), 0, "ntm:stone"));
        for c in ["ntm:cladding_paint", "ntm:cladding_rubber", "ntm:cladding_lead", "ntm:cladding_desh", "ntm:cladding_ghiorsium"] {
            assert!(cladding_rad(c).is_some(), "{c}");
            for t in 0..4 {
                // ArmorModHandler.cladding = 5.
                assert!(mod_fits(Some(t), 5, c), "{c} {t}");
                assert!(!mod_fits(Some(t), 0, c), "{c} {t}");
            }
        }
    }

    #[test]
    fn tooltips_print_the_java_numbers() {
        assert_eq!(resistance_line(0.6 * 0.2).as_deref(), Some("§eRadiation resistance: 0.12"));
        assert_eq!(resistance_line(0.0225 * 0.1).as_deref(), Some("§eRadiation resistance: 0.002"));
        assert_eq!(resistance_line(0.0009), None);
        assert_eq!(resistance_line(0.0), None);
        assert_eq!(resistance_line(2.0 * 0.4 + 0.5).as_deref(), Some("§eRadiation resistance: 1.3"));
        assert_eq!(cladding_lines("ntm:cladding_rubber").unwrap()[0], "§e+0.005 rad-resistance");
        assert_eq!(cladding_lines("ntm:cladding_ghiorsium").unwrap()[0], "§e+0.5 rad-resistance");
        assert!(cladding_lines("ntm:stone").is_none());
    }

    /// ArmorRegistry.getProtectionFromItem: own classes, filter minus blacklist, mods.
    #[test]
    fn protection_joins_the_item_its_filter_and_its_mods() {
        use Hazard::*;
        let bare = |id| Worn { id, filter: None, mods: vec![] };
        // ArmorUtil.java:46 and :57: the M65 and the hood stop sand by themselves.
        assert!(has_protection(Some(&bare("ntm:gas_mask_m65")), Sand));
        assert!(!has_protection(Some(&bare("ntm:gas_mask_m65")), Light));
        assert!(has_protection(Some(&bare("ntm:hazmat_helmet")), Sand));
        assert!(!has_protection(None, Sand));
        // No filter: no gas protection.
        assert!(!has_protection(Some(&bare("ntm:gas_mask_m65")), LungGas));
        // A filter on a helmet that is no IGasMask counts for nothing.
        assert!(!has_protection(Some(&Worn { id: "ntm:steel_helmet", filter: Some("ntm:gas_mask_filter"), mods: vec![] }), LungGas));
        // The attachment mask inside a plain helmet: its SAND and its filter, minus blistering gas.
        let mask = Worn { id: "ntm:attachment_mask", filter: Some("ntm:gas_mask_filter_combo"), mods: vec![] };
        let helmet = Worn { id: "ntm:steel_helmet", filter: None, mods: vec![mask] };
        let p = protection(&helmet);
        assert!(p.contains(&Sand) && p.contains(&CarbonMonoxide) && p.contains(&Bacteria) && !p.contains(&BlisteringGas));
        assert!(has_any_protection(Some(&helmet), &[Light, Sand]));
        assert!(!has_any_protection(Some(&helmet), &[Light, InertGas]));
        assert!(registered_classes("ntm:gas_mask_filter_combo").len() == 6 && is_filter("ntm:gas_mask_filter_combo"));
        assert!(filter_classes("ntm:gas_mask_m65").is_empty() && !is_filter("ntm:gas_mask_m65"));
    }

    /// ArmorUtil.damageGasMaskFilter, both overloads, with the copy bug.
    #[test]
    fn the_filter_worn_is_the_javas() {
        use FilterWear::*;
        assert_eq!(filter_wear(true, true, true, true), Helmet);
        assert_eq!(filter_wear(true, true, false, false), Helmet);
        // ArmorUtil.java:335-341 then :347-351: the mod's filter, worn, lands in the helmet.
        const { assert!(JAVA_FILTER_COPY_BUG) };
        assert_eq!(filter_wear(true, false, true, true), CopyIntoHelmet);
        assert_eq!(filter_wear(true, false, true, false), None);
        assert_eq!(filter_wear(true, false, false, false), None);
        // ArmorUtil.java:317-326: a plain helmet hands over to its IGasMask mod.
        assert_eq!(filter_wear(false, false, true, true), Mod);
        assert_eq!(filter_wear(false, false, true, false), None);
        assert_eq!(filter_wear(false, false, false, false), None);
        assert_eq!(filter_wear(false, true, true, false), None);
        assert_eq!(filter_wear(false, true, false, false), Helmet);
    }

    /// ArmorGasMask.getBlacklist (:148-156), ArmorHazmatMask and ArmorLiquidatorMask (no restriction).
    #[test]
    fn the_new_masks_follow_their_java_blacklists() {
        use Hazard::*;
        let combo = "ntm:gas_mask_filter_combo";
        for refused in [LungGas, BlisteringGas, Bacteria] {
            assert!(!filter_protects("ntm:gas_mask_mono", combo, refused), "{refused:?}");
        }
        assert!(filter_protects("ntm:gas_mask_mono", combo, CarbonMonoxide));
        assert!(filter_protects("ntm:gas_mask_mono", combo, FineParticles));
        for mask in ["ntm:gas_mask", "ntm:gas_mask_olde"] {
            assert!(filter_protects(mask, combo, LungGas) && filter_protects(mask, combo, Bacteria), "{mask}");
            assert!(!filter_protects(mask, combo, BlisteringGas), "{mask}");
        }
        for hood in ["ntm:hazmat_paa_helmet", "ntm:liquidator_helmet"] {
            assert!(filter_protects(hood, combo, BlisteringGas), "{hood}");
            // ArmorUtil.java:60-61: light and sand without any filter.
            let bare = Worn { id: hood, filter: None, mods: vec![] };
            assert!(has_protection(Some(&bare), Light) && has_protection(Some(&bare), Sand), "{hood}");
        }
        // ArmorUtil.java:47-48: the rag masks stop by themselves, and are no IGasMask.
        let rag = Worn { id: "ntm:mask_rag", filter: None, mods: vec![] };
        assert!(has_protection(Some(&rag), CoarseParticles) && !has_protection(Some(&rag), LungGas));
        assert!(!is_gas_mask("ntm:mask_rag") && !is_gas_mask("ntm:goggles"));
        let piss = Worn { id: "ntm:mask_piss", filter: None, mods: vec![] };
        assert!(has_protection(Some(&piss), LungGas));
    }

}
