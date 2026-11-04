//! HBM sound ids and the positional loops derived from confirmed machine state.
//! Legacy categories live beside the converted definitions, never inferred from
//! a `ntm:` prefix. K2 callers can look up their original Java key unchanged.
use crate::{
    combustion::Combustion,
    crafter::Station,
    heater::Heater,
    machine::{Data, Machine},
    steam::{Shape, Steam},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};
mod motion;
pub use motion::Motion;

#[derive(Deserialize)]
struct Events {
    events: BTreeMap<String, String>,
    categories: BTreeMap<String, String>,
}
fn events() -> &'static Events {
    static EVENTS: OnceLock<Events> = OnceLock::new();
    EVENTS.get_or_init(|| {
        serde_json::from_str(include_str!("../data/sound-events.json")).expect("sound event table")
    })
}
/// The exact Java key, including its original case; absent means not imported.
pub fn event(java: &str) -> Option<&'static str> {
    events().events.get(java).map(String::as_str)
}
pub fn category(event: &str) -> Option<&'static str> {
    events().categories.get(event).map(String::as_str)
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Envelope {
    Plain,
    Centrifuge,
    Turbofan,
    Leviathan,
    Industrial,
    Assembler,
    Shredder,
    Refinery,
    ArcLid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loop {
    pub slot: u8,
    pub value: f64,
    pub event: String,
    pub volume: f32,
    pub pitch: f32,
    /// AudioDynamic has its own linear falloff, independent of maxVolume.
    pub range: f32,
    /// Centrifuges ramp +2/-3 to 60, with pitch (duration-10)/100 + 0.5.
    pub envelope: Envelope,
    pub offset: [f64; 3],
}
fn sound(java: &str, volume: f32, pitch: f32, range: f32) -> Loop {
    Loop {
        slot: 0,
        value: 0.,
        event: event(java).expect("imported HBM event").into(),
        volume,
        pitch,
        range,
        envelope: Envelope::Plain,
        offset: [0.; 3],
    }
}

/// Read on the server before publishing its existing machine JSON. Transient
/// `on` fields deliberately stay out of persistence; only their audio is sent.
pub fn machine(m: &Machine) -> Vec<Loop> {
    if let Data::ArcFurnace(f) = &m.data {
        let mut lid = sound("hbm:door.wgh_start", 0.75, 1., 15.);
        lid.envelope = Envelope::ArcLid;
        lid.value = f64::from(f.lid) / f64::from(crate::arc_furnace::LID);
        let mut progress = sound(
            "hbm:block.electricHum",
            if f.progressing { 1.5 } else { 0. },
            0.75,
            15.,
        );
        progress.slot = 1;
        return vec![lid, progress];
    }
    let mut s = match &m.data {
        Data::Crafter(c) => {
            let (id, volume, pitch, range) = match c.station {
                Station::Assembly => ("hbm:block.motor", 0.5, 0.75, 15.),
                Station::PyroOven => ("hbm:block.pyroOperate", 1., 1., 15.),
                Station::Centrifuge => ("hbm:block.centrifugeOperate", 1., 1., 10.),
                Station::ChemicalPlant => ("hbm:block.chemicalPlant", 1., 1., 15.),
                Station::Purex | Station::Crystallizer => {
                    ("hbm:block.chemicalPlant", 1., 0.75, 15.)
                }
                _ => return vec![],
            };
            // Keep a stopped centrifuge descriptor long enough for its client
            // ramp-down; zero volume marks the server's isProgressing=false.
            if !c.running && !matches!(c.station, Station::Centrifuge | Station::Assembly) {
                return vec![];
            }
            let mut s = sound(id, if c.running { volume } else { 0. }, pitch, range);
            s.envelope = match c.station {
                Station::Centrifuge => Envelope::Centrifuge,
                Station::Assembly => Envelope::Assembler,
                _ => Envelope::Plain,
            };
            Some(s)
        }
        Data::GasCent(c) => {
            let mut s = sound(
                "hbm:block.centrifugeOperate",
                if c.progress > 0 { 1. } else { 0. },
                1.,
                10.,
            );
            s.envelope = Envelope::Centrifuge;
            Some(s)
        }
        Data::Processor(p) if p.shape == crate::processing::Shape::Vacuum => {
            let mut s = sound("hbm:block.boiler", if p.on { 1. } else { 0. }, 1., 15.);
            s.envelope = Envelope::Refinery;
            Some(s)
        }
        Data::Refinery(r) => {
            let mut s = sound("hbm:block.boiler", if r.on { 1. } else { 0. }, 1., 15.);
            s.envelope = Envelope::Refinery;
            Some(s)
        }
        Data::Shredder(shredder) => Some(Loop {
            slot: 0,
            value: 0.,
            event: "minecraft:entity.minecart.riding".into(),
            volume: if shredder.progress > 0 { 1. } else { 0. },
            pitch: 0.75,
            range: 16.,
            envelope: Envelope::Shredder,
            offset: [0.; 3],
        }),
        Data::Combustion(c) => match &**c {
            Combustion::Diesel(d) if d.was_on => {
                Some(sound("hbm:block.engine", 1., 1., 10.))
            }
            Combustion::Engine(e) if e.was_on => {
                Some(sound("hbm:block.igeneratorOperate", 1., 1., 10.))
            }
            // Java momentum is a client interpolation; the owner applies it.
            Combustion::Turbofan(f) => {
                let mut s = sound(
                    "hbm:block.turbofanOperate",
                    if f.was_on { 1. } else { 0. },
                    f.afterburner as f32 * 0.16,
                    50.,
                );
                s.envelope = Envelope::Turbofan;
                Some(s)
            }
            Combustion::Turbine(t) if t.rpm >= 10 && t.state != -1 => Some(sound(
                "hbm:block.turbinegasRunning",
                2.,
                0.55 + 0.01 * t.rpm as f32,
                20.,
            )),
            _ => None,
        },
        Data::Heater(h) => match &**h {
            // createAudioLoop's 0.25 is immediately updated to getVolume(1F).
            Heater::Electric(e) if e.on => Some(sound("hbm:block.electricHum", 1., 1., 7.5)),
            _ => None,
        },
        Data::Boiler(b) => {
            let mut s = sound("hbm:block.boiler", if b.on { 1. } else { 0. }, 1., 10.);
            s.envelope = Envelope::Refinery;
            Some(s)
        }
        Data::Steam(s) => match &**s {
            Steam::Turbine(t) if t.on || t.flywheel > 0 || t.shape == Shape::Chungus => {
                match t.shape {
                    Shape::Chungus => {
                        let mut s = sound(
                            "hbm:block.chungusTurbineRunning",
                            if t.on { 1. } else { 0. },
                            1.,
                            20.,
                        );
                        s.envelope = Envelope::Leviathan;
                        Some(s)
                    }
                    Shape::Industrial => {
                        let spin = (t.flywheel as f64 / crate::steam::FLYWHEEL * 2.).min(1.) as f32;
                        {
                            let mut s = sound(
                                "hbm:block.largeTurbineRunning",
                                0.25 + spin * 0.75,
                                0.5 + spin * 0.5,
                                20.,
                            );
                            s.offset = [0.5; 3];
                            s.envelope = Envelope::Industrial;
                            Some(s)
                        }
                    }
                }
            }
            _ => None,
        },
        _ => None,
    };
    s.take().into_iter().collect()
}

/// AudioDynamic.func: full gain at the source and zero at its explicit range.
pub fn gain(volume: f32, range: f32, distance: f32) -> f32 {
    (volume * (1. - distance / range)).max(0.)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Kind;
    #[test]
    fn java_constant_events_and_boiler_coast_are_preserved() {
        let mut diesel = Machine::new(Kind::Combustion, "ntm:machine_diesel", None);
        let Data::Combustion(c) = &mut diesel.data else { panic!() };
        let Combustion::Diesel(d) = &mut **c else { panic!() };
        d.was_on = true;
        assert_eq!(machine(&diesel)[0].event, "ntm:block.engine");
        let mut chemical = Machine::new(Kind::Crafter, "ntm:machine_chemical_plant", None);
        let Data::Crafter(c) = &mut chemical.data else { panic!() };
        c.station = Station::ChemicalPlant;
        c.running = true;
        assert_eq!(machine(&chemical)[0].event, "ntm:block.chemicalplant");
        // HeatBoiler keeps audioTime=20 after losing its heating precondition.
        let mut boiler = Machine::new(Kind::Boiler, "ntm:heat_boiler", None);
        let Data::Boiler(b) = &mut boiler.data else { panic!() };
        b.on = true;
        let mut motion = Motion::new(1);
        assert_eq!(motion.step(&machine(&boiler)[0]).0, 1.);
        let Data::Boiler(b) = &mut boiler.data else { panic!() };
        b.on = false;
        let stopped = machine(&boiler).remove(0);
        for _ in 0..19 { assert_eq!(motion.step(&stopped).0, 1.); }
        assert_eq!(motion.step(&stopped).0, 0.);
    }
    #[test]
    fn combustion_snapshot_starts_updates_and_stops_the_java_loop() {
        let mut m = Machine::new(Kind::Combustion, "ntm:machine_combustion_engine", None);
        assert!(machine(&m).is_empty());
        let Data::Combustion(c) = &mut m.data else {
            panic!()
        };
        let Combustion::Engine(e) = &mut **c else {
            panic!()
        };
        e.was_on = true;
        let loops = machine(&m);
        assert_eq!(loops[0].event, "ntm:block.igeneratoroperate");
        assert_eq!(
            (loops[0].volume, loops[0].pitch, loops[0].range),
            (1., 1., 10.)
        );
        m.audio = loops;
        let client: Machine = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(
            client.audio, m.audio,
            "transient server state reaches every client in the existing snapshot"
        );
        let Data::Combustion(c) = &mut m.data else {
            panic!()
        };
        let Combustion::Engine(e) = &mut **c else {
            panic!()
        };
        e.was_on = false;
        assert!(machine(&m).is_empty());
    }
    #[test]
    fn nuclear_wave_uses_the_torex_radius_and_stops_after_149_ticks() {
        assert!(nuclear_arrived(1, 3.74));
        assert!(!nuclear_arrived(1, 3.75));
        assert!(nuclear_arrived(149, 336.74));
        assert!(!nuclear_arrived(149, 336.75));
        assert!(!nuclear_arrived(150, 0.));
    }
    #[test]
    fn arc_lid_and_progress_are_independent_instances_in_the_public_snapshot() {
        let mut m = Machine::new(Kind::ArcFurnace, "ntm:machine_arc_furnace_large", None);
        let Data::ArcFurnace(f) = &mut m.data else {
            panic!()
        };
        f.progressing = true;
        f.lid = 500;
        m.audio = machine(&m);
        let audio = snapshot(&serde_json::to_string(&m).unwrap());
        assert_eq!(audio.len(), 2);
        assert_eq!(audio[0].event, "ntm:door.wgh_start");
        assert_eq!(audio[1].event, "ntm:block.electrichum");
        assert_ne!(audio[0].slot, audio[1].slot);
    }
    #[test]
    fn installed_upgrades_do_not_beep_on_load_or_after_generation_reuse() {
        let m = Machine::new(Kind::ArcFurnace, "ntm:machine_arc_furnace_large", None);
        let mut before = mark(&m);
        let mut slots = vec![None; 30];
        slots[4] = Some(crate::server::container_bridge::Stack::new(
            "ntm:upgrade_speed_1",
            1,
        ));
        with_slots(&mut before, &m.block, &slots);
        assert!(changed(&before, &before).is_empty());
        slots[4] = Some(crate::server::container_bridge::Stack::new(
            "ntm:upgrade_speed_2",
            1,
        ));
        let mut after = mark(&m);
        with_slots(&mut after, &m.block, &slots);
        assert_eq!(changed(&before, &after)[0].event, "ntm:item.upgradeplug");
        after.generation = 1;
        assert!(changed(&before, &after).is_empty());
    }
    #[test]
    fn audio_dynamic_range_does_not_scale_with_volume() {
        assert_eq!(gain(0.5, 10., 0.), 0.5);
        assert_eq!(gain(0.5, 10., 5.), 0.25);
        assert_eq!(gain(0.5, 10., 10.), 0.);
        assert_eq!(gain(0.5, 10., 20.), 0.);
    }
}

/// A one-shot caused by a successful machine transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub event: &'static str,
    pub volume: f32,
    pub pitch: f32,
    pub offset: [f64; 3],
}
fn cue(java: &str, volume: f32, pitch: f32, offset: [f64; 3]) -> Cue {
    Cue {
        event: event(java).expect("imported HBM cue"),
        volume,
        pitch,
        offset,
    }
}
/// Called only after a machine commit succeeds, never for refused or unloaded
/// state. Keeping playback outside the journal avoids replaying old one-shots.
#[derive(Clone, Debug, Default)]
pub struct Mark {
    generation: u64,
    press: Option<(bool, Option<(String, u32)>)>,
    turbine: Option<(i32, i32)>,
    warning: u32,
    loader: Option<f64>,
    steam_hit: Option<f32>,
    upgrades: Vec<Option<crate::server::container_bridge::Stack>>,
    upgrade_offset: [f64; 3],
}
pub fn mark(machine: &Machine) -> Mark {
    let mut mark = Mark {
        generation: machine.generation,
        ..Mark::default()
    };
    let press = match &machine.data {
        Data::BurnerPress(p) => Some(&p.core),
        Data::ElectricPress(p) => Some(&p.core),
        _ => None,
    };
    mark.press = press.map(|p| (p.retracting, p.input.clone()));
    if let Data::Combustion(c) = &machine.data
        && let Combustion::Turbine(t) = &**c
    {
        mark.turbine = Some((t.state, t.counter));
    }
    if let Data::SolarStirling(s) = &machine.data
        && let crate::solar_stirling::SolarStirling::Stirling(s) = &**s
    {
        mark.warning = s.warn_cooldown;
    }
    if let Data::Pile(p) = &machine.data
        && let crate::pile::Pile::Loader(l) = &**p
    {
        mark.loader = Some(l.level);
    }
    if let Data::Steam(s) = &machine.data
        && let Steam::Engine(e) = &**s
        && e.sound_hit
    {
        mark.steam_hit = Some(0.5 + e.acceleration / 80.);
    }
    mark
}
pub fn changed(before: &Mark, after: &Mark) -> Vec<Cue> {
    if before.generation != after.generation {
        return vec![];
    }
    let mut out = Vec::new();
    for (old, new) in before.upgrades.iter().zip(&after.upgrades) {
        if old != new
            && new
                .as_ref()
                .is_some_and(|s| s.id.starts_with("ntm:upgrade_"))
        {
            out.push(cue("hbm:item.upgradePlug", 1., 1., after.upgrade_offset));
        }
    }
    if let Some(pitch) = after.steam_hit {
        out.push(cue("hbm:block.steamEngineOperate", 1., pitch, [0.; 3]));
    }
    if let (Some((old, old_input)), Some((new, new_input))) = (&before.press, &after.press)
        && *new
        && !old
        && new_input != old_input
    {
        out.push(cue("hbm:block.pressOperate", 1.5, 1., [0.; 3]));
    }
    if let (Some((_, old)), Some((state, new))) = (before.turbine, after.turbine) {
        if old != 50 && new == 50 && state == -1 {
            out.push(cue("hbm:block.turbinegasStartup", 1., 1., [0., 2., 0.]));
        }
        if old == 225 && new == 224 && state == 0 {
            out.push(cue("hbm:block.turbinegasShutdown", 1., 1., [0., 2., 0.]));
        }
    }
    if after.warning == 100 && before.warning != 100 {
        out.push(cue("hbm:block.warnOverspeed", 2., 1., [0.5, 1., 0.5]));
    }
    if let (Some(old), Some(new)) = (before.loader, after.loader) {
        if old == 0. && new > 0. {
            out.push(cue("hbm:weapon.reload.boltOpen", 1., 1., [0.5; 3]));
        }
        if old == 1. && new < 1. {
            out.push(cue("hbm:weapon.reload.boltOpen", 1., 0.75, [0.5; 3]));
        }
    }
    out
}

