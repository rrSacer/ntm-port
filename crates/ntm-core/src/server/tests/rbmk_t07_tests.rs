//! T07: the five control rods in the step as their TEs (`TileEntityRBMKControl` and its manual and automatic kinds):
//! the target their screens set, the level that follows it at `speed * dialControlSpeed` a tick, the ReaSim rods'
//! power from the network below (5000 a tick moved, 50000 at most, never saved), the automatic curve over the column's
//! heat, and what the save keeps.
use super::*;
use crate::rbmk::persist::{ControlState, RbmkColumn};
use crate::screens::control::{Command, Fields, Grant, Value};

const Y: i32 = 71;
const AT: Pos = [0, Y, 0];
/// `TileEntityRBMKControl.speed`, the dial at its default 1.
const SPEED: f64 = 0.00277;

fn column(id: &str) -> String {
    format!("ntm:{id}[facing=north,honey_level=0]")
}

fn col(g: &mut Game, p: Pos) -> RbmkColumn {
    let Data::Rbmk(c) = g.ready().world.machines[&(OW.to_string(), p)].data.clone() else { panic!("not a column") };
    *c
}

fn ctl(g: &mut Game, p: Pos) -> ControlState {
    col(g, p).control.expect("a control rod")
}

/// The fields sent from the open screen of the column at `p`, as its next command.
fn send(g: &mut Game, p: Pos, seq: u32, fields: Fields) {
    let grant = Grant::decode(&g.server.native_screen_open("p1", OW, p).expect("a screen")).unwrap();
    let cmd = Command { session: grant.session, generation: grant.snapshot.generation, pos: p, seq, fields };
    let at = [f64::from(p[0]) + 0.5, f64::from(p[1]), f64::from(p[2]) + 3.5];
    g.server.native_screen_command("p1", OW, at, &cmd.encode().unwrap()).unwrap();
}

fn level(v: f64) -> Fields {
    let mut f = Fields::default();
    f.set("level", Value::Double(v));
    f
}

#[test]
fn a_manual_rod_moves_to_its_target_at_the_javas_speed_and_keeps_it() {
    let mut g = game();
    g.place(AT, &column("rbmk_control"));
    g.run(2);
    let st = ctl(&mut g, AT);
    assert_eq!((st.level, st.target_level, st.powered), (0.0, 0.0, false), "a fresh rod: in, no target, not powered");
    send(&mut g, AT, 1, level(1.0));
    g.run(1);
    let st = ctl(&mut g, AT);
    assert_eq!((st.target_level, st.starting_level), (1.0, 0.0), "the start is where the rod was");
    // One step a tick, each `+= speed`, as the Java adds it (by the bits), until the target.
    let mut expect = 0.0f64;
    for _ in 0..5 {
        let before = ctl(&mut g, AT).level;
        g.run(1);
        let now = ctl(&mut g, AT).level;
        expect = before + SPEED;
        assert_eq!(now.to_bits(), expect.to_bits());
    }
    assert!(expect > 0.0);
    // At the target it stops there exactly: 1 / 0.00277 is 361.01 steps.
    g.run(400);
    assert_eq!(ctl(&mut g, AT).level, 1.0);
    // The model shows it (`level` in thousandths), the save keeps level, target and start.
    assert_eq!(g.world.borrow().engine[&(OW.to_string(), AT)].1[1], 1000);
    let world = g.world.borrow().clone();
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    reopened.run(1);
    let st = ctl(&mut reopened, AT);
    assert_eq!((st.level, st.target_level, st.starting_level), (1.0, 1.0, 0.0));
    // Down to 0.5: the start is 1 again, and it moves back.
    send(&mut reopened, AT, 1, level(0.5));
    reopened.run(2);
    let st = ctl(&mut reopened, AT);
    assert_eq!(st.starting_level, 1.0);
    assert!(st.level < 1.0 && st.level > 0.5);
}

