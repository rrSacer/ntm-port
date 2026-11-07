//! The second review of the bombs (M2): each test failed before its fix
//! (docs/bombas.md, "Revisão").
use super::bomb_tests::{assemble, bomb_data as bomb_at, floor, place_bomb, power, set_creative};
use super::*;
use crate::bomb;

fn bomb_game_on(world: Rc<RefCell<World>>, max_r: i32) -> Game {
    let mut g = Game::new(world);
    g.run(10);
    floor(&mut g, 24);
    let who = player();
    g.server.bomb_command(&mut g.sh, Some(&who), &format!("config mundo raiomax {max_r}")).unwrap();
    g.run(2);
    g
}

fn bomb_game(max_r: i32) -> Game {
    bomb_game_on(Rc::new(RefCell::new(World::new())), max_r)
}

fn until_quiet(g: &mut Game, limit: usize) {
    for _ in 0..limit {
        g.run(1);
        if g.server.blasts.nukes.is_empty() && g.ready().world.bombs.queue.is_empty() {
            return;
        }
    }
    panic!("explosão não terminou");
}

fn dug(w: &World, at: Pos) -> usize {
    (60..70).filter(|&y| w.get(OW, [at[0], y, at[2]]) == "minecraft:air").count()
}

/// P1-2: three C4 charges whose fuses end in the same tick: each vanilla
/// explosion costs ~12 M of fuel, so they go off over several ticks (a
/// tick's budget is 22 M), and all of them do.
#[test]
fn p1_2_charges_ending_together_go_off_within_the_tick_budget() {
    let mut g = bomb_game(8);
    let at = [[-12, 70, 0], [0, 70, 0], [12, 70, 0]];
    for p in at {
        place_bomb(&mut g, p, "ntm:c4");
    }
    for p in at {
        power(&mut g, p, true);
    }
    // Explosions per tick: what `/ntmbomba status` logs of each.
    let mut worst = 0;
    let mut fired = 0;
    for _ in 0..bomb::FUSE_TICKS as usize + 40 {
        g.run(1);
        let now = g.server.blasts.log.len();
        worst = worst.max(now - fired);
        fired = now;
    }
    assert_eq!(fired, at.len(), "nem todas explodiram");
    assert!(worst <= 1, "{worst} cargas de força 15 no mesmo tick");
    for p in at {
        assert!(dug(&g.world.borrow(), p) > 0, "sem cratera em {p:?}");
    }
}

/// P1-6 (M2 review), revised to follow HBM 1.7.10: the server stops right after the tick a nuke went off. The bomb is
/// spent and its explosion is in the world's save; read back, `EntityNukeExplosionMK5` keeps only `ticksExisted`
/// (`writeEntityToNBT`), its `strength` is 0 and it dies on its first update: the crater does not go on after the
/// restart, and the bomb does not come back either.
#[test]
fn p1_6_a_nuke_set_off_before_a_restart_stops_with_it_as_the_mk5_entity_does() {
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = bomb_game_on(w.clone(), 8);
    let at = [0, 70, 0];
    let id = "ntm:nuke_n2";
    place_bomb(&mut g, at, id);
    assemble(&mut g, at, id, 0..12);
    power(&mut g, at, true);
    for _ in 0..10 {
        g.run(1);
        if bomb_at(&mut g, at).is_none() {
            break;
        }
    }
    assert!(bomb_at(&mut g, at).is_none(), "não detonou");
    let before = dug(&w.borrow(), at);
    drop(g);
    let mut g = Game::new(w.clone());
    g.run(30);
    until_quiet(&mut g, 3000);
    assert_eq!(dug(&w.borrow(), at), before, "the crater stops with the world");
    assert!(bomb_at(&mut g, at).is_none(), "the bomb was spent");
    assert!(w.borrow().kv.map.keys().all(|k| !k.starts_with("nx.")), "{:?}", w.borrow().kv.map.keys());
}

