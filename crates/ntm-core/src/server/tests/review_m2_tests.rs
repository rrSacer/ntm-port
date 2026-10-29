//! The second review of the bombs (M2): each test failed before its fix
//! (docs/bombas.md, "Revisão").
use super::*;
use crate::bomb::{self, actions as bomb_actions};

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

fn bomb_game_on(world: Rc<RefCell<World>>, kv: Rc<RefCell<MemKv>>, max_r: i32) -> Game {
    let mut g = Game::new(world, kv, 1);
    g.run(10);
    floor(&mut g, 24);
    let who = player();
    g.server.bomb_command(&mut g.sh, Some(&who), &format!("config mundo raiomax {max_r}")).unwrap();
    g.run(2);
    g
}

fn bomb_game(max_r: i32) -> Game {
    bomb_game_on(Rc::new(RefCell::new(World::new())), Rc::new(RefCell::new(MemKv::default())), max_r)
}

fn place_bomb(g: &mut Game, at: Pos, id: &str) {
    g.place(at, &format!("{id}[lit=false]"));
    g.run(2);
}

fn power(g: &mut Game, at: Pos, id: &str) {
    g.world.borrow_mut().set(OW, at, &format!("{id}[lit=true]"));
}

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

fn bomb_at(g: &mut Game, at: Pos) -> Option<bomb::Bomb> {
    match g.ready().world.machines.get(&(OW.to_string(), at)).map(|m| m.data.clone()) {
        Some(Data::Bomb(b)) => Some(b),
        _ => None,
    }
}

fn ran(g: &Game, cmd: &str) -> usize {
    g.world.borrow().ran.iter().filter(|l| l.contains(cmd)).count()
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
        power(&mut g, p, "ntm:c4");
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

/// P1-6: the server stops right after the tick a nuke went off. The
/// detonation is in the world's save with the bomb's removal, so it still
/// happens after the restart.
#[test]
fn p1_6_a_detonation_survives_a_crash_in_its_tick() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = bomb_game_on(w.clone(), kv.clone(), 8);
    let at = [0, 70, 0];
    let id = "ntm:nuke_n2";
    place_bomb(&mut g, at, id);
    assemble(&mut g, at, id, 0..12);
    power(&mut g, at, id);
    for _ in 0..10 {
        g.run(1);
        if bomb_at(&mut g, at).is_none() {
            break;
        }
    }
    assert!(bomb_at(&mut g, at).is_none(), "não detonou");
    drop(g);
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.run(30);
    until_quiet(&mut g, 3000);
    assert!(dug(&w.borrow(), at) > 0, "a explosão se perdeu na queda");
    assert!(kv.borrow().map.keys().all(|k| !k.starts_with("nx.")), "{:?}", kv.borrow().map.keys());
}

/// P1-5: the last component's transaction is forgotten by a crash (the
/// world comes back from an older autosave, the igniter in the hand).
/// The detonator, used before the check against the world ends, doesn't
/// set the bomb off; the check then undoes the component.
#[test]
fn p1_5_no_detonation_before_the_components_are_confirmed() {
    let kv = Rc::new(RefCell::new(MemKv::default()));
    let w = Rc::new(RefCell::new(World::new()));
    let mut g = bomb_game_on(w.clone(), kv.clone(), 8);
    g.players[0].creative = false;
    w.borrow_mut().gamemode = "survival".into();
    let at = [4, 70, 0];
    let id = "ntm:nuke_boy";
    place_bomb(&mut g, at, id);
    assemble(&mut g, at, id, 0..4);
    g.client.detonator(&mut g.ch, OW, Some(at), true);
    g.run(3);
    let last = bomb::bomb_type(id).unwrap().slots[4].item;
    g.open(at);
    g.hold(last, 1);
    let autosave = w.borrow().clone();
    g.click(bomb_actions::INSERT);
    g.run(20);
    assert_eq!(g.held(), None);
    assert!(bomb_at(&mut g, at).unwrap().blast().is_some());
    drop(g);
    *w.borrow_mut() = autosave; // the crash: the last part in the hand again
    let mut g = Game::new(w.clone(), kv.clone(), 2);
    g.players[0].creative = false;
    for _ in 0..200 {
        g.run(1);
        if matches!(g.server.session, Session::Ready(_)) {
            break;
        }
    }
    assert!(!g.ready().verify.is_empty(), "nada a conferir");
    g.client.detonator(&mut g.ch, OW, None, false);
    g.run(3);
    assert!(bomb_at(&mut g, at).is_some(), "detonou antes de conferir os componentes");
    g.run(400);
    let b = bomb_at(&mut g, at).expect("a bomba continua");
    assert!(b.blast().is_none(), "o componente esquecido continua na bomba: {b:?}");
    assert_eq!(g.held(), Some((last.to_string(), 1)));
    assert!(g.server.blasts.nukes.is_empty());
}

/// P2-9: the area radiation's entry grace is per player: one who joins
/// later (not the client's) gets the dose once their own grace is over.
#[test]
fn p2_9_area_dose_grace_is_per_player() {
    let mut g = bomb_game(8);
    g.players[0].creative = false;
    g.ready().world.bombs.zones.push(crate::blast::Zone { dim: OW.into(), center: [500, 70, 500], radius: 50, peak: 10_000, t0: 0, half_life: 1_000_000 });
    g.run(crate::radiation::GRACE_TICKS as usize + 20);
    let other = PlayerInfo { id: "p2".into(), name: "Alex".into(), dim: OW.into(), pos: [500.5, 70.0, 500.5], creative: false };
    g.players.push(other);
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
    g.client.detonator(&mut g.ch, OW, Some(at), true);
    g.run(3);
    g.world.borrow_mut().set(OW, at, "minecraft:diamond_block");
    g.client.detonator(&mut g.ch, OW, None, false);
    g.run(3);
    assert_eq!(g.world.borrow().get(OW, at), "minecraft:diamond_block");
    assert!(g.server.blasts.nukes.is_empty() && g.ready().world.bombs.queue.is_empty(), "detonou outra coisa");
}

/// P2-12: a charge's damage needs a line of sight (vanilla's exposure):
/// behind an obsidian wall, none.
#[test]
fn p2_12_charge_damage_is_blocked_by_walls() {
    for wall in [false, true] {
        let mut g = bomb_game(8);
        g.players[0].creative = false;
        g.players[0].pos = [6.5, 70.0, 0.5];
        g.world.borrow_mut().gamemode = "survival".into();
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
        power(&mut g, at, "ntm:tnt_ntm");
        g.run(bomb::FUSE_TICKS as usize + 10);
        assert!(bomb_at(&mut g, at).is_none());
        let hits = ran(&g, "damage Steve");
        if wall {
            assert_eq!(hits, 0, "dano através de obsidiana");
        } else {
            assert!(hits > 0, "sem dano em campo aberto");
        }
    }
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

/// P2-11: the explosions get what the folder has left after the worlds'
/// saves (with room for them to grow), and they count in its warning.
#[test]
fn p2_11_explosions_share_the_folder_quota() {
    let mut g = bomb_game(8);
    let who = player();
    g.server.bomb_command(&mut g.sh, Some(&who), "teste 8").unwrap();
    g.run(3);
    let lim = g.server.blasts.nukes.limits;
    let (core, keys) = g.server.worlds.usage();
    assert!(lim.total_bytes + core + persist::MAX_SAVE as u64 <= persist::QUOTA, "{lim:?} + {core}");
    assert!(lim.keys + keys < persist::MAX_KEYS, "{lim:?} + {keys} chaves");
    assert!(lim.tick_bytes < 1 << 20);
}
