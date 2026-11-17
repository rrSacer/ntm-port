//! Integration tests for `ntm:machine_battery_socket` and `ntm:machine_battery_redd`
//! (`TileEntityBatterySocket.java`, `TileEntityBatteryREDD.java`, `TileEntityBatteryBase.java`).
use super::*;
use crate::energy_network::{BigInt, MODE_BUFFER, MODE_INPUT, MODE_NONE, MODE_OUTPUT};
use crate::machine::BatteryMode;
use crate::storage::{self, Body, Storage};

const LEAD: &str = "ntm:battery_pack__battery_lead";
const CORE: Pos = [0, 70, 0];

fn store(g: &mut Game, at: Pos) -> Storage {
    let Data::Storage(s) = g.machine(at).data else { panic!("no storage at {at:?}") };
    *s
}

fn edit<F: FnOnce(&mut Storage)>(g: &mut Game, at: Pos, f: F) {
    let r = g.ready();
    let Data::Storage(s) = &mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data else { panic!() };
    f(s);
}

fn battery(g: &mut Game, at: Pos, mode: BatteryMode, charge: u64) {
    let r = g.ready();
    let Data::Battery(b) = &mut r.world.machines.get_mut(&(OW.to_string(), at)).unwrap().data else { panic!() };
    b.mode = mode;
    b.power = charge;
}

fn bat_power(g: &mut Game, at: Pos) -> u64 {
    let Data::Battery(b) = g.machine(at).data else { panic!() };
    b.power
}

/// A pack put in slot `i` of the machine, as a player's hand does (the engine's slot, which the bridge reads).
fn put(g: &mut Game, at: Pos, i: usize, id: &str, charge: Option<i64>) {
    let mut s = crate::server::container_bridge::Stack::new(id, 1);
    s.custom = charge.map(|c| format!("{{charge:{c}l}}"));
    g.world.borrow_mut().engine.get_mut(&(OW.to_string(), at)).unwrap().0[i] = Some(s);
    g.run(1);
}

/// The pack's charge as the machine holds it: the engine's slot gets it with the next step (one tick behind).
fn pack_charge(g: &mut Game, at: Pos, i: usize) -> Option<i64> {
    store(g, at).kept[i].as_ref().and_then(storage::charge_of)
}

fn engine_charge(g: &mut Game, at: Pos, i: usize) -> Option<i64> {
    let w = g.world.borrow();
    w.engine[&(OW.to_string(), at)].0[i].as_ref().and_then(storage::charge_of)
}

/// A socket facing north at `CORE`: its west port (cell `[0,70,0]`, face -x) and its east one
/// (cell `[1,70,0]`, face +x) get a cable each; a battery sits at the end of each.
fn socket_line(g: &mut Game) -> (Pos, Pos) {
    g.place(CORE, "ntm:machine_battery_socket[facing=north,honey_level=0]");
    let (west, east) = ([-2, 70, 0], [3, 70, 0]);
    g.place([-1, 70, 0], "ntm:red_cable");
    g.place([2, 70, 0], "ntm:red_cable");
    g.place(west, "ntm:machine_battery[facing=north]");
    g.place(east, "ntm:machine_battery[facing=north]");
    g.run(3);
    (west, east)
}

#[test]
fn the_socket_is_a_conductor_whatever_its_mode() {
    let mut g = game();
    let (west, east) = socket_line(&mut g);
    battery(&mut g, west, BatteryMode::Output, 1_000_000);
    battery(&mut g, east, BatteryMode::Input, 0);
    for mode in [MODE_NONE, MODE_INPUT, MODE_OUTPUT, MODE_BUFFER] {
        edit(&mut g, CORE, |s| {
            s.red_low = mode;
            s.red_high = mode;
        });
        let before = (bat_power(&mut g, west), bat_power(&mut g, east));
        g.run(20);
        let after = (bat_power(&mut g, west), bat_power(&mut g, east));
        assert!(after.1 > before.1, "mode {mode}: the energy crosses the socket");
        assert_eq!(before.0 - after.0, after.1 - before.1, "mode {mode}: and none is lost");
    }
}

#[test]
fn a_socket_in_input_mode_charges_its_pack_from_the_net_and_the_stack_keeps_it() {
    let mut g = game();
    let (west, east) = socket_line(&mut g);
    battery(&mut g, west, BatteryMode::Output, 1_000_000);
    battery(&mut g, east, BatteryMode::None, 0);
    edit(&mut g, CORE, |s| s.red_low = MODE_INPUT);
    let before = bat_power(&mut g, west);
    put(&mut g, CORE, 0, LEAD, Some(0));
    g.run(9);
    let charge = pack_charge(&mut g, CORE, 0).unwrap();
    assert!(charge > 0, "charged");
    assert_eq!(before - bat_power(&mut g, west), charge as u64, "exactly what the battery lost");
    // A battery gives max / 600 a tick (1 666), the pack would take 10 000 (`chargeRate`).
    assert_eq!(charge, 10 * 1_666);
    // The engine's slot is written with each step: the network moved energy after the last one.
    assert_eq!(engine_charge(&mut g, CORE, 0), Some(9 * 1_666));
    g.run(1);
    assert_eq!(engine_charge(&mut g, CORE, 0), Some(10 * 1_666));
}