/// P2-9: the area radiation's entry grace is per player: one who joins
/// later (not the client's) gets the dose once their own grace is over.
#[test]
fn p2_9_area_dose_grace_is_per_player() {
    let mut g = bomb_game(8);
    set_creative(&mut g, false);
    g.ready().world.bombs.zones.push(crate::blast::Zone { dim: OW.into(), center: [500, 70, 500], radius: 50, peak: 10_000, t0: 0, half_life: 1_000_000 });
    g.run(crate::radiation::GRACE_TICKS as usize + 20);
    let other = PlayerInfo { id: "p2".into(), name: "Alex".into(), dim: OW.into(), pos: [500.5, 70.0, 500.5], creative: false };
    let mut first = player();
    first.creative = false;
    g.server.set_players(vec![first, other]);
    g.run(40);
    let dose = |g: &mut Game| g.ready().world.radiation.get("p2").map_or(0, |d| d.millirad);
    assert_eq!(dose(&mut g), 0, "sem carência para quem acabou de entrar");
    g.run(crate::radiation::GRACE_TICKS as usize + 40);
    assert!(dose(&mut g) > 0, "a zona nunca deu dose ao segundo jogador");
}

/// P2-10: the detonator's bomb was replaced by another block (no break
/// event yet): nothing goes off, and that block stays.
#[test]
fn p2_10_the_detonator_checks_the_bomb_is_still_there() {
    let mut g = bomb_game(8);
    let at = [4, 70, 0];
    let id = "ntm:nuke_n2";
    place_bomb(&mut g, at, id);
    assemble(&mut g, at, id, 0..12);
    let p = player();
    g.server.detonator(&mut g.sh, &p, Some(at), true);
    g.run(3);
    g.world.borrow_mut().set(OW, at, "minecraft:diamond_block");
    let said = g.server.detonator(&mut g.sh, &p, None, false);
    assert!(said.contains("não está mais lá"), "{said}");
    g.run(3);
    assert_eq!(g.world.borrow().get(OW, at), "minecraft:diamond_block");
    assert!(g.server.blasts.nukes.is_empty() && g.ready().world.bombs.queue.is_empty(), "detonou outra coisa");
}

/// P1-13: test blasts out of range are refused before anything happens.
#[test]
fn p1_13_test_blasts_are_bounded() {
    let mut g = bomb_game(8);
    let who = player();
    for args in ["teste 100 forca", "teste 0 forca", "teste 0", "teste 99999", "teste -5 balefire"] {
        assert!(g.server.bomb_command(&mut g.sh, Some(&who), args).is_err(), "{args} aceito");
    }
    g.run(3);
    assert!(g.server.blasts.nukes.is_empty() && g.ready().world.bombs.queue.is_empty());
    assert!(g.server.bomb_command(&mut g.sh, Some(&who), "teste 4 forca").is_ok());
}


/// P2-12: a charge's damage needs a line of sight (vanilla's exposure):
/// behind an obsidian wall, none.
#[test]
fn p2_12_charge_damage_is_blocked_by_walls() {
    for wall in [false, true] {
        let mut g = bomb_game(8);
        let mut p = player();
        p.creative = false;
        p.pos = [6.5, 70.0, 0.5];
        g.server.set_players(vec![p]);
        if wall {
            let mut w = g.world.borrow_mut();
            for y in 69..75 {
                for z in -4..=4 {
                    w.set(OW, [3, y, z], "minecraft:obsidian");
                }
            }
        }
        let at = [0, 70, 0];
        place_bomb(&mut g, at, "ntm:tnt_ntm");
        power(&mut g, at, true);
        g.run(bomb::FUSE_TICKS as usize + 10);
        assert!(bomb_at(&mut g, at).is_none());
        let hits = g.world.borrow().hurts.iter().filter(|h| h.0 == "p1" && h.2 == "minecraft:explosion").count();
        if wall {
            assert_eq!(hits, 0, "dano através de obsidiana");
        } else {
            assert!(hits > 0, "sem dano em campo aberto");
        }
    }
}
