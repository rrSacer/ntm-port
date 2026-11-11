//! R3 extension of the save verifier: a home record and a destination record can reach disk separately.
//! Each tick runs the production Server/flush, not a copy of the replay guard. The original verifier covers
//! vanilla item conservation; this extension checks that a destination that saved the output never loses it
//! to an earlier explosion replayed from an older home. The control is that saved vanilla output (one item).
use ntm_core::{
    Pos,
    kv::{Kv, MemKv},
    server::{CommittedEffects, Server, ServerHost, WorldEffect},
};
use std::collections::{BTreeSet, HashMap, VecDeque};

const DIM: &str = "minecraft:overworld";
const HOME: Pos = [15, 64, 0];
const DEST: Pos = [16, 65, 0];
const SERIAL: u64 = 700;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
struct Destination {
    seen: bool,
    items: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Write {
    Home(u8),
    Destination(Destination),
}
impl Write {
    fn same_target(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Home(_), Self::Home(_)) | (Self::Destination(_), Self::Destination(_))
        )
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct State {
    home: u8,
    disk_home: u8,
    destination: Destination,
    disk_destination: Destination,
    writes: Vec<Write>,
    refused: bool,
    destination_out: bool,
}
struct Host<'a> {
    kv: MemKv,
    state: &'a mut State,
}
impl Kv for Host<'_> {
    fn get(&mut self, k: &str) -> Option<Vec<u8>> {
        self.kv.get(k)
    }
    fn set(&mut self, k: &str, v: &[u8]) -> Result<(), String> {
        self.kv.set(k, v)
    }
    fn delete(&mut self, k: &str) {
        self.kv.delete(k);
    }
}
impl ServerHost for Host<'_> {
    fn get_block(&mut self, _: &str, _: Pos) -> Option<String> {
        Some("minecraft:air".into())
    }
    fn set_blocks(&mut self, _: &str, _: &[(Pos, String)]) -> Result<(), String> {
        Ok(())
    }
    fn tell(&mut self, _: &str, _: &str) {}
    fn log(&mut self, _: &str) {}
    fn explode(&mut self, _: &str, _: [f64; 3], _: f32, _: bool, _: bool) -> bool {
        if self.state.refused {
            return false;
        }
        // A replay over a saved ordinary item would destroy it. Vanilla's saved control still has one.
        if !self.state.destination_out {
            self.state.destination.items = 0;
        }
        true
    }
    fn effect_seen(&mut self, id: u64, _: &str, pos: Pos) -> bool {
        !self.state.destination_out
            && pos == DEST
            && id == ntm_core::ledger::effect_id(SERIAL, 1)
            && self.state.destination.seen
    }
    fn apply_items(
        &mut self,
        id: u64,
        _: &str,
        pos: Pos,
        _: &[(String, u32)],
        _: &[(String, Pos)],
    ) -> bool {
        if self.state.destination_out {
            return false;
        }
        assert_eq!((id, pos), (ntm_core::ledger::effect_id(SERIAL, 1), DEST));
        if !self.state.destination.seen {
            self.state.destination.items += 1;
            self.state.destination.seen = true;
        }
        true
    }
    fn store_obligations(
        &mut self,
        _: &str,
        _: [i32; 2],
        _: &[ntm_core::persist::Removal],
        batches: &[CommittedEffects],
        _: &BTreeSet<ntm_core::persist::Obligation>,
    ) -> bool {
        self.state.home = batches.first().map_or(2, |b| b.done as u8);
        true
    }
}
fn tick(state: &mut State) {
    if state.home == 2 {
        return;
    }
    let effects = [
        WorldEffect::Explosion {
            dim: DIM.into(),
            at: [15.5, 65., 0.5],
            power: 5.,
            fire: false,
            breaks: false,
        },
        WorldEffect::Items {
            dim: DIM.into(),
            pos: DEST,
            items: vec![("ntm:sawblade".into(), 1)],
        },
    ];
    let batch = CommittedEffects {
        serial: SERIAL,
        done: state.home.into(),
        owners: vec![(DIM.into(), HOME)],
        effects: effects
            .into_iter()
            .skip(state.home.into())
            .collect::<VecDeque<_>>(),
        ..Default::default()
    };
    let mut host = Host {
        kv: MemKv::default(),
        state,
    };
    let mut server = Server::native(&mut host, 0);
    server.native_obligations_loaded(DIM, [0, 0], vec![], vec![batch], BTreeSet::new());
    server.native_settle(&mut host);
}
#[derive(Clone, Copy, Debug)]
enum Op {
    Tick,
    Capture(bool),
    Write,
    Kill,
    Refuse,
    DestinationOut,
}
fn step(s: &mut State, op: Op) {
    match op {
        Op::Tick => tick(s),
        Op::Capture(home_first) => {
            // Snapshot between ticks, then one FIFO writer; newer queued records of a target coalesce.
            let home = Write::Home(s.home);
            let dest = Write::Destination(s.destination);
            s.writes.extend(if home_first {
                [home, dest]
            } else {
                [dest, home]
            });
        }
        Op::Write => {
            let write = s.writes.remove(0);
            if !s.writes.iter().any(|w| write.same_target(*w)) {
                match write {
                    Write::Home(cursor) => s.disk_home = cursor,
                    Write::Destination(dest) => s.disk_destination = dest,
                }
            }
        }
        Op::Kill => {
            s.writes.clear();
            s.home = s.disk_home;
            s.destination = s.disk_destination;
        }
        Op::Refuse => s.refused = !s.refused,
        Op::DestinationOut => s.destination_out = !s.destination_out,
    }
}
fn explore(
    s: State,
    left: usize,
    seen: &mut HashMap<(State, usize), u128>,
    path: &mut Vec<Op>,
) -> Result<u128, Vec<Op>> {
    if s.destination.seen && s.destination.items != 1 {
        return Err(path.clone());
    }
    if left == 0 {
        return Ok(1);
    }
    if let Some(count) = seen.get(&(s.clone(), left)) {
        return Ok(*count);
    }
    let mut ops = vec![
        Op::Tick,
        Op::Capture(true),
        Op::Capture(false),
        Op::Kill,
        Op::Refuse,
        Op::DestinationOut,
    ];
    if !s.writes.is_empty() {
        ops.push(Op::Write);
    }
    let mut count = 0;
    for op in ops {
        let mut next = s.clone();
        step(&mut next, op);
        path.push(op);
        count += explore(next, left - 1, seen, path)?;
        path.pop();
    }
    seen.insert((s, left), count);
    Ok(count)
}
#[test]
fn saved_outputs_survive_an_older_homes_explosion_replay() {
    let depth = std::env::var("NTM_SAVE_MODEL_DEPTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let mut seen = HashMap::new();
    let count = explore(State::default(), depth, &mut seen, &mut vec![]).unwrap_or_else(|path| {
        panic!("R3: saved vanilla control has one item, replay lost it: {path:?}")
    });
    println!(
        "explosion order: depth {depth}, {} distinct states, {count} complete sequences",
        seen.len()
    );
}
