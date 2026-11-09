//! `Reactor::remove` (the Java `invalidate()` of a rod) and the column bookkeeping around it.

use ntm_core::rbmk::dials::{Dials, GameRules};
use ntm_core::rbmk::jrandom::JavaRandom;
use ntm_core::rbmk::reactor::{Column, Kind, Reactor};
use ntm_core::rbmk::rods::{RodStack, RodType};

fn reactor(rules: &[(&str, &str)]) -> Reactor {
    let mut r = GameRules::default();
    for (k, v) in rules {
        r.set(k, v);
    }
    Reactor::new(Dials::refresh(&r, false), JavaRandom::new(1), 10)
}

fn rod_at(x: i32, hull: f64) -> Column {
    let rod = RodType::by_variable("rbmk_fuel_ueu").unwrap();
    let mut stack = RodStack::fresh(rod);
    stack.set_hull_heat(rod, hull);
    let mut c = Column::new(Kind::Rod, x, 0);
    c.rod_mut().unwrap().fuel = Some((rod, stack));
    c
}

#[test]
fn removing_a_hot_rod_requests_the_meltdown_only_at_1500_or_more() {
    for (hull, melts) in [(1499.999, false), (1500.0, true), (3000.0, true)] {
        let mut r = reactor(&[]);
        r.add(Column::new(Kind::Blank, -1, 0));
        let i = r.add(rod_at(0, hull));
        r.add(Column::new(Kind::Moderator, 1, 0));
        let gone = r.remove(i);
        assert_eq!(gone.is_some(), melts, "hull {hull}");
        // the order of the rest and the lookup survive
        assert_eq!(r.columns.iter().map(|c| c.x).collect::<Vec<_>>(), [-1, 1]);
        assert_eq!(
            (r.at(-1, 0), r.at(1, 0), r.at(0, 0)),
            (Some(0), Some(1), None)
        );
    }
}

#[test]
fn removal_is_quiet_when_meltdowns_are_off_or_there_is_no_fuel() {
    let mut r = reactor(&[("dialDisableMeltdowns", "true")]);
    let i = r.add(rod_at(0, 5000.0));
    assert!(r.remove(i).is_none());
    let mut r = reactor(&[]);
    let i = r.add(Column::new(Kind::Rod, 0, 0));
    assert!(r.remove(i).is_none());
}

#[test]
fn removed_columns_drop_their_pending_streams_and_neighbour_caches() {
    let mut r = reactor(&[]);
    r.add(Column::new(Kind::Blank, 0, 0));
    let a = r.add(rod_at(1, 20.0));
    let b = r.add(rod_at(2, 20.0));
    r.add(Column::new(Kind::Cooler, 3, 0));
    for i in [a, b] {
        r.columns[i].rod_mut().unwrap().flux_quantity = 100.0;
    }
    r.step();
    assert_eq!(r.pending_streams(), 8);
    r.remove(a);
    assert_eq!(r.pending_streams(), 4);
    r.step(); // the survivor and the cooler (its cache points past the shifted indices) still run
    assert_eq!(r.columns.len(), 3);
}

#[test]
fn storage_columns_only_exchange_heat() {
    let mut r = reactor(&[]);
    let mut hot = Column::new(Kind::Blank, 0, 0);
    hot.heat = 400.0;
    r.add(hot);
    let s = r.add(Column::new(Kind::Storage, 1, 0));
    r.columns[s].heat = 20.0;
    r.step();
    assert!(r.columns[s].heat > 20.0);
}

#[test]
fn outgasser_output_takes_the_host_stack_limit() {
    use ntm_core::rbmk::outgasser::Stack;
    let mut r = reactor(&[]);
    let rod = RodType::by_variable("rbmk_fuel_po210be").unwrap();
    let mut source = Column::new(Kind::Rod, 0, 0);
    source.rod_mut().unwrap().fuel = Some((rod, RodStack::fresh(rod)));
    r.add(source);
    let mut o = Column::new(Kind::Outgasser, 1, 0);
    let og = o.outgasser_mut().unwrap();
    og.progress = 9_990.0;
    og.input = Some(Stack {
        item: "minecraft_1_7_10:brown_mushroom".into(),
        count: 2,
        meta: 0,
        max_stack: 64,
    });
    let i = r.add(o);
    r.max_stack.insert("mush".into(), 16);
    r.step();
    r.step();
    let out = r.columns[i]
        .outgasser()
        .unwrap()
        .output
        .clone()
        .expect("a mush was made");
    assert_eq!((out.item.as_str(), out.max_stack), ("mush", 16));
}