#[test]
fn a_reasim_rod_moves_only_on_power_and_pays_5000_for_each_tick_it_moves() {
    let mut g = game();
    g.place(AT, &column("rbmk_control_reasim"));
    g.run(2);
    send(&mut g, AT, 1, level(1.0));
    g.run(20);
    let st = ctl(&mut g, AT);
    assert!(st.powered && !st.has_power, "no network: no power");
    assert_eq!((st.level, st.power), (0.0, 0), "it does not move");
    // A full battery feeding a cable under the rod (`canConnect`: DOWN only).
    let bat = [1, Y - 1, 0];
    g.place([0, Y - 1, 0], "ntm:red_cable");
    g.place(bat, "ntm:machine_battery[facing=east,honey_level=0]");
    g.run(3);
    if let Some(Data::Battery(b)) = g.ready().world.machines.get_mut(&(OW.to_string(), bat)).map(|m| &mut m.data) {
        // Enough to give the rod more than a move a tick (`max / 600`).
        b.max = 100_000_000;
        b.power = b.max;
        b.mode = crate::machine::BatteryMode::Output;
    }
    let battery = |g: &mut Game| match &g.ready().world.machines[&(OW.to_string(), bat)].data {
        Data::Battery(b) => b.power,
        _ => panic!("not a battery"),
    };
    // What the battery gives is what the rod's buffer gains plus 5000 for each tick it moved, each move one `speed`
    // (`level != lastLevel`), and never more than its 50000.
    let (b0, p0) = (battery(&mut g), ctl(&mut g, AT).power);
    let mut moves = 0;
    for _ in 0..200 {
        let before = ctl(&mut g, AT);
        g.run(1);
        let after = ctl(&mut g, AT);
        if after.level != before.level {
            moves += 1;
            assert_eq!(after.level.to_bits(), (before.level + SPEED).min(1.0).to_bits());
        }
        assert!(after.power <= crate::rbmk::persist::CONTROL_MAX_POWER);
    }
    let (b1, p1) = (battery(&mut g), ctl(&mut g, AT).power);
    assert!(moves > 0);
    assert_eq!(b0 as i64 - b1 as i64, (p1 - p0) + 5_000 * moves, "{moves} moves");
    // At its target it pays nothing: the buffer fills and the battery gives no more.
    for _ in 0..3000 {
        if ctl(&mut g, AT).level == 1.0 {
            break;
        }
        g.run(1);
    }
    g.run(40);
    let st = ctl(&mut g, AT);
    assert_eq!((st.level, st.power), (1.0, crate::rbmk::persist::CONTROL_MAX_POWER));
    let full = battery(&mut g);
    g.run(5);
    assert_eq!((battery(&mut g), ctl(&mut g, AT).power), (full, crate::rbmk::persist::CONTROL_MAX_POWER));
    // The buffer is not saved (`writeToNBT` has no `power`): a reopened rod starts empty, its level kept.
    let json = serde_json::to_string(&col(&mut g, AT)).unwrap();
    assert!(!json.contains("\"power\"") && !json.contains("has_power"), "{json}");
    let world = g.world.borrow().clone();
    let mut reopened = Game::new(Rc::new(RefCell::new(world)));
    reopened.run(1);
    let st = ctl(&mut reopened, AT);
    assert_eq!(st.level, 1.0);
}

#[test]
fn the_power_pays_only_when_the_rod_moves() {
    // The step on its own: a powered rod with 5000 moves once and is out; with 4999 it never starts.
    let mut g = game();
    g.place(AT, &column("rbmk_control_reasim"));
    g.run(2);
    send(&mut g, AT, 1, level(1.0));
    g.run(1);
    let set_power = |g: &mut Game, v: i64| {
        if let Some(Data::Rbmk(c)) = g.ready().world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) {
            c.control.as_mut().unwrap().power = v;
        }
    };
    set_power(&mut g, 4_999);
    g.run(1);
    let st = ctl(&mut g, AT);
    assert_eq!((st.level, st.power, st.has_power), (0.0, 4_999, false));
    set_power(&mut g, 5_000);
    g.run(1);
    let st = ctl(&mut g, AT);
    assert_eq!((st.level, st.power, st.has_power), (SPEED, 0, true));
    g.run(1);
    assert_eq!(ctl(&mut g, AT).level, SPEED, "out of power");
}

