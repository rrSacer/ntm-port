// What the player's client shows of the NTM (L4): the Geiger box and the
// radioactive items' tooltip lines. What they say is ntm_core::client_rad
// (tested there); the numbers are the server's, per player.
use horizon_client::{extras::Corner, extras_hud::Corners, hud::HudBatch, items::ItemStack, text::Text};
use ntm_core::client_rad;

/// The Geiger counter: a box in the top left with the rate and the dose, in
/// the colour of the dose, once the server has sent one.
pub(super) fn hud(game: &mut crate::game::Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    if !game.native_corner_shown(Corner::TopLeft) {
        return;
    }
    let dose = game.native_player_value(client_rad::KEY_DOSE);
    let rate = game.native_player_value(client_rad::KEY_RATE);
    let Some((lines, color)) = client_rad::geiger(dose, rate) else { return };
    let width = lines.iter().map(|l| game.native_text_width(l)).fold(0.0, f32::max) + 6.0;
    let height = lines.len() as f32 * 10.0 + 4.0;
    let (x, y) = corners.place(Corner::TopLeft, width, height);
    game.native_fill(batch, [x, y, width, height], 0x7800_0000);
    for (i, line) in lines.iter().enumerate() {
        game.native_text(batch, line, [x + 3.0, y + 2.0 + i as f32 * 10.0], color, true);
    }
}

/// A radioactive item's lines, after the others.
pub(super) fn tooltip(_game: &crate::game::Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let depletion = crate::game::ntm_glue::pile_depletion(stack.data().and_then(|d|d.other("minecraft:custom_data")));
    if let Some(rod) = ntm_core::pile::Rod::of(&stack.item,depletion) {
        let spec = ntm_core::pile::RODS[rod.kind as usize];
        if spec.life > 0.0 {
            lines.push((Text::legacy(&format!("Lifetime: {:.0}",spec.life)),u32::MAX));
            if rod.depletion > 0.0 { lines.push((Text::legacy(&format!("Depletion: {:.0}%",rod.depletion/spec.life*100.0)),u32::MAX)); }
        }
    }
    if let Some(fuel)=ntm_core::zirnox::fuel(&stack.item) {
        let life=crate::game::ntm_glue::stack(&Some(stack.clone())).map_or(0,|s|ntm_core::zirnox::life(&s));
        let percent=(f64::from(life)/f64::from(fuel.max_life)*100000.).floor()/1000.;
        lines.push((Text::legacy(&format!("§eConsumido: {percent:.3}% ({life} / {})",fuel.max_life)),u32::MAX));
        lines.push((Text::legacy(if fuel.breeding {"Breeding: exige barras ativas vizinhas"}else{"Reage com a própria barra e as vizinhas ativas"}),u32::MAX));
        if !fuel.breeding {lines.push((Text::legacy(&format!("Calor por decaimento: {}",fuel.heat_per_decay)),u32::MAX));}
    }
    if &*stack.item=="ntm:ams_lens" {
        let damage=match stack.data().and_then(|d|d.other("minecraft:custom_data")) {
            Some(horizon_world::nbt::Tag::Compound(c))=>match c.get(&horizon_world::nbt::JavaString::from("damage")) {Some(horizon_world::nbt::Tag::Long(n))=>*n,_=>0},
            _=>0,
        };
        lines.push((Text::literal(ntm_core::consumables::lens_line(ntm_core::consumables::AMS_LENS_MAX,damage)),u32::MAX));
    }
    let native=crate::game::ntm_glue::stack(&Some(stack.clone()));
    if let Some(p)=ntm_core::radioisotopes::pellet(&stack.item) {
        let life=native.as_ref().map_or(p.lifespan_ticks,ntm_core::radioisotopes::life);
        lines.push((Text::legacy(&format!("Calor: {} TU/t",p.heat_tu.decay_enabled)),u32::MAX));
        lines.push((Text::legacy(&format!("Vida restante: {:.3}% ({life} / {} ticks)",life as f64/p.lifespan_ticks as f64*100.,p.lifespan_ticks)),u32::MAX));
        lines.push((Text::legacy(&format!("Decai para: {}",p.decay_item)),u32::MAX));
    }
    let rate=native.as_ref().and_then(|s|ntm_core::zirnox::radiation_rate(s).or_else(||ntm_core::radioisotopes::radiation_rate(s)));
    let hazard_lines=rate.map_or_else(||client_rad::tooltip_lines(&stack.item,stack.count),|rate|client_rad::tooltip_rate_lines(rate,stack.count));
    for line in hazard_lines {
        lines.push((Text::legacy(&line), u32::MAX));
    }
}
