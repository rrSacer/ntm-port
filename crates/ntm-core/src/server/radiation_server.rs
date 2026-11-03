//! Radiation on the players (docs/radiacao.md, stage L4): once a second
//! each player's rate is what they carry plus the fallout zones where they
//! stand. Past the joining grace the dose grows by what they carry (the
//! zones' dose is the bombs', `bomb_server.rs`), sickness acts, and their
//! client's Geiger reads both values.
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
    pub(super) fn expose_player(&mut self,host:&mut impl ServerHost,id:&str,amount:u64) {
        let factor=host.radiation_factor(id);
        let amount=(amount as f64*factor) as u64;
        self.update_player_dose(host,id,|dose|radiation::add_dose(dose,amount));
    }
    pub(super) fn radiation_tick(&mut self, host: &mut impl ServerHost) {
        let tick = self.tick;
        if !tick.is_multiple_of(RATE_EVERY) {
            return;
        }
        self.radiation_shown.retain(|id, _| self.players.iter().any(|p| &p.id == id));
        for p in self.players.clone() {
            let carried = host.inventory_stacks(&p.id).unwrap_or_default();
            let held = carried.iter().fold(0u64,|sum,s|sum.saturating_add(crate::zirnox::radiation_rate(s).or_else(||crate::radioisotopes::radiation_rate(s)).map_or_else(||radiation::rate_of([(s.id.as_str(),s.count)]),|rate|u64::from(rate)*u64::from(s.count))));
            let held = u32::try_from(held).unwrap_or(u32::MAX);
            let now = self.ready.world.ticks;
            let zones = self.ready.world.bombs.zones.iter().map(|z| z.rate(&p.dim, p.pos, now)).fold(0u32, u32::saturating_add);
            let grace = self.blasts.entered.get(&p.id).is_none_or(|t| tick.saturating_sub(*t) < radiation::GRACE_TICKS);
            let protected = (f64::from(held)*host.radiation_factor(&p.id)) as u32;
            let Some(harms) = self.update_player_dose(host,&p.id,|dose| {
                if p.creative || grace {Vec::new()} else {radiation::accumulate(dose,protected);radiation::sickness(dose)}
            }) else {continue};
            let dose = &self.ready.world.radiation[&p.id];
            let shown = (i64::try_from(dose.millirad).unwrap_or(i64::MAX), i64::from(held.saturating_add(zones).saturating_add(self.ready.zirnox_radiation_rate.get(&p.id).copied().unwrap_or(0))));
            for h in harms {
                match h {
                    Harm::Effect { id, amplifier, seconds } => host.effect(&p.id, id, seconds, amplifier),
                    Harm::Hurt { amount, kind } => host.hurt(&p.id, amount, kind),
                }
            }
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
}
