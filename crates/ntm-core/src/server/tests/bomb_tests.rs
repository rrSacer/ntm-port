//! Bombs in the fake world (docs/bombas.md): assembly through item
//! transactions, redstone, fuses, the detonator, the explosions, their
//! damage, radiation and settings.
use super::*;
use crate::bomb::{self, actions as bomb_actions};

/// A stone floor y 60..=69 over `r` blocks around the origin.
fn floor(g: &mut Game, r: i32) {
    let mut w = g.world.borrow_mut();
    for x in -r..=r {
        for z in -r..=r {
            for y in 60..70 {
                w.set(OW, [x, y, z], "minecraft:stone");
            }
        }
    }
}

/// A game with a stone floor and nukes cut to radius `max_r`.
fn bomb_game(max_r: i32) -> Game {
    let mut g = game();
    floor(&mut g, 24);
    let who = player();
    let out = g.server.bomb_command(&mut g.sh, Some(&who), &format!("config mundo raiomax {max_r}"));
    assert!(out.is_ok(), "{out:?}");
    g.run(2);
    g
}

fn place_bomb(g: &mut Game, at: Pos, id: &str) {
    g.place(at, &format!("{id}[lit=false]"));
    g.run(2);
}

fn power(g: &mut Game, at: Pos, id: &str) {
    g.world.borrow_mut().set(OW, at, &format!("{id}[lit=true]"));
}

/// Puts components `range` of the bomb at `at` in from the hand.
fn assemble(g: &mut Game, at: Pos, id: &str, range: std::ops::Range<usize>) {
    g.open(at);
    for s in &bomb::bomb_type(id).unwrap().slots[range] {
        g.hold(s.item, 1);
        g.click(bomb_actions::INSERT);
        for _ in 0..200 {
            if g.held().is_none() && g.server.txs().is_empty() {
                break;
            }
            g.run(1);
        }
        assert_eq!(g.held(), None, "{} not taken: {:?}", s.item, g.ch.chat);
    }
}

fn bomb_data(g: &mut Game, at: Pos) -> Option<bomb::Bomb> {
    match g.ready().world.machines.get(&(OW.to_string(), at)).map(|m| m.data.clone()) {
        Some(Data::Bomb(b)) => Some(b),
        _ => None,
    }
}

fn ran(g: &Game, cmd: &str) -> usize {
    g.world.borrow().ran.iter().filter(|l| l.contains(cmd)).count()
}

/// Runs until no explosion is active (or `limit` ticks).
fn until_quiet(g: &mut Game, limit: usize) {
    for _ in 0..limit {
        g.run(1);
        if g.server.blasts.nukes.is_empty() {
            return;
        }
    }
    panic!("explosão não terminou: {:?}", g.server.blasts.nukes.active().map(|p| p.line()).collect::<Vec<_>>());
}

#[test]
fn bomb_lit_matches_the_registry() {
    assert!(super::super::bomb_server::BOMB_LIT_FOR_TESTS.starts_with(registry::BOMB_POWERED));
}

