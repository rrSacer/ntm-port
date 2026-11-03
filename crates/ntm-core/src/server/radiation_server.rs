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
            let r = &mut self.ready;
            let now = r.world.ticks;
            let zones = r.world.bombs.zones.iter().map(|z| z.rate(&p.dim, p.pos, now)).fold(0u32, u32::saturating_add);
            let grace = self.blasts.entered.get(&p.id).is_none_or(|t| tick.saturating_sub(*t) < radiation::GRACE_TICKS);
            let dose = r.world.radiation.entry(p.id.clone()).or_default();
            // Creative and spectator players aren't contaminated
            // (`ContaminationUtil`); the Geiger still reads the rate.
            let harms = if p.creative || grace {
                Vec::new()
            } else {
                radiation::accumulate(dose, held);
                radiation::sickness(dose)
            };
            let shown = (i64::try_from(dose.millirad).unwrap_or(i64::MAX), i64::from(held.saturating_add(zones).saturating_add(r.zirnox_radiation_rate.get(&p.id).copied().unwrap_or(0))));
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
