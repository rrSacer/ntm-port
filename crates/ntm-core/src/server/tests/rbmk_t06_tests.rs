//! T06: a screen session per player (`Server::native_screen_open`), its commands taken only from the current session,
//! once, within reach, for the block entity it opened on, and applied in the machine's next transaction; a refused
//! command changes nothing; the snapshot follows the block while the screen is open.
use super::*;
use crate::rbmk::persist::{AutoControl, RbmkColumn};
use crate::screens::control::{Command, Fields, Grant, Refusal, Snapshot, Value};

const Y: i32 = 71;
const AT: Pos = [0, Y, 0];
const NEAR: [f64; 3] = [0.5, Y as f64, 3.5];

fn column(id: &str) -> String {
    format!("ntm:{id}[facing=north,honey_level=0]")
}

fn auto(g: &mut Game) -> Option<AutoControl> {
    let Data::Rbmk(c) = g.ready().world.machines[&(OW.to_string(), AT)].data.clone() else { panic!("not a column") };
    c.auto
}

fn settings(v: [f64; 4]) -> Fields {
    let mut f = Fields::default();
    f.set("levelUpper", Value::Double(v[0]))
        .set("levelLower", Value::Double(v[1]))
        .set("heatUpper", Value::Double(v[2]))
        .set("heatLower", Value::Double(v[3]));
    f
}

fn opened() -> (Game, Grant) {
    let mut g = game();
    g.place(AT, &column("rbmk_control_auto"));
    g.run(2);
    let grant = Grant::decode(&g.server.native_screen_open("p1", OW, AT).expect("a screen")).unwrap();
    (g, grant)
}

fn cmd(grant: &Grant, seq: u32, fields: Fields) -> Vec<u8> {
    Command { session: grant.session, generation: grant.snapshot.generation, pos: AT, seq, fields }.encode().unwrap()
}

/// What a refusal must leave as it was: the machine (its model and its save) and every block entity of the engine.
fn untouched(g: &mut Game) -> (String, String) {
    let m = serde_json::to_string(&g.ready().world.machines[&(OW.to_string(), AT)]).unwrap();
    let e = format!("{:?}", g.world.borrow().engine);
    (m, e)
}

#[test]
fn a_command_of_the_open_screen_sets_the_column_in_its_next_transaction() {
    let (mut g, grant) = opened();
    assert_eq!(auto(&mut g), Some(AutoControl::default()));
    assert_eq!(crate::rbmk::screen::auto_of(&grant.snapshot.fields), AutoControl::default());
    assert_eq!(g.server.native_screen_command("p1", OW, NEAR, &cmd(&grant, 1, settings([90.0, 10.0, 800.0, 200.0]))), Ok(()));
    // Taken, waiting for the machine's transaction: the column moves with the next tick, not before.
    assert_eq!(auto(&mut g), Some(AutoControl::default()));
    g.run(1);
    let a = auto(&mut g).unwrap();
    assert_eq!((a.level_upper, a.level_lower, a.heat_upper, a.heat_lower), (90.0, 10.0, 800.0, 200.0));
    // The block entity's save holds it: a reopened world reads it back.
    let world = g.world.borrow().clone();
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    reopened.run(1);
    let Data::Rbmk(c) = reopened.ready().world.machines[&(OW.to_string(), AT)].data.clone() else { panic!() };
    assert_eq!(c.auto, Some(a));
    // The curve, and the screen's data shows it (its overlays).
    let mut f = Fields::default();
    f.set("function", Value::Int(2));
    assert_eq!(g.server.native_screen_command("p1", OW, NEAR, &cmd(&grant, 2, f)), Ok(()));
    g.run(1);
    assert_eq!(auto(&mut g).unwrap().function, 2);
    assert_eq!(g.world.borrow().engine[&(OW.to_string(), AT)].1[2], 2);
}