#[test]
fn a_redstone_signal_on_any_port_cell_puts_the_socket_in_its_high_mode() {
    let mut g = game();
    let (west, east) = socket_line(&mut g);
    // The sink is the east battery; the pack feeds it only in `redHigh` (output).
    battery(&mut g, east, BatteryMode::Input, 0);
    battery(&mut g, west, BatteryMode::None, 0);
    edit(&mut g, CORE, |s| {
        s.red_low = MODE_NONE;
        s.red_high = MODE_OUTPUT;
    });
    put(&mut g, CORE, 0, LEAD, Some(5_000_000));
    g.run(10);
    assert_eq!(bat_power(&mut g, east), 0, "no signal: no output");
    // The cell at the other end of the volume, the back right one.
    let cells = Storage::redstone_cells(storage::SOCKET, CORE, Some("north"));
    assert_eq!(cells.len(), 4);
    let far = *cells.iter().find(|c| **c != CORE).unwrap();
    g.world.borrow_mut().signal.insert((OW.to_string(), far), 7);
    g.run(10);
    let gained = bat_power(&mut g, east);
    assert!(gained > 0, "the signal switched it to output: {gained}");
    assert_eq!(5_000_000 - pack_charge(&mut g, CORE, 0).unwrap() as u64, gained, "the pack gave what the battery took");
    g.world.borrow_mut().signal.clear();
    g.run(2);
    let at = bat_power(&mut g, east);
    g.run(10);
    assert_eq!(bat_power(&mut g, east), at, "and without it, no more");
}

#[test]
fn a_pack_goes_in_only_as_a_battery_item_and_comes_out_by_hand_always() {
    use crate::native::{SlotAction, SlotOrigin, slot_allows};
    let steel = crate::server::container_bridge::Stack::new("ntm:ingot_steel", 1);
    let lead = crate::server::container_bridge::Stack::new(LEAD, 1);
    // The item rules are the machine file's: batteries only, one at a time.
    let filters = crate::native::block_slots(storage::SOCKET);
    assert_eq!(filters.len(), 1);
    assert!(filters[0].accept.as_ref().unwrap().contains(&LEAD.to_string()));
    assert!(!filters[0].accept.as_ref().unwrap().contains(&steel.id));
    assert_eq!(filters[0].max, Some(1));
    assert_eq!(crate::native::block_slots(storage::REDD).len(), 2);
    // A hand takes anything out; a hopper puts a battery into the socket and takes nothing from the REDD.
    for block in [storage::SOCKET, storage::REDD] {
        assert!(slot_allows(block, &[], 0, &lead, SlotAction::Extract, SlotOrigin::Manual));
        assert!(slot_allows(block, &[], 0, &lead, SlotAction::Insert, SlotOrigin::Manual));
    }
    assert!(slot_allows(storage::SOCKET, &[], 0, &lead, SlotAction::Insert, SlotOrigin::Automation));
    assert!(!slot_allows(storage::SOCKET, &[], 0, &lead, SlotAction::Extract, SlotOrigin::Automation));
    assert!(!slot_allows(storage::REDD, &[], 0, &lead, SlotAction::Insert, SlotOrigin::Automation));
    assert!(!slot_allows(storage::REDD, &[], 1, &lead, SlotAction::Extract, SlotOrigin::Automation));
}

/// A REDD facing north at `CORE`: the multiblock is 9 wide, its ports are the six of `getConPos`.
fn redd_ports(g: &mut Game) -> Vec<(Pos, Pos)> {
    g.place(CORE, "ntm:machine_battery_redd[facing=north,honey_level=0]");
    g.run(3);
    Storage::port_cells(storage::REDD, CORE, Some("north"))
}