#[test]
fn a_little_boy_assembled_from_the_hand_goes_off_by_redstone() {
    let mut g = bomb_game(10);
    g.players[0].creative = false;
    g.world.borrow_mut().gamemode = "survival".into();
    let at = [3, 70, 3];
    let id = "ntm:nuke_boy";
    place_bomb(&mut g, at, id);
    assemble(&mut g, at, id, 0..4);
    assert!(bomb_data(&mut g, at).unwrap().blast().is_none());
    // Not ready: redstone does nothing.
    power(&mut g, at, id);
    g.run(3);
    assert!(bomb_data(&mut g, at).is_some());
    assert!(g.screen_text().contains("Faltam componentes"), "{}", g.screen_text());
    assert!(g.screen_text().contains("Ignitor: vazio"), "{}", g.screen_text());
    g.world.borrow_mut().set(OW, at, &format!("{id}[lit=false]"));
    g.run(2);
    assemble(&mut g, at, id, 4..5);
    g.run(6);
    // Four already in: the fifth fills the last slot.
    let b = bomb_data(&mut g, at).unwrap();
    assert_eq!(b.slots.iter().flatten().count(), 5, "{b:?}");
    assert!(g.screen_text().contains("Pronta (raio 120)"), "{}", g.screen_text());
    // Redstone: the rising edge sets it off.
    power(&mut g, at, id);
    g.run(2);
    assert!(bomb_data(&mut g, at).is_none(), "a bomba continua");
    assert_eq!(g.world.borrow().get(OW, at), "minecraft:air");
    assert!(ran(&g, "playsound") > 0 && ran(&g, "particle") > 0, "sem efeitos");
    until_quiet(&mut g, 2000);
    // A crater under it (stone removed), sellafield in it, damage and dose.
    let w = g.world.borrow();
    let dug = (60..70).filter(|&y| w.get(OW, [3, y, 3]) == "minecraft:air").count();
    // R=10 em pedra: o modelo do NTM cava pouco (custo 6^(0,5+3,5i/R)).
    assert!(dug >= 2, "cratera rasa: {dug}");
    assert!(w.blocks.values().any(|s| s == "ntm:sellafield_slaked"));
    assert!(w.ran.iter().any(|l| l.contains("damage Steve")), "sem dano");
    drop(w);
    let dose = g.ready().world.radiation.get("p1").map_or(0, |d| d.millirad);
    assert!(dose > 0, "sem dose");
    assert_eq!(g.ready().world.bombs.zones.len(), 1);
    assert!(g.server.area_rate("p1") > 0);
    // A zona soma dose a cada segundo, sem depender da amostra do cliente.
    // (Passada a carência de entrada; o pulso já deu o teto de 2.500 RAD:
    // zera para medir.)
    g.run(crate::radiation::GRACE_TICKS as usize);
    g.ready().world.radiation.insert("p1".into(), Default::default());
    let before = 0;
    g.run(40);
    let after = g.ready().world.radiation.get("p1").map_or(0, |d| d.millirad);
    assert!(after > before, "a zona não somou dose: {before} → {after}");
    // Criativo: nenhuma radiação de área.
    g.players[0].creative = true;
    g.run(1);
    assert_eq!(g.server.area_rate("p1"), 0);
    g.players[0].creative = false;
    // The components are spent: nothing came back.
    assert_eq!(g.inv("ntm:boy_igniter"), 0);
    // Nothing of the explosion left in the folder.
    assert!(g.kv.borrow().map.keys().all(|k| !k.starts_with("nx.")), "{:?}", g.kv.borrow().map.keys());
}

#[test]
fn a_charge_burns_its_fuse_then_digs_like_vanilla() {
    let mut g = bomb_game(10);
    let at = [0, 70, 0];
    let id = "ntm:dynamite";
    place_bomb(&mut g, at, id);
    power(&mut g, at, id);
    g.run(2);
    let fuse = bomb_data(&mut g, at).and_then(|b| b.fuse);
    assert!(fuse.is_some_and(|f| f > 60), "{fuse:?}");
    assert!(ran(&g, "entity.tnt.primed") > 0);
    g.run(bomb::FUSE_TICKS as usize + 4);
    assert!(bomb_data(&mut g, at).is_none());
    let w = g.world.borrow();
    assert_eq!(w.get(OW, [0, 69, 0]), "minecraft:air");
    assert_eq!(w.get(OW, [0, 60, 0]), "minecraft:stone", "fundo da carga");
    assert!(g.server.blasts.nukes.is_empty());
}

