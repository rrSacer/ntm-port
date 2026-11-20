//! T10: the RBMK console (`TileEntityRBMKConsole`): its scan of the 15×15 cells around its target every 10 ticks, and
//! its commands through its screen's session, which wait in the queues of the columns they touch (AZ-5 only on the
//! manual rods).
use super::*;
use crate::rbmk::console::{self, ColumnType, Console};
use crate::rbmk::persist::{ControlState, RbmkColumn};
use crate::screens::control::{Command, Fields, Grant, Value};

const Y: i32 = 71;
const CONSOLE: Pos = [20, Y, 20];
const TARGET: Pos = [0, Y, 0];

fn column(id: &str) -> String {
    format!("ntm:{id}[facing=north,honey_level=0]")
}

fn col(g: &mut Game, p: Pos) -> RbmkColumn {
    let Data::Rbmk(c) = g.ready().world.machines[&(OW.to_string(), p)].data.clone() else { panic!("not a column") };
    *c
}

fn console_of(g: &mut Game) -> Console {
    let Data::RbmkConsole(c) = g.ready().world.machines[&(OW.to_string(), CONSOLE)].data.clone() else { panic!("not a console") };
    *c
}

fn link(g: &mut Game, target: Pos, rotation: u8) {
    let Data::RbmkConsole(c) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), CONSOLE)).unwrap().data else { panic!() };
    (c.state.target, c.state.rotation) = (target, rotation);
}

/// The fields sent from the console's open screen, standing beside it.
fn send(g: &mut Game, seq: u32, fields: Fields) -> Result<(), crate::screens::control::Refusal> {
    let grant = Grant::decode(&g.server.native_screen_open("p1", OW, CONSOLE).expect("a screen")).unwrap();
    let cmd = Command { session: grant.session, generation: grant.snapshot.generation, pos: CONSOLE, seq, fields };
    let at = [f64::from(CONSOLE[0]) + 0.5, f64::from(CONSOLE[1]), f64::from(CONSOLE[2]) + 2.5];
    g.server.native_screen_command("p1", OW, at, &cmd.encode().unwrap())
}

/// A reactor: a manual rod at the target, a moderated manual rod and an automatic rod beside it, a boiler.
fn reactor(g: &mut Game) {
    g.place(CONSOLE, "ntm:rbmk_console[facing=north,honey_level=0]");
    g.place(TARGET, &column("rbmk_control"));
    g.place([2, Y, 0], &column("rbmk_control_mod"));
    g.place([4, Y, 0], &column("rbmk_control_auto"));
    g.place([0, Y, 2], &column("rbmk_boiler"));
    g.run(2);
    for p in [TARGET, [2, Y, 0], [4, Y, 0]] {
        let Data::Rbmk(c) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), p)).unwrap().data else { panic!() };
        let st = c.control.get_or_insert_with(ControlState::default);
        (st.level, st.target_level) = (0.6, 0.6);
    }
}

/// Every 10 ticks the console sees the columns of its window, turned by its rotation: their types, heat and rods'
/// levels; the screen reads them.
#[test]
fn the_console_scans_its_window_every_ten_ticks() {
    let mut g = game();
    reactor(&mut g);
    link(&mut g, TARGET, 0);
    g.run(11);
    let c = console_of(&mut g);
    assert_eq!(c.scan.len(), 225);
    let at = |x: i32, z: i32| (0..225).find(|&i| console::offset(c.state.rotation, i) == (x, z)).unwrap() as usize;
    assert_eq!(c.scan[at(0, 0)].as_ref().map(|v| v.kind), Some(ColumnType::Control));
    assert_eq!(c.scan[at(4, 0)].as_ref().map(|v| v.kind), Some(ColumnType::ControlAuto));
    assert_eq!(c.scan[at(0, 2)].as_ref().map(|v| v.kind), Some(ColumnType::Boiler));
    assert!(c.scan[at(1, 0)].is_none());
    // A column placed now is seen at the next multiple of 10 ticks, not before.
    g.place([1, Y, 0], &column("rbmk_blank"));
    let seen = (0..20)
        .find(|_| {
            g.run(1);
            console_of(&mut g).scan[at(1, 0)].is_some()
        })
        .expect("seen within 20 ticks");
    // (The world's counter moves on at the end of the step that scanned.)
    let scanned_at = g.ready().world.ticks - 1;
    assert!(seen > 0 && scanned_at.is_multiple_of(10), "seen after {seen} ticks, scanned at tick {scanned_at}");
    // Turned 90°: the same columns at other indices.
    link(&mut g, TARGET, 1);
    g.run(10);
    let turned = console_of(&mut g);
    let at1 = |x: i32, z: i32| (0..225).find(|&i| console::offset(1, i) == (x, z)).unwrap() as usize;
    assert_eq!(turned.scan[at1(4, 0)].as_ref().map(|v| v.kind), Some(ColumnType::ControlAuto));
    assert_ne!(at1(4, 0), at(4, 0));
    // The screen's snapshot carries the types.
    let grant = Grant::decode(&g.server.native_screen_open("p1", OW, CONSOLE).unwrap()).unwrap();
    let kinds = grant.snapshot.fields.get_int_array("k").unwrap();
    assert_eq!(kinds[at1(0, 0)], ColumnType::Control as i32);
    assert_eq!(kinds.iter().filter(|k| **k >= 0).count(), 5, "three rods, the boiler, the blank");
}