#[test]
fn a_refused_command_changes_nothing() {
    let (mut g, grant) = opened();
    let good = settings([50.0, 5.0, 500.0, 50.0]);
    let before = untouched(&mut g);
    let refuse = |g: &mut Game, player: &str, dim: &str, at: [f64; 3], data: Vec<u8>, why: Refusal| {
        assert_eq!(g.server.native_screen_command(player, dim, at, &data), Err(why.clone()), "{why:?}");
        g.run(1);
        assert_eq!(untouched(g), before, "after {why:?}");
    };
    refuse(&mut g, "p2", OW, NEAR, cmd(&grant, 1, good.clone()), Refusal::NoSession);
    refuse(&mut g, "p1", OW, [20.0, Y as f64, 0.0], cmd(&grant, 1, good.clone()), Refusal::TooFar);
    refuse(&mut g, "p1", "minecraft:the_nether", NEAR, cmd(&grant, 1, good.clone()), Refusal::TooFar);
    let mut bad = Fields::default();
    bad.set("function", Value::Int(3));
    assert!(matches!(g.server.native_screen_command("p1", OW, NEAR, &cmd(&grant, 1, bad)), Err(Refusal::Invalid(_))));
    g.run(1);
    assert_eq!(untouched(&mut g), before);
    let mut broken = cmd(&grant, 2, good.clone());
    broken.push(0);
    assert!(matches!(g.server.native_screen_command("p1", OW, NEAR, &broken), Err(Refusal::Invalid(_))));
    // Another place, an older generation in the command.
    let other = Command { session: grant.session, generation: grant.snapshot.generation, pos: [1, Y, 0], seq: 3, fields: good.clone() };
    refuse(&mut g, "p1", OW, NEAR, other.encode().unwrap(), Refusal::OtherPlace);
    let old = Command { generation: grant.snapshot.generation.wrapping_sub(1), ..Command::decode(&cmd(&grant, 3, good.clone())).unwrap() };
    refuse(&mut g, "p1", OW, NEAR, old.encode().unwrap(), Refusal::Generation);
    // Taken once: the same sequence number again is a repeat, whatever it carries.
    assert_eq!(g.server.native_screen_command("p1", OW, NEAR, &cmd(&grant, 10, good.clone())), Ok(()));
    g.run(1);
    let after = untouched(&mut g);
    assert_ne!(after, before);
    let other_values = settings([1.0, 1.0, 1.0, 1.0]);
    assert_eq!(g.server.native_screen_command("p1", OW, NEAR, &cmd(&grant, 10, other_values.clone())), Err(Refusal::Repeated));
    assert_eq!(g.server.native_screen_command("p1", OW, NEAR, &cmd(&grant, 9, other_values.clone())), Err(Refusal::Repeated));
    g.run(1);
    assert_eq!(untouched(&mut g), after);
    // A screen opened again: the earlier session's commands are refused.
    let again = Grant::decode(&g.server.native_screen_open("p1", OW, AT).unwrap()).unwrap();
    assert_ne!(again.session, grant.session);
    assert_eq!(g.server.native_screen_command("p1", OW, NEAR, &cmd(&grant, 11, other_values.clone())), Err(Refusal::OldSession));
    g.run(1);
    assert_eq!(untouched(&mut g), after);
    // The player left.
    g.server.native_screen_left("p1");
    assert_eq!(g.server.native_screen_command("p1", OW, NEAR, &cmd(&again, 1, other_values)), Err(Refusal::NoSession));
}

#[test]
fn a_command_waiting_on_a_column_that_went_is_dropped() {
    let (mut g, grant) = opened();
    assert_eq!(g.server.native_screen_command("p1", OW, NEAR, &cmd(&grant, 1, settings([90.0, 10.0, 800.0, 200.0]))), Ok(()));
    // The column is replaced before the command's transaction: a new block entity, a new generation.
    let old = g.world.borrow().get(OW, AT);
    g.world.borrow_mut().set(OW, AT, "minecraft:air");
    g.server.native_block_changed(OW, AT, &old, "minecraft:air");
    g.run(1);
    g.place(AT, &column("rbmk_control_auto"));
    g.run(2);
    assert_eq!(auto(&mut g), Some(AutoControl::default()), "the new column is as placed");
    // The session was the old block entity's.
    assert_eq!(g.server.native_screen_command("p1", OW, NEAR, &cmd(&grant, 2, settings([1.0; 4]))), Err(Refusal::Generation));
}

#[test]
fn the_snapshot_follows_the_column_while_the_screen_is_open() {
    let (mut g, grant) = opened();
    assert!(g.server.native_screen_snapshots().is_empty(), "the grant was the first");
    g.server.native_screen_command("p1", OW, NEAR, &cmd(&grant, 1, settings([90.0, 10.0, 800.0, 200.0]))).unwrap();
    g.run(1);
    let sent = g.server.native_screen_snapshots();
    assert_eq!(sent.len(), 1);
    let s = Snapshot::decode(&sent[0].1).unwrap();
    assert_eq!((sent[0].0.as_str(), s.session, s.pos), ("p1", grant.session, AT));
    assert_eq!(crate::rbmk::screen::auto_of(&s.fields).level_upper, 90.0);
    // Unchanged: nothing more.
    g.run(3);
    assert!(g.server.native_screen_snapshots().is_empty());
    // A column with no screen of its kind opens none.
    g.place([3, Y, 3], &column("rbmk_control"));
    g.run(1);
    assert!(g.server.native_screen_open("p1", OW, [3, Y, 3]).is_none());
}

#[test]
fn a_column_saved_before_t06_opens_with_the_tes_defaults() {
    let (mut g, _) = opened();
    let key = (OW.to_string(), AT);
    if let Data::Rbmk(c) = &mut g.ready().world.machines.get_mut(&key).unwrap().data {
        **c = RbmkColumn { auto: None, ..(**c).clone() };
    }
    let grant = Grant::decode(&g.server.native_screen_open("p1", OW, AT).unwrap()).unwrap();
    assert_eq!(crate::rbmk::screen::auto_of(&grant.snapshot.fields), AutoControl::default());
}

#[test]
fn a_screen_without_a_machine_slot_has_a_menu() {
    for b in ["ntm:rbmk_control_auto", "ntm:rbmk_control_reasim_auto"] {
        let c = crate::native::container(b).expect("a menu");
        assert_eq!((c.slots, c.inputs.len(), c.outputs.len()), (0, 0, 0), "{b}");
    }
    // The manual rods' screen is T07's: no menu yet, as before.
    assert!(crate::native::container("ntm:rbmk_control").is_none());
}
