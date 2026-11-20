//! T11: the RBMK storage's queue (`TileEntityRBMKStorage`) and the crane (`TileEntityCraneConsole`): what it loads and
//! unloads, and each transfer as one publication of both blocks.
use super::*;
use container_bridge::Stack;

const Y: i32 = 71;
const STORAGE: Pos = [0, Y, 0];

fn column(id: &str) -> String {
    format!("ntm:{id}[facing=north,honey_level=0]")
}

fn slots(g: &mut Game, p: Pos) -> Vec<Option<String>> {
    g.world.borrow().engine.get(&(OW.to_string(), p)).map(|e| e.0.iter().map(|s| s.as_ref().map(|s| s.id.clone())).collect()).unwrap_or_default()
}

/// Every 10 world ticks (in the column's own commit) one pass of the queue: each rod one place towards slot 0.
#[test]
fn the_storage_moves_its_queue_every_ten_ticks() {
    let mut g = game();
    g.place(STORAGE, &column("rbmk_storage"));
    g.run(2);
    let mut s: Vec<Option<Stack>> = vec![None; 12];
    s[2] = Some(Stack::new("ntm:rbmk_fuel_ueu", 1));
    s[11] = Some(Stack::new("ntm:rbmk_fuel_po210be", 1));
    g.world.borrow_mut().engine.insert((OW.to_string(), STORAGE), (s, Vec::new()));
    // Step by step: the queue changes by exactly one pass, once every 10 ticks (the phase is the port's clock's, as the
    // console's scan: `world.ticks`, not `getTotalWorldTime`).
    let mut passes = Vec::new();
    for _ in 0..25 {
        let (time, before) = (g.ready().world.ticks, slots(&mut g, STORAGE));
        g.run(1);
        let after = slots(&mut g, STORAGE);
        let mut expected = before.clone();
        crate::rbmk::crane::storage_pass(&mut expected);
        if after != before {
            assert_eq!(after, expected, "one pass at {time}");
            passes.push(time);
        }
    }
    assert!(passes.len() >= 2 && passes.windows(2).all(|w| w[1] - w[0] == 10), "{passes:?}");
    let end = slots(&mut g, STORAGE);
    assert_eq!((end[0].as_deref(), end[9].as_deref()), (Some("ntm:rbmk_fuel_ueu"), Some("ntm:rbmk_fuel_po210be")), "{end:?}");
}