#[derive(Serialize, Deserialize)]
pub struct NuclearCue {
    pub dimension: String,
    pub at: [f64; 3],
}
/// EntityNukeTorex's client wave, not a generic fixed speed-of-sound timer.
pub fn nuclear_arrived(age: u32, distance: f64) -> bool {
    age < 150 && distance < (f64::from(age) * 1.5 + 1.) * 1.5
}

/// Ambient one-shots emitted by the source's server-side processing branches.
pub fn ambient(machine: &Machine, seed: i64) -> Vec<Cue> {
    let mut random = crate::bedrock::JavaRandom::new(seed);
    match &machine.data {
        Data::Firebox(f) if f.on && random.next_int(15) == 0 => vec![Cue {
            event: "minecraft:block.fire.ambient",
            volume: 1.,
            pitch: 0.5 + random.next_float() * 0.5,
            offset: [0.; 3],
        }],
        Data::Boiler(b) if b.on && random.next_int(400) == 0 => {
            vec![cue("hbm:block.boilerGroan", 0.5, 1., [0.5, 2., 0.5])]
        }
        Data::BlastFurnace(f) if f.working && random.next_int(10) == 0 => vec![Cue {
            event: "minecraft:block.fire.ambient",
            volume: 1.,
            pitch: 0.5 + random.next_float() * 0.25,
            offset: [0.; 3],
        }],
        Data::Combustion(c) => match &**c {
            Combustion::Turbofan(f)
                if f.was_on && f.afterburner > 90 && random.next_int(30) == 0 =>
            {
                vec![cue(
                    "hbm:block.damage",
                    3.,
                    0.95 + random.next_float() * 0.2,
                    [0.5, 1.5, 0.5],
                )]
            }
            _ => vec![],
        },
        _ => vec![],
    }
}