#[test]
fn the_detonator_links_and_sets_off_a_ready_bomb() {
    let mut g = bomb_game(8);
    let at = [4, 70, 0];
    let id = "ntm:nuke_n2";
    place_bomb(&mut g, at, id);
    // Unlinked: says so.
    g.client.detonator(&mut g.ch, OW, None, false);
    g.run(3);
    assert!(g.ch.chat.last().is_some_and(|c| c.contains("sem bomba")), "{:?}", g.ch.chat);
    g.client.detonator(&mut g.ch, OW, Some(at), true);
    g.run(3);
    assert!(g.ch.chat.last().is_some_and(|c| c.contains("Detonador ligado")), "{:?}", g.ch.chat);
    g.client.detonator(&mut g.ch, OW, None, false);
    g.run(3);
    assert!(g.ch.chat.last().is_some_and(|c| c.contains("não está pronta")), "{:?}", g.ch.chat);
    assemble(&mut g, at, id, 0..12);
    g.client.detonator(&mut g.ch, OW, None, false);
    g.run(3);
    assert!(bomb_data(&mut g, at).is_none());
    until_quiet(&mut g, 2000);
    // N2: no fallout, no slag, no radiation.
    let w = g.world.borrow();
    assert!(!w.blocks.values().any(|s| s.starts_with("ntm:sellafield") || s.starts_with("ntm:fallout")));
    drop(w);
    assert!(g.ready().world.bombs.zones.is_empty());
}

#[test]
fn breaking_a_bomb_gives_its_components_back_once() {
    let mut g = bomb_game(8);
    let at = [2, 70, 0];
    let id = "ntm:nuke_man";
    place_bomb(&mut g, at, id);
    assemble(&mut g, at, id, 0..2);
    g.world.borrow_mut().set(OW, at, "minecraft:air");
    g.server.block_broken(OW, at, &format!("{id}[lit=false]"));
    g.run(40);
    assert_eq!(g.world.borrow().dropped.len(), 2, "{:?}", g.world.borrow().dropped);
}

#[test]
fn an_explosion_resumes_after_a_restart() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = Game::new(w.clone(), kv.clone(), 1);
    g.run(10);
    floor(&mut g, 24);
    let who = player();
    g.server.bomb_command(&mut g.sh, Some(&who), "config mundo raiomax 14").unwrap();
    g.server.bomb_command(&mut g.sh, Some(&who), "teste 14").unwrap();
    g.run(2);
    assert!(!g.server.blasts.nukes.is_empty());
    assert!(kv.borrow().map.keys().any(|k| k.starts_with("nx.")), "sem diário");
    drop(g);
    // Back in the same world: the explosion (in the fallout already) comes
    // back from the folder and finishes.
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    let mut resumed = false;
    for _ in 0..200 {
        g.run(1);
        resumed |= !g.server.blasts.nukes.is_empty();
        if resumed && g.server.blasts.nukes.is_empty() {
            break;
        }
    }
    assert!(resumed, "não retomou");
    until_quiet(&mut g, 3000);
    assert!((60..70).filter(|&y| w.borrow().get(OW, [0, y, 0]) == "minecraft:air").count() >= 3);
    assert!(kv.borrow().map.keys().all(|k| !k.starts_with("nx.")));
}

#[test]
fn settings_are_global_or_per_world_and_enhanced_falls_back() {
    let mut g = bomb_game(8);
    let who = player();
    let s = g.server.bomb_command(&mut g.sh, Some(&who), "config").unwrap();
    assert!(s.contains("raiomax 8") && s.contains("do mundo"), "{s}");
    let s = g.server.bomb_command(&mut g.sh, Some(&who), "config global modo aprimorado").unwrap();
    assert!(s.contains("modo aprimorado"), "{s}");
    // The world's own settings still win.
    assert!(g.server.bomb_config(&mut g.sh).mode == crate::blast::Mode::Otimizado);
    g.server.bomb_command(&mut g.sh, Some(&who), "config mundo limpar").unwrap();
    assert!(g.server.bomb_config(&mut g.sh).mode == crate::blast::Mode::Aprimorado);
    assert!(g.server.bomb_command(&mut g.sh, Some(&who), "config mundo efeitos talvez").is_err());
    // Enhanced isn't available yet: a blast still shows the optimized effects.
    g.server.bomb_command(&mut g.sh, Some(&who), "config global raiomax 6").unwrap();
    g.server.bomb_command(&mut g.sh, Some(&who), "teste 6").unwrap();
    g.run(3);
    assert!(ran(&g, "explosion_emitter") > 0);
    assert!(g.server.blasts.warned_fallback);
    until_quiet(&mut g, 2000);
    // The global settings live in the folder; a new server reads them.
    assert!(g.kv.borrow().map.contains_key(crate::blast::config::KEY));
}
