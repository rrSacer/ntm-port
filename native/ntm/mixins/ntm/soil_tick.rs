// WasteEarth and BlockSellafield use the authoritative random-tick level already in place.
fn ntm_soil_tick(
    level: &mut crate::game::GameLevel<'_>,
    ticks: &mut horizon_world::level::Ticks,
    dimension: horizon_world::level::Dimension,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    handled: &mut bool,
) {
    if !level.native_authoritative { return; }
    let name = &*level.facts.get(state).name;
    // The field's sources (slice 4d): sellafield's rad before it decays, a radioactive ore's random tick.
    {
        let name = name.to_string();
        crate::game::ntm_glue::field_random_tick(level, ticks, crate::game::ntm_glue::level_dim_id(dimension), [pos.x, pos.y, pos.z], &name);
    }
    let name = &*level.facts.get(state).name;
    if name == "ntm:sellafield" || name.starts_with("ntm:sellafield__") {
        // BlockSellafield.updateTick (Onda 1, slice 5d): down a level, or slaked from level 0, at the Java's chance.
        // The mods have no random source: the cell and `Level.randValue` (it moves with every random tick drawn).
        *handled = true;
        let cell = ntm_core::hash::fnv64(&[pos.x.to_le_bytes(), pos.y.to_le_bytes(), pos.z.to_le_bytes()].concat());
        let seed = cell ^ u64::from(ticks.random_tick_value as u32).rotate_left(32);
        if let Some(next) = ntm_core::walking::sellafield_decay(name, seed)
            && let Some(state) = level.registry.default_state(&next)
        {
            let behaviours = level.behaviours;
            behaviours.set_block(level, ticks, pos, state, 3);
        }
        return;
    }
    if !matches!(name, "ntm:waste_earth" | "ntm:waste_mycelium") { return; }
    *handled = true;
    let above = [pos.x, pos.y + 1, pos.z];
    // Same loaded-world light source as native_light; this hook holds a
    // GameLevel, and its darken value belongs to this dimension, not Game.dim.
    if !level.world.loaded(above[0], above[2]) { return; }
    let raw = level.world.light(above[0], above[1], above[2]);
    let light = (raw & 15).max((raw >> 4).saturating_sub(level.darken.clamp(0,15) as u8));
    let opacity = level.facts.get(level.world.block(above[0], above[1], above[2])).dampening;
    if ntm_core::zirnox::soil_decays(name, light, opacity)
        && let Some(dirt) = level.registry.default_state("minecraft:dirt")
    {
        let behaviours = level.behaviours;
        behaviours.set_block(level, ticks, pos, dirt, 3);
    }
}