#[test]
fn the_redd_takes_in_power_from_a_cable_on_any_port_and_gives_it_out_of_another() {
    let mut g = game();
    let ports = redd_ports(&mut g);
    assert_eq!(ports.len(), 6);
    // The cable on the first port's outside, and a battery behind it; the same on the last.
    let outside = |(c, f): (Pos, Pos), n: i32| [c[0] + f[0] * n, c[1], c[2] + f[2] * n];
    let (a, b) = (ports[0], ports[5]);
    g.place(outside(a, 1), "ntm:red_cable");
    g.place(outside(b, 1), "ntm:red_cable");
    let (src, sink) = (outside(a, 2), outside(b, 2));
    g.place(src, "ntm:machine_battery[facing=north]");
    g.place(sink, "ntm:machine_battery[facing=north]");
    g.run(3);
    battery(&mut g, src, BatteryMode::Output, 1_000_000);
    battery(&mut g, sink, BatteryMode::Input, 0);
    edit(&mut g, CORE, |s| s.red_low = MODE_BUFFER);
    g.run(30);
    let (lost, gained) = (1_000_000 - bat_power(&mut g, src), bat_power(&mut g, sink));
    assert!(gained > 0, "it passed through the REDD");
    assert_eq!(lost, gained + store(&mut g, CORE).power.long_value() as u64, "what left the source is in the sink or the store");
    // Its own store holds what the net poured in: it is a buffer, which is also a receiver.
    assert!(store(&mut g, CORE).power.long_value() > 0);
}

#[test]
fn the_redd_in_input_mode_absorbs_and_does_not_give_back_until_its_mode_says() {
    let mut g = game();
    let ports = redd_ports(&mut g);
    let (c, f) = ports[2];
    let (cable, src) = ([c[0] + f[0], c[1], c[2] + f[2]], [c[0] + f[0] * 2, c[1], c[2] + f[2] * 2]);
    g.place(cable, "ntm:red_cable");
    g.place(src, "ntm:machine_battery[facing=north]");
    g.run(3);
    battery(&mut g, src, BatteryMode::Output, 1_000_000);
    edit(&mut g, CORE, |s| s.red_low = MODE_INPUT);
    g.run(5);
    let held = store(&mut g, CORE).power.clone();
    assert_eq!(BigInt::from_i64(1_000_000 - bat_power(&mut g, src) as i64), held);
    assert_eq!(store(&mut g, CORE).offer(), 0, "input mode: no output");
    edit(&mut g, CORE, |s| s.red_low = MODE_NONE);
    g.run(5);
    assert_eq!(store(&mut g, CORE).power, held, "off: it neither takes nor gives");
}

#[test]
fn the_redd_keeps_its_store_when_the_world_is_saved_and_opened_again() {
    let mut g = game();
    redd_ports(&mut g);
    let mut huge = BigInt::from_i64(i64::MAX);
    for _ in 0..3 {
        huge = huge.add(&huge.clone());
    }
    edit(&mut g, CORE, |s| {
        s.power = huge.clone();
        s.red_low = MODE_NONE;
        s.red_high = MODE_BUFFER;
        s.priority = 3;
    });
    g.run(5);
    let w = g.world.clone();
    drop(g);
    let mut g = Game::new(w);
    g.run(5);
    let again = store(&mut g, CORE);
    assert_eq!((again.power, again.red_low, again.red_high, again.priority), (huge, MODE_NONE, MODE_BUFFER, 3));
    assert_eq!(again.body, Body::Redd);
}

#[test]
fn the_socket_remembers_its_modes_and_the_stacks_charge_is_the_pack_s() {
    let mut g = game();
    g.place(CORE, "ntm:machine_battery_socket[facing=north,honey_level=0]");
    g.run(3);
    put(&mut g, CORE, 0, LEAD, Some(777));
    edit(&mut g, CORE, |s| {
        s.red_low = MODE_BUFFER;
        s.priority = 3;
    });
    g.run(5);
    let w = g.world.clone();
    drop(g);
    let mut g = Game::new(w);
    g.run(5);
    let s = store(&mut g, CORE);
    assert_eq!((s.red_low, s.priority), (MODE_BUFFER, 3));
    assert_eq!(pack_charge(&mut g, CORE, 0), Some(777), "the pack kept its charge in the engine's slot");
}

#[test]
fn the_buttons_of_the_screen_reach_the_machine() {
    let mut g = game();
    g.place(CORE, "ntm:machine_battery_socket[facing=north,honey_level=0]");
    g.run(3);
    let mut was = [0, 2, 1];
    for (id, check) in [(0u32, 0usize), (1, 1), (2, 2)] {
        g.world.borrow_mut().presses.push((OW.to_string(), CORE, id));
        g.run(1);
        let s = store(&mut g, CORE);
        let now = [s.red_low as i32, s.red_high as i32, i32::from(s.priority)];
        for i in 0..3 {
            assert_eq!(now[i], if i == check { was[i] + 1 } else { was[i] }, "button {id}");
        }
        was = now;
        let w = g.world.borrow();
        let data = &w.engine[&(OW.to_string(), CORE)].1;
        assert_eq!(data[2..5], now, "the screen's data follows");
    }
}

