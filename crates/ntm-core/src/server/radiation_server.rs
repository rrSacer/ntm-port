//! Radiation on the players (docs/radiacao.md, stage L4): once a second
//! each player's rate is what they carry. Past the joining grace the dose
//! grows by it, and their client's Geiger reads both values. Sickness and death act every tick (`sickness_tick`).
use super::*;
use crate::radiation::{self, Harm};

/// Ticks between the players' rates.
pub const RATE_EVERY: u64 = 20;

impl Server {
    pub fn player_dose(&self,id:&str)->radiation::Dose {self.ready.world.radiation.get(id).cloned().unwrap_or_default()}
    pub(super) fn update_player_dose<T>(&mut self,host:&mut impl ServerHost,id:&str,change:impl FnOnce(&mut radiation::Dose)->T)->Option<T> {
        let expected=host.player_dose(id,&self.player_dose(id))?;
        let mut desired=expected.clone();
        let result=change(&mut desired);
        if !host.commit_player_dose(id,&expected,&desired){return None;}
        self.ready.world.radiation.insert(id.into(),desired);
        Some(result)
    }
    /// `HbmLivingProps.radBuf`: the exposure the player had in the last
    /// second, in milliRAD/s, as the radiation tick measured it (the same
    /// numbers that move the dose). 0 before the first reading.
    pub fn rad_buf(&self, id: &str) -> i64 {
        self.radiation_shown.get(id).map_or(0, |s| s.1)
    }
    /// `ContaminationUtil.contaminate(.., RADIATION, .., amount)` adds the raw
    /// amount to `radEnv` before any gate (creative, armor, immunity), so a
    /// bottle's 5 RAD reaches the next reading even when it changes no dose.
    /// A direct `incrementRadiation` does not call this.
    pub fn note_contamination(&mut self, id: &str, millirad: i64) {
        let env = self.radiation_env.entry(id.into()).or_insert(0);
        *env = env.saturating_add(millirad);
    }
    /// `ContaminationUtil.contaminate:287`: a player who joined or respawned less than 200 ticks ago
    /// (`ticksExisted < 200`) takes no dose; `blasts.entered` drops the dead, so a respawn starts it again.
    pub(super) fn in_grace(&self, id: &str) -> bool {
        self.blasts.entered.get(id).is_none_or(|t| self.tick.saturating_sub(*t) < radiation::GRACE_TICKS)
    }
    pub(super) fn expose_player(&mut self,host:&mut impl ServerHost,id:&str,amount:u64) {
        let factor=host.radiation_factor(id);
        let amount=(amount as f64*factor) as u64;
        self.update_player_dose(host,id,|dose|radiation::add_dose(dose,amount));
    }
    /// `EntityEffectHandler.handleRadiationEffect:221-258`, every tick, for each non-creative player: from 1000 RAD
    /// radiation kills them (`radiation::death`, decision D2 of slice 5) and draws nothing; between 200 and 1000 they
    /// roll their band's effects (`radiation::sickness_roll`) on the world's sickness random (decision D1). It is
    /// seeded from the world's seed the first time someone is sick; a tick with nobody in a band draws nothing and
    /// writes nothing. The Java has no joining grace here (that is `contaminate`'s `ticksExisted`).
    pub(super) fn sickness_tick(&mut self, host: &mut impl ServerHost) {
        // The bands' edges are radiation::death's and radiation::sickness_roll's alone.
        let mut alive = Vec::new();
        for p in self.players.clone().iter().filter(|p| !p.creative) {
            let Some(dose) = host.player_dose(&p.id, &self.player_dose(&p.id)) else { continue };
            if radiation::death(&mut dose.clone()).is_none() {
                alive.push((p.id.clone(), dose));
            } else if let Some(Some(Harm::Hurt { amount, kind })) = self.update_player_dose(host, &p.id, radiation::death) {
                host.hurt(&p.id, amount, kind);
            }
        }
        if alive.is_empty() {
            return;
        }
        let mut rng = match &*self.ready.world.sickness {
            Some(s) => s.rng(),
            None => crate::rbmk::jrandom::JavaRandom::new(host.world_seed()),
        };
        let before = rng.state();
        for (id, dose) in alive {
            for h in radiation::sickness_roll(&dose, &mut rng) {
                if let Harm::Effect { id: effect, amplifier, ticks } = h {
                    host.effect(&id, effect, ticks, amplifier);
                }
            }
        }
        // Kept only when something was drawn: a tick with nobody in a band writes nothing.
        if rng.state() != before {
            let mut kept = radiation::SicknessRng { seed: 0, gaussian: None };
            kept.set_rng(&rng);
            *self.ready.world.sickness = Some(kept);
        }
    }
    /// `TileEntityDecon.updateEntity`, every tick: each decontaminator whose
    /// box reaches a player takes 0.5 RAD from them (creative too:
    /// `incrementRadiation` asks nothing), ends their radiation potion
    /// (`HbmPotion.radiation`, `removePotionEffect`) and clears their
    /// contamination list (`HbmLivingProps.getCont`, which the port has not).
    /// Mobs are not reached: the port keeps no dose for them.
    pub(super) fn decon_tick(&mut self, host: &mut impl ServerHost) {
        for p in self.players.clone() {
            let n = radiation::decon_cells(p.pos)
                .into_iter()
                .filter(|&c| host.get_block(&p.dim, c).is_some_and(|b| blockstate::id_of(&b) == radiation::DECON))
                .count();
            if n == 0 {
                continue;
            }
            if self.player_dose(&p.id).millirad > 0 || host.player_dose(&p.id, &self.player_dose(&p.id)).is_some_and(|d| d.millirad > 0) {
                self.update_player_dose(host, &p.id, |dose| radiation::decontaminate(dose, n));
            }
            self.end_irradiation(host, &p.id);
        }
    }
    pub(super) fn radiation_tick(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        if !tick.is_multiple_of(RATE_EVERY) {
            return;
        }
        self.radiation_shown.retain(|id, _| self.players.iter().any(|p| &p.id == id));
        self.radiation_env.retain(|id, _| self.players.iter().any(|p| &p.id == id));
        for p in self.players.clone() {
            let carried = host.inventory_stacks(&p.id).unwrap_or_default();
            let held = carried.iter().fold(0u64,|sum,s|sum.saturating_add(crate::zirnox::radiation_rate(s).or_else(||crate::radioisotopes::radiation_rate(s)).map_or_else(||radiation::rate_of([(s.id.as_str(),s.count)]),|rate|u64::from(rate)*u64::from(s.count))));
            let held = u32::try_from(held).unwrap_or(u32::MAX);
            let grace = self.in_grace(&p.id);
            let protected = (f64::from(held)*host.radiation_factor(&p.id)) as u32;
            let Some(()) = self.update_player_dose(host,&p.id,|dose| {
                if !(p.creative || grace) {radiation::accumulate(dose,protected);}
            }) else {continue};
            let dose = &self.ready.world.radiation[&p.id];
            // `radBuf = radEnv; radEnv = 0` once a second: the continuous
            // sources of this second plus the point contaminations since the last.
            let events = self.radiation_env.remove(&p.id).unwrap_or(0);
            let continuous = i64::from(held.saturating_add(self.ready.zirnox_radiation_rate.get(&p.id).copied().unwrap_or(0)));
            let shown = (i64::try_from(dose.millirad).unwrap_or(i64::MAX), continuous.saturating_add(events));
            let last = self.radiation_shown.get(&p.id).copied();
            if last.map(|l| l.0) != Some(shown.0) {
                host.set_player_value(&p.id, radiation::KEY_DOSE, shown.0);
            }
            if last.map(|l| l.1) != Some(shown.1) {
                host.set_player_value(&p.id, radiation::KEY_RATE, shown.1);
            }
            self.radiation_shown.insert(p.id.clone(), shown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kv::MemKv,server::container_bridge::Stack};
    #[derive(Default)]
    struct Host {kv:MemKv,carried:Vec<Stack>,shown:BTreeMap<String,i64>}
    impl Kv for Host {
        fn get(&mut self,k:&str)->Option<Vec<u8>>{self.kv.get(k)}
        fn set(&mut self,k:&str,v:&[u8])->Result<(),String>{self.kv.set(k,v)}
        fn delete(&mut self,k:&str){self.kv.delete(k)}
    }
    impl ServerHost for Host {
        fn get_block(&mut self,_:&str,_:Pos)->Option<String>{None}
        fn set_blocks(&mut self,_:&str,_:&[(Pos,String)])->Result<(),String>{Ok(())}
        fn tell(&mut self,_:&str,_:&str){}
        fn log(&mut self,_:&str){}
        fn inventory_stacks(&mut self,_:&str)->Option<Vec<Stack>>{Some(self.carried.clone())}
        fn set_player_value(&mut self,_:&str,key:&str,value:i64){self.shown.insert(key.into(),value);}
    }
    #[test]
    fn persistent_dose_wins_over_legacy_and_rejected_commit_publishes_nothing() {
        struct Persistent {inner:Host,dose:Option<crate::radiation::Dose>,reject:bool}
        impl Kv for Persistent {
            fn get(&mut self,k:&str)->Option<Vec<u8>>{self.inner.get(k)}
            fn set(&mut self,k:&str,v:&[u8])->Result<(),String>{self.inner.set(k,v)}
            fn delete(&mut self,k:&str){self.inner.delete(k)}
        }
        impl ServerHost for Persistent {
            fn get_block(&mut self,_:&str,_:Pos)->Option<String>{None}
            fn set_blocks(&mut self,_:&str,_:&[(Pos,String)])->Result<(),String>{Ok(())}
            fn tell(&mut self,_:&str,_:&str){}
            fn log(&mut self,_:&str){}
            fn player_dose(&mut self,_:&str,legacy:&crate::radiation::Dose)->Option<crate::radiation::Dose>{Some(self.dose.clone().unwrap_or_else(||legacy.clone()))}
            fn commit_player_dose(&mut self,_:&str,expected:&crate::radiation::Dose,desired:&crate::radiation::Dose)->bool {
                if self.reject || self.dose.as_ref().is_some_and(|d|d!=expected){return false;}
                self.dose=Some(desired.clone());true
            }
        }
        let mut h=Persistent{inner:Host::default(),dose:None,reject:false};
        let mut s=Server::native(&mut h,0);
        s.ready.world.radiation.insert("p".into(),crate::radiation::Dose{millirad:123_000});
        s.update_player_dose(&mut h,"p",|d|radiation::add_dose(d,7_000));
        assert_eq!(h.dose.as_ref().unwrap().millirad,130_000);
        // Simulate reopening against an older independent world/KV save.
        let mut reopened=Server::native(&mut h,0);
        reopened.update_player_dose(&mut h,"p",|d|d.millirad=d.millirad.saturating_sub(1_000));
        assert_eq!(reopened.player_dose("p").millirad,129_000);
        h.reject=true;
        assert!(reopened.update_player_dose(&mut h,"p",|d|d.millirad=0).is_none());
        assert_eq!(h.dose.as_ref().unwrap().millirad,129_000);
        assert_eq!(reopened.player_dose("p").millirad,129_000);
    }
    #[test]
    fn carried_pellet_rate_uses_custom_long_not_fresh_catalog_rate() {
        let mut h=Host::default();let mut s=Server::native(&mut h,0);
        s.players=vec![PlayerInfo{id:"p".into(),name:"p".into(),dim:"minecraft:overworld".into(),pos:[0.,64.,0.],creative:true}];
        h.carried=vec![Stack{custom:Some("{PELLET_DEPLETION:5760000000l}".into()),..Stack::new("ntm:pellet_rtg_radium",2)}];
        s.tick=20;s.radiation_tick(&mut h);assert_eq!(h.shown[radiation::KEY_RATE],11250);
        h.carried[0].custom=Some("{PELLET_DEPLETION:0l}".into());s.tick=40;s.radiation_tick(&mut h);assert_eq!(h.shown[radiation::KEY_RATE],0);
        // The shared path still honors the ZIRNOX modifier and ordinary waste.
        h.carried=vec![Stack::new("ntm:nuclear_waste_short__uranium235",1),Stack{custom:Some("{life:6250}".into()),..Stack::new("ntm:rod_zirnox__uranium_fuel",1)}];
        s.tick=60;s.radiation_tick(&mut h);assert_eq!(h.shown[radiation::KEY_RATE],67875);
    }
    /// A bottle's point `contaminate` joins the continuous sources in the same
    /// second's reading, once, and the next window starts empty.
    #[test]
    fn point_contamination_joins_the_window_once() {
        let mut h=Host::default();let mut s=Server::native(&mut h,0);
        s.players=vec![PlayerInfo{id:"p".into(),name:"p".into(),dim:"minecraft:overworld".into(),pos:[0.,64.,0.],creative:true}];
        h.carried=vec![Stack{custom:Some("{PELLET_DEPLETION:5760000000l}".into()),..Stack::new("ntm:pellet_rtg_radium",2)}];
        s.note_contamination("p",5_000);s.note_contamination("p",15_000);
        s.note_contamination("gone",9_000);
        assert_eq!(s.rad_buf("p"),0,"nothing before the window closes");
        s.tick=20;s.radiation_tick(&mut h);
        assert_eq!(h.shown[radiation::KEY_RATE],11250+20_000);
        assert_eq!(s.rad_buf("p"),31_250);
        s.tick=40;s.radiation_tick(&mut h);
        assert_eq!(s.rad_buf("p"),11_250,"events are spent with their window");
        assert!(!s.radiation_env.contains_key("gone"));
    }
    /// TileEntityDecon through the server tick: every decontaminator under the
    /// player takes 0.5 RAD a tick, creative included, and ends the irradiation.
    #[test]
    fn decontaminators_take_their_half_rad_and_end_the_irradiation() {
        #[derive(Default)]
        struct Decon {inner:Host,decons:Vec<Pos>,dose:Option<crate::radiation::Dose>,commits:u32}
        impl Kv for Decon {
            fn get(&mut self,k:&str)->Option<Vec<u8>>{self.inner.get(k)}
            fn set(&mut self,k:&str,v:&[u8])->Result<(),String>{self.inner.set(k,v)}
            fn delete(&mut self,k:&str){self.inner.delete(k)}
        }
        impl ServerHost for Decon {
            fn get_block(&mut self,_:&str,p:Pos)->Option<String>{Some(if self.decons.contains(&p){"ntm:decon[facing=north]".into()}else{"minecraft:stone".into()})}
            fn set_blocks(&mut self,_:&str,_:&[(Pos,String)])->Result<(),String>{Ok(())}
            fn tell(&mut self,_:&str,_:&str){}
            fn log(&mut self,_:&str){}
            fn player_dose(&mut self,_:&str,legacy:&crate::radiation::Dose)->Option<crate::radiation::Dose>{Some(self.dose.clone().unwrap_or_else(||legacy.clone()))}
            fn commit_player_dose(&mut self,_:&str,_:&crate::radiation::Dose,desired:&crate::radiation::Dose)->bool{self.dose=Some(desired.clone());self.commits+=1;true}
        }
        let mut h=Decon{decons:vec![[0,63,0]],dose:Some(crate::radiation::Dose{millirad:2_000}),..Default::default()};
        let mut s=Server::native(&mut h,0);
        s.players=vec![PlayerInfo{id:"p".into(),name:"p".into(),dim:"minecraft:overworld".into(),pos:[0.5,64.,0.5],creative:true}];
        s.ready.world.zirnox.irradiate("p",600,3);
        s.decon_tick(&mut h);
        assert_eq!(h.dose.as_ref().unwrap().millirad,1_500);
        assert!(!s.ready.world.zirnox.irradiated.contains_key("p"));
        // A second one reaching the same player: one RAD a tick, down to 0.
        h.decons.push([1,63,0]);s.players[0].pos=[1.0,64.,0.5];
        s.decon_tick(&mut h);
        assert_eq!(h.dose.as_ref().unwrap().millirad,500);
        s.decon_tick(&mut h);
        assert_eq!(h.dose.as_ref().unwrap().millirad,0);
        // Nothing left to take: no commit.
        let commits=h.commits;s.decon_tick(&mut h);assert_eq!(h.commits,commits);
        // Out of reach: nothing.
        h.dose=Some(crate::radiation::Dose{millirad:2_000});s.players[0].pos=[3.5,64.,0.5];
        s.decon_tick(&mut h);
        assert_eq!(h.dose.as_ref().unwrap().millirad,2_000);
    }
    /// WasteEarth.onEntityWalking: glowing mycelium gives the radiation potion
    /// (30 s, amplifier 3) in a world without any ZIRNOX debris, and the potion
    /// doses (3 + 1) * 0.05 RAD a tick; a decontaminator ends it.
    #[test]
    fn mycelium_irradiates_without_debris_and_decon_ends_it() {
        #[derive(Default)]
        struct Soil {inner:Host,block:String,dose:Option<crate::radiation::Dose>}
        impl Kv for Soil {
            fn get(&mut self,k:&str)->Option<Vec<u8>>{self.inner.get(k)}
            fn set(&mut self,k:&str,v:&[u8])->Result<(),String>{self.inner.set(k,v)}
            fn delete(&mut self,k:&str){self.inner.delete(k)}
        }
        impl ServerHost for Soil {
            fn get_block(&mut self,_:&str,p:Pos)->Option<String>{Some(if p==[0,63,0]{self.block.clone()}else{"minecraft:stone".into()})}
            fn set_blocks(&mut self,_:&str,_:&[(Pos,String)])->Result<(),String>{Ok(())}
            fn tell(&mut self,_:&str,_:&str){}
            fn log(&mut self,_:&str){}
            fn player_dose(&mut self,_:&str,legacy:&crate::radiation::Dose)->Option<crate::radiation::Dose>{Some(self.dose.clone().unwrap_or_else(||legacy.clone()))}
            fn commit_player_dose(&mut self,_:&str,_:&crate::radiation::Dose,desired:&crate::radiation::Dose)->bool{self.dose=Some(desired.clone());true}
        }
        let mut h=Soil{block:"ntm:waste_mycelium".into(),dose:Some(crate::radiation::Dose{millirad:0}),..Default::default()};
        let mut s=Server::native(&mut h,0);
        s.players=vec![PlayerInfo{id:"p".into(),name:"p".into(),dim:"minecraft:overworld".into(),pos:[0.5,64.,0.5],creative:false}];
        // Past the player's first 200 ticks (`ContaminationUtil:287`).
        s.blasts.entered.insert("p".into(),0);s.tick=radiation::GRACE_TICKS;
        assert!(s.ready.world.zirnox.debris.is_empty());
        // A step on the mycelium (crate::walking): the step counters half a block behind.
        s.ready.walks.insert("p".into(),crate::walking::Walk{dim:"minecraft:overworld".into(),at:[0.0,64.,0.5],walked:0.0,next_step:0});
        s.zirnox_world_effects(&mut h);s.zirnox_irradiation(&mut h);
        assert_eq!(s.ready.world.zirnox.irradiated.get("p"),Some(&(599,3)));
        assert_eq!(h.dose.as_ref().unwrap().millirad,200);
        h.block="minecraft:stone".into();
        s.zirnox_world_effects(&mut h);s.zirnox_irradiation(&mut h);
        assert_eq!(h.dose.as_ref().unwrap().millirad,400,"the potion keeps dosing off the mycelium");
        h.block="ntm:decon".into();
        s.decon_tick(&mut h);
        assert!(!s.ready.world.zirnox.irradiated.contains_key("p"));
        h.block="minecraft:stone".into();
        s.zirnox_world_effects(&mut h);s.zirnox_irradiation(&mut h);
        assert_eq!(h.dose.as_ref().unwrap().millirad,0,"after the decontaminator nothing doses");
    }
}
