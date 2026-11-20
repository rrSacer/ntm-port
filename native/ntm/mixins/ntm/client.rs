// What the player's client shows of the NTM (L4): the Geiger box and the
// radioactive items' tooltip lines. What they say is ntm_core::client_rad
// (tested there); the numbers are the server's, per player.
use horizon_client::{extras::Corner, extras_hud::Corners, hud::HudBatch, items::ItemStack, text::Text};
use ntm_core::client_rad;

/// The Geiger counter: a box in the top left with the rate and the dose, in
/// the colour of the dose, once the server has sent one.
pub(super) fn hud(game: &mut crate::game::Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    look_overlay(game, batch);
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

/// `ILookOverlay.printHook` of the block in the crosshair (`ModEventHandlerClient:205-221`): `printGeneric`'s title
/// and lines right of the crosshair (`ntm_core::look_overlay`).
fn look_overlay(game: &mut crate::game::Game<'_>, batch: &mut HudBatch) {
    use ntm_core::look_overlay as lo;
    let Some(hit) = game.native_raycast(lo::reach(game.creative)) else { return };
    let Some(o) = lo::of_block(&hit.block) else { return };
    let (shadow, title, lines) = lo::layout(batch.width, batch.height, o.lines.len());
    game.native_text(batch, &o.title, shadow, o.background, false);
    game.native_text(batch, &o.title, title, o.title_color, false);
    for (line, at) in o.lines.iter().zip(lines) {
        let (color, text) = lo::line_color(line);
        game.native_text(batch, text, at, color, true);
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
    if let Some(line)=crate::game::ntm_items::lens_tooltip(stack) {
        lines.push((Text::literal(line),u32::MAX));
    }
    for line in ntm_core::item_hooks::cladding_lines(&stack.item).unwrap_or_default() {
        lines.push((Text::legacy(&line),u32::MAX));
    }
    if let Some(line)=ntm_core::item_hooks::resistance_line(crate::game::ntm_items::worn_resistance(stack)) {
        lines.push((Text::legacy(&line),u32::MAX));
    }
    crate::game::ntm_armor_table::mods_tooltip(_game, stack, lines);
    let native=crate::game::ntm_glue::stack(&Some(stack.clone()));
    // An autocrafter's template copy: its filter mode (`GUIAutocrafter.drawScreen`).
    for line in native.as_ref().map(ntm_core::autocrafter::template_lines).unwrap_or_default() {
        lines.push((Text::legacy(&line),u32::MAX));
    }
    // An RBMK rod (`ItemRBMKRod.addInformation`): its state read from the stack's tag, the Java's keys translated here.
    if let Some(rod)=ntm_core::rbmk::persist::rod_of(&stack.item) {
        let mut state=native.as_ref().map(ntm_core::rbmk::persist::stack_rod).unwrap_or_default();
        for line in ntm_core::rbmk::tooltip::lines(rod,&mut state) {
            let args:Vec<Text>=line.args.iter().map(|a|match a.strip_prefix("§key:") { Some(k)=>Text::translate(k,&[]), None=>Text::legacy(a) }).collect();
            // `EnumChatFormatting.X + I18nUtil.resolveKey(..)`: the line's codes style the whole translated line (a
            // code alone in its own component would style nothing).
            let text=match &line.key {
                Some(k)=>Text::translate(k,&args).with_style(&legacy_style(line.style)),
                None=>Text::legacy(&format!("{}{}",line.style,line.args[0])),
            };
            lines.push((text,u32::MAX));
        }
    }
    if let Some(p)=ntm_core::radioisotopes::pellet(&stack.item) {
        let life=native.as_ref().map_or(p.lifespan_ticks,ntm_core::radioisotopes::life);
        lines.push((Text::legacy(&format!("Calor: {} TU/t",p.heat_tu.decay_enabled)),u32::MAX));
        lines.push((Text::legacy(&format!("Vida restante: {:.3}% ({life} / {} ticks)",life as f64/p.lifespan_ticks as f64*100.,p.lifespan_ticks)),u32::MAX));
        lines.push((Text::legacy(&format!("Decai para: {}",p.decay_item)),u32::MAX));
    }
    let rate=native.as_ref().and_then(|s|ntm_core::zirnox::radiation_rate(s).or_else(||ntm_core::radioisotopes::radiation_rate(s)));
    // An RBMK rod's rate as `HazardTypeRadiation` prints its float level (truncated, never past a u32).
    let rbmk=native.as_ref().and_then(ntm_core::rbmk::hazards::radiation_level);
    let hazard_lines=match (rate,rbmk) {
        (_,Some(level))=>ntm_core::rbmk::hazards::radiation_tooltip(level,stack.count),
        (Some(rate),None)=>client_rad::tooltip_rate_lines(rate,stack.count),
        (None,None)=>client_rad::tooltip_lines(&stack.item,stack.count),
    };
    for line in hazard_lines {
        lines.push((Text::legacy(&line), u32::MAX));
    }
    // `HazardTypeHot.addHazardInformation`, after the radiation as the rod registers them: gold, while the hull is hot.
    if native.as_ref().and_then(ntm_core::rbmk::hazards::hot_level).is_some_and(|l|l>0) {
        lines.push((Text::literal("[").append(Text::translate("trait.hot",&[])).append(Text::literal("]")).with_style(&legacy_style("\u{a7}6")),u32::MAX));
    }
}

/// The style a run of legacy codes (`§9`, `§o`) leaves, as `StringDecomposer.iterateFormatted` applies them.
fn legacy_style(codes: &str) -> horizon_client::text::Style {
    let reset = horizon_client::text::Style::default();
    let mut style = reset.clone();
    let mut chars = codes.chars();
    while let Some(c) = chars.next() {
        if c == '\u{a7}' && let Some(code) = chars.next() {
            style = horizon_client::text::legacy_format(&style, code, &reset);
        }
    }
    style
}