#[test]
fn an_automatic_rod_follows_its_curve_over_the_columns_heat() {
    let mut g = game();
    g.place(AT, &column("rbmk_control_auto"));
    g.run(2);
    // The column at 20 °C: below the lower bound the rod goes to the level at the lower bound.
    let mut f = Fields::default();
    f.set("levelUpper", Value::Double(10.0))
        .set("levelLower", Value::Double(40.0))
        .set("heatUpper", Value::Double(500.0))
        .set("heatLower", Value::Double(100.0));
    send(&mut g, AT, 1, f);
    g.run(2);
    assert_eq!(ctl(&mut g, AT).target_level, 0.4);
    g.run(200);
    assert_eq!(ctl(&mut g, AT).level, 0.4);
    // Equal bounds at the column's heat: 0/0 in the curve, a NaN target, a rod that no longer moves (D2: as the Java).
    let mut f = Fields::default();
    f.set("levelUpper", Value::Double(10.0))
        .set("levelLower", Value::Double(40.0))
        .set("heatUpper", Value::Double(20.0))
        .set("heatLower", Value::Double(20.0));
    send(&mut g, AT, 1, f);
    g.run(5);
    let st = ctl(&mut g, AT);
    assert!(st.target_level.is_nan(), "{st:?}");
    assert_eq!(st.level, 0.4);
    // Inverted bounds read as min and max for the comparisons, the curve with them as given.
    let mut f = Fields::default();
    f.set("levelUpper", Value::Double(0.0))
        .set("levelLower", Value::Double(100.0))
        .set("heatUpper", Value::Double(0.0))
        .set("heatLower", Value::Double(40.0));
    send(&mut g, AT, 1, f);
    g.run(2);
    // heat 20 between 0 and 40: (20 - 40) * ((0 - 100) / (0 - 40)) + 100 = 50.
    assert_eq!(ctl(&mut g, AT).target_level, 0.5);
}

#[test]
fn a_control_rod_from_before_t07_reads_as_the_fresh_te() {
    let mut g = game();
    g.place(AT, &column("rbmk_control_reasim_auto"));
    g.run(2);
    if let Some(Data::Rbmk(c)) = g.ready().world.machines.get_mut(&(OW.to_string(), AT)).map(|m| &mut m.data) {
        c.control = None;
    }
    g.run(1);
    let st = ctl(&mut g, AT);
    assert!(st.powered, "the step gave it its state back, as the fresh TE");
    assert_eq!((st.level, st.target_level), (0.0, 0.0));
}

#[test]
fn a_rod_at_zero_stops_a_stream_and_one_out_lets_it_through() {
    // A uranium channel, the control rod, a second channel: the far channel takes the near one's flux only once the
    // rod is out (`getMult`: level 0 ends the stream).
    let mut g = game();
    for (p, id) in [([0, Y, 0], "rbmk_rod_reasim"), ([1, Y, 0], "rbmk_control"), ([2, Y, 0], "rbmk_rod")] {
        g.place(p, &column(id));
    }
    g.run(2);
    for (p, rod) in [([0, Y, 0], "ntm:rbmk_fuel_po210be"), ([2, Y, 0], "ntm:rbmk_fuel_ueu")] {
        let stack = crate::server::container_bridge::Stack::new(rod, 2);
        let held = HeldStack { encoded: format!("{stack:?}"), stack };
        g.world.borrow_mut().hands = Some(PlayerSlots { hand: Some(PlayerSlot::Inventory(0)), inventory: vec![Some(held), None], creative: false, ..Default::default() });
        assert!(g.server.native_rbmk_use(&mut g.sh, OW, p, p, "p1"));
    }
    let far = |g: &mut Game| (0..40).map(|_| { g.run(1); col(g, [2, Y, 0]).last_flux_quantity }).fold(0.0, f64::max);
    assert_eq!(far(&mut g), 0.0, "the rod in: no flux beyond it");
    send(&mut g, [1, Y, 0], 1, level(1.0));
    g.run(400);
    assert!(far(&mut g) > 0.0, "the rod out: the stream goes through");
}