/// Read only the public audio part of a saved machine; serde skips private
/// inventory/process fields without building a second copy of their JSON.
pub fn snapshot(json: &str) -> Vec<Loop> {
    #[derive(Deserialize)]
    struct Snapshot {
        #[serde(default)]
        audio: Vec<Loop>,
    }
    serde_json::from_str::<Snapshot>(json).map_or_else(|_| vec![], |v| v.audio)
}

pub fn upgrade_slots(block: &str) -> Option<(std::ops::Range<usize>, [f64; 3])> {
    match block {
        "ntm:machine_arc_furnace_large" => Some((4..5, [0.5; 3])),
        "ntm:machine_crystallizer" => Some((5..7, [0.5; 3])),
        "ntm:machine_pyrooven" => Some((4..6, [0.5; 3])),
        "ntm:machine_oil_well" | "ntm:machine_pumpjack" | "ntm:machine_fracking_tower" => {
            Some((5..8, [0.5, 1.5, 0.5]))
        }
        _ => None,
    }
}
/// Sound for setting an upgrade slot is independent of whether that upgrade's
/// processing modifier is implemented. Baseline loaded slots never beep.
pub fn with_slots(mark: &mut Mark, block: &str, slots: &crate::server::container_bridge::Slots) {
    if let Some((range, offset)) = upgrade_slots(block) {
        mark.upgrades = range.map(|i| slots.get(i).cloned().flatten()).collect();
        mark.upgrade_offset = offset;
    }
}