/// AZ-5 (`level` 0 for every CONTROL column the scan saw): the two manual rods take a target of 0 from where they are
/// at the next tick; the automatic rod is not touched.
#[test]
fn az5_drops_the_manual_rods_only() {
    let mut g = game();
    reactor(&mut g);
    link(&mut g, TARGET, 0);
    g.run(11);
    let kinds: Vec<Option<ColumnType>> = console_of(&mut g).scan.iter().map(|c| c.as_ref().map(|v| v.kind)).collect();
    send(&mut g, 1, console::az5(&kinds)).unwrap();
    assert!(!queued_for(&mut g, [4, Y, 0]), "nothing waits for the automatic rod (its curve sets its target)");
    g.run(1);
    for p in [TARGET, [2, Y, 0]] {
        let st = col(&mut g, p).control.unwrap();
        assert_eq!((st.target_level, st.starting_level), (0.0, 0.6), "{p:?}");
        assert!(st.level < 0.6, "it moves down at once: {}", st.level);
    }
}

fn queued_for(g: &mut Game, p: Pos) -> bool {
    g.ready().pending_commands.iter().any(|q| q.value.0 == (OW.to_string(), p))
}

/// A level for a selection, a group set outright, the boiler's compressor and the screens: each waits for its column's
/// (or the console's) next tick; a refused command changes nothing.
#[test]
fn the_consoles_commands_reach_their_columns() {
    let mut g = game();
    reactor(&mut g);
    link(&mut g, TARGET, 0);
    g.run(11);
    let idx = |x: i32, z: i32| (0..225).find(|&i| console::offset(0, i) == (x, z)).unwrap();
    let mut f = Fields::default();
    f.set("level", Value::Double(0.25)).set(&format!("sel_{}", idx(2, 0)), Value::Int(idx(2, 0))).set("sel_x", Value::Int(idx(4, 0)));
    send(&mut g, 1, f).unwrap();
    let mut f = Fields::default();
    f.set("assignColor", Value::Byte(3)).set("cols", Value::IntArray(vec![idx(0, 0), idx(2, 0), idx(4, 0)]));
    send(&mut g, 2, f).unwrap();
    assert!(!queued_for(&mut g, [4, Y, 0]), "a level and a group only for manual rods");
    let mut f = Fields::default();
    f.set("toggle", Value::Byte(1)).set("id", Value::Byte(1)).set("s7", Value::Byte(1));
    send(&mut g, 3, f).unwrap();
    with_boiler_cold(&mut g);
    let mut f = Fields::default();
    f.set("compressor", Value::Byte(1)).set("cols", Value::IntArray(vec![idx(0, 2)]));
    send(&mut g, 4, f).unwrap();
    g.run(1);
    assert_eq!(col(&mut g, [2, Y, 0]).control.unwrap().target_level, 0.25);
    assert_eq!((col(&mut g, TARGET).control.unwrap().color, col(&mut g, [2, Y, 0]).control.unwrap().color), (Some(3), Some(3)));
    assert_eq!(col(&mut g, [0, Y, 2]).piped.unwrap().output.fluid, "HOTSTEAM", "the boiler's compressor");
    let s = console_of(&mut g).state.screens[1].clone();
    assert_eq!((s.kind, s.columns), (1, vec![7]));
    // Refused whole: a column out of the window.
    let before = (console_of(&mut g).state, col(&mut g, [2, Y, 0]).control);
    let mut f = Fields::default();
    f.set("level", Value::Double(1.0)).set("sel_a", Value::Int(225)).set("sel_b", Value::Int(idx(2, 0)));
    assert!(send(&mut g, 5, f).is_err());
    g.run(1);
    assert_eq!((console_of(&mut g).state, col(&mut g, [2, Y, 0]).control.map(|c| c.target_level)), (before.0, before.1.map(|c| c.target_level)));
}

fn with_boiler_cold(g: &mut Game) {
    let Data::Rbmk(c) = &mut g.ready().world.machines.get_mut(&(OW.to_string(), [0, Y, 2])).unwrap().data else { panic!() };
    c.heat = 20.0;
}
