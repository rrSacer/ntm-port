//! Machine upgrades (`items/machine/ItemMachineUpgrade.java`,
//! `inventory/UpgradeManagerNT.java`): the items a machine's upgrade slot
//! takes (`SlotUpgrade`) and the level a machine reads from them.

use crate::server::container_bridge::Stack;

/// Every `ItemMachineUpgrade` (`ModItems.java:3736-3763`); the slot takes
/// any of them, a machine reads the ones it lists as valid.
pub const ITEMS: &[&str] = &[
    "ntm:upgrade_speed_1",
    "ntm:upgrade_speed_2",
    "ntm:upgrade_speed_3",
    "ntm:upgrade_effect_1",
    "ntm:upgrade_effect_2",
    "ntm:upgrade_effect_3",
    "ntm:upgrade_power_1",
    "ntm:upgrade_power_2",
    "ntm:upgrade_power_3",
    "ntm:upgrade_fortune_1",
    "ntm:upgrade_fortune_2",
    "ntm:upgrade_fortune_3",
    "ntm:upgrade_afterburn_1",
    "ntm:upgrade_afterburn_2",
    "ntm:upgrade_afterburn_3",
    "ntm:upgrade_overdrive_1",
    "ntm:upgrade_overdrive_2",
    "ntm:upgrade_overdrive_3",
    "ntm:upgrade_radius",
    "ntm:upgrade_health",
    "ntm:upgrade_smelter",
    "ntm:upgrade_shredder",
    "ntm:upgrade_centrifuge",
    "ntm:upgrade_crystallizer",
    "ntm:upgrade_nullifier",
    "ntm:upgrade_screm",
    "ntm:upgrade_gc_speed",
    "ntm:upgrade_5g",
];

/// Whether `item` is a machine upgrade (`instanceof ItemMachineUpgrade`).
pub fn is_upgrade(item: &str) -> bool {
    ITEMS.contains(&item)
}

/// `UpgradeManagerNT.getLevel(kind)`: the tiers of the `kind` upgrades in
/// `slots` added, up to `max` (`getValidUpgrades`). `kind` is `speed`,
/// `power`...: the tiered, non-mutex types.
pub fn level(slots: &[Option<Stack>], kind: &str, max: u32) -> u32 {
    let prefix = format!("ntm:upgrade_{kind}_");
    let total: u32 = slots
        .iter()
        .flatten()
        .filter_map(|s| s.id.strip_prefix(&prefix)?.parse::<u32>().ok())
        .sum();
    total.min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_add_up_to_the_machines_maximum() {
        let s = |id: &str| Some(Stack::new(id, 1));
        assert_eq!(level(&[s("ntm:upgrade_speed_1")], "speed", 3), 1);
        assert_eq!(
            level(
                &[s("ntm:upgrade_speed_2"), s("ntm:upgrade_speed_3")],
                "speed",
                3
            ),
            3
        );
        assert_eq!(level(&[s("ntm:upgrade_power_3")], "speed", 3), 0);
        assert_eq!(level(&[None, s("ntm:upgrade_speed_3")], "speed", 2), 2);
        assert_eq!(level(&[], "speed", 3), 0);
    }

    #[test]
    fn the_slot_takes_machine_upgrades_only() {
        assert!(is_upgrade("ntm:upgrade_speed_1") && is_upgrade("ntm:upgrade_5g"));
        assert!(!is_upgrade("ntm:upgrade_template") && !is_upgrade("minecraft:stone"));
    }
}
