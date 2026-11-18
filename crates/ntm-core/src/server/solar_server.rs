//! Batch 4's world inputs. Mirrors and boilers run in the world's stable
//! machine order: a mirror after its boiler leaves heat for the next tick.
use super::*;
use crate::{
    machine::{Data, MachineEffect},
    solar_stirling::SolarStirling,
};

impl Server {
    pub(super) fn solar_tick(&mut self, host: &mut impl ServerHost) {
        let r = &mut self.ready;
        let keys: Vec<Key> = r
            .world
            .machines
            .iter()
            .filter(|(k, m)| {
                r.loaded.get(*k) == Some(&true) && matches!(m.data, Data::SolarStirling(_))
            })
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            let Some(Data::SolarStirling(s)) = r.world.machines.get_mut(&key).map(|m| &mut m.data)
            else {
                continue;
            };
            match &mut **s {
                SolarStirling::Boiler(b) => b.step(),
                SolarStirling::Mirror(m) => {
                    let sun = m.sun(key.1[1], host.solar_sample(&key.0, key.1));
                    let target = (key.0.clone(), [m.target[0], m.target[1] - 1, m.target[2]]);
                    if sun > 0
                        && r.loaded.get(&target) == Some(&true)
                        && let Some(Data::SolarStirling(s)) =
                            r.world.machines.get_mut(&target).map(|m| &mut m.data)
                        && let SolarStirling::Boiler(b) = &mut **s
                    {
                        b.heat = b.heat.wrapping_add(sun);
                    }
                }
                SolarStirling::Stirling(_) => {}
            }
        }
    }
    /// `finish` of every loaded Stirling. The tick that breaks the gear also explodes
    /// (TileEntityStirling.java:92, `newExplosion(null, x+0.5, y+1, z+0.5, 5F, false, false)`): that
    /// step is committed with its blast as one batch, before this tick's native_save. The gear stays
    /// lost, without the EntityCog the Java throws (the nominal defect of the Stirling's doc).
    pub(super) fn stirling_finish(&mut self, host: &mut impl ServerHost) {
        let r = &mut self.ready;
        let mut failing = Vec::new();
        for (k, m) in &mut r.world.machines {
            if r.loaded.get(k) == Some(&true)
                && let Data::SolarStirling(s) = &mut m.data
                && let SolarStirling::Stirling(s) = &mut **s
            {
                if s.fails_on_finish() {
                    failing.push(k.clone());
                } else {
                    s.finish();
                }
            }
        }
        for key in failing {
            let published = self.publish_machine_effects(host, &key, |m| {
                let Data::SolarStirling(s) = &mut m.data else { return Vec::new() };
                let SolarStirling::Stirling(s) = &mut **s else { return Vec::new() };
                s.finish();
                vec![MachineEffect::Explode { offset: [0., 1., 0.], power: 5., fire: false, breaks: false }]
            });
            // Refused: the live model keeps its gear and count and fails on the next hot tick. Only the
            // heat of this tick is spent, as `finish` spends it every tick (it is never saved).
            if !published
                && let Some(Data::SolarStirling(s)) = self.ready.world.machines.get_mut(&key).map(|m| &mut m.data)
                && let SolarStirling::Stirling(s) = &mut **s
            {
                s.heat = 0;
                host.log(&format!("ntm: the failing Stirling at {:?} was not committed; it fails again on its next hot tick", key.1));
            }
        }
    }
    /// The mirror tool stores the core one block up, even when a dummy was clicked.
    pub fn native_solar_target(&self, dim: &str, core: Pos) -> Option<Pos> {
        let m = self.ready.world.machines.get(&(dim.into(), core))?;
        matches!(&m.data, Data::SolarStirling(s) if matches!(&**s, SolarStirling::Boiler(_)))
            .then_some([core[0], core[1] + 1, core[2]])
    }
    pub fn native_mirror_link(
        &mut self,
        dim: &str,
        at: Pos,
        target: Pos,
    ) -> Option<Result<(), &'static str>> {
        let m = self.ready.world.machines.get_mut(&(dim.into(), at))?;
        let Data::SolarStirling(s) = &mut m.data else {
            return None;
        };
        let SolarStirling::Mirror(m) = &mut **s else {
            return None;
        };
        Some(m.link(at, target))
    }
    pub fn native_stirling_gear(&self, dim: &str, core: Pos) -> Option<&'static str> {
        let m = self.ready.world.machines.get(&(dim.into(), core))?;
        let Data::SolarStirling(s) = &m.data else {
            return None;
        };
        let SolarStirling::Stirling(s) = &**s else {
            return None;
        };
        if s.has_cog { None } else { s.shape.gear() }
    }
    /// The native caller takes the matching hand item only after this atomic
    /// BE update succeeds, while retaining exclusive access to the same Game.
    pub fn native_stirling_repair(&mut self,host:&mut impl ServerHost,dim:&str,core:Pos,gear:&str)->bool {
        if self.native_stirling_gear(dim,core)!=Some(gear){return false;}
        let key=(dim.to_string(),core);
        if self.ready.broken.contains_key(&key)||self.ready.world.machines.get(&key).is_some_and(|m|!self.model_current(&key,m)){return false;}
        let Some(mut machine)=self.ready.world.machines.get(&key).cloned() else{return false};
        if !host.get_block(dim,core).is_some_and(|b|blockstate::id_of(&b)==machine.block){return false;}
        let Some(expected)=host.machine_slots(dim,core) else{return false};
        let Data::SolarStirling(s)=&mut machine.data else{return false};
        let SolarStirling::Stirling(s)=&mut **s else{return false};
        s.has_cog=true;
        let data=Self::machine_screen(&mut machine);
        let json=serde_json::to_string(&machine).expect("machine serialization");
        if !host.commit_machine(&MachineCommit{cells:Vec::new(),guard:CellGuard::machine(&key,&machine),expected_saved:self.ready.native_saved.contains_key(&key),dim:dim.into(),pos:core,expected:expected.clone(),desired:expected.clone(),data:data.clone(),json:json.clone()}){return false;}
        self.ready.world.machines.insert(key.clone(),machine);
        self.ready.bridge.last.insert(key.clone(),expected);
        self.ready.bridge.data.insert(key.clone(),data);
        self.ready.native_saved.insert(key,json);
        host.show(dim,&crate::blast::visuals::Show::Sound{id:"ntm:item.upgradeplug",source:"player",at:core.map(|n|f64::from(n)+0.5),volume:1.5,pitch:0.75});
        true
    }
}
