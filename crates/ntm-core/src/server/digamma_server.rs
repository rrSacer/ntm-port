//! Digamma on the players (`crate::digamma`): the dose kept in each player's own storage, its `maxHealth`
//! modifier and its death, the DRX beam's and the digamma flame's ways in.
use super::*;
use crate::digamma::{self, Gates};

/// A 1.7.10 player's bounding box (`EntityPlayer.setSize(0.6F, 1.8F)`), from the feet.
const HALF_WIDTH: f64 = 0.3;
const HEIGHT: f64 = 1.8;

/// `Entity.func_145775_I` (`doBlockCollisions`, at the end of `moveEntity`): the cells of the bounding box shrunk
/// by 0.001 on every side, each one's `onEntityCollidedWithBlock` once.
pub(super) fn touched_cells(feet: [f64; 3]) -> Vec<Pos> {
    let lo = [feet[0] - HALF_WIDTH + 0.001, feet[1] + 0.001, feet[2] - HALF_WIDTH + 0.001];
    let hi = [feet[0] + HALF_WIDTH - 0.001, feet[1] + HEIGHT - 0.001, feet[2] + HALF_WIDTH - 0.001];
    let mut out = Vec::new();
    for x in lo[0].floor() as i32..=hi[0].floor() as i32 {
        for y in lo[1].floor() as i32..=hi[1].floor() as i32 {
            for z in lo[2].floor() as i32..=hi[2].floor() as i32 {
                out.push([x, y, z]);
            }
        }
    }
    out
}

impl Server {
    /// What keeps player `p`'s digamma where it is (`digamma::Gates`).
    fn digamma_gates(&self, host: &mut impl ServerHost, p: &PlayerInfo) -> Gates {
        let worn = host.worn_armor(&p.id);
        Gates {
            creative: p.creative,
            joining: self.in_grace(&p.id),
            stability: matches!(host.effect_state(&p.id, digamma::STABILITY), Some(Some(_))),
            armor: worn.is_some_and(|w| digamma::wears_set(&w, &digamma::FAU_SET) || digamma::wears_set(&w, &digamma::DNS_SET)),
        }
    }

    /// `applyDigammaData` / `contaminate(.., DIGAMMA, DIGAMMA, amount)` on player `p`: past the gates,
    /// `incrementDigamma` and `setDigamma` (the storage writes the modifier), and at 10 the digamma's blow. True when
    /// their digamma was written.
    pub(super) fn digamma_dose(&mut self, host: &mut impl ServerHost, p: &PlayerInfo, amount: f32) -> bool {
        if digamma::blocked(self.digamma_gates(host, p)) {
            return false;
        }
        let legacy = self.player_dose(&p.id);
        let Some(before) = host.player_digamma(&p.id, &legacy) else { return false };
        let after = digamma::increment(before, amount);
        if !host.commit_player_digamma(&p.id, &legacy, before, after) {
            return false;
        }
        let max_health = host.max_health(&p.id).unwrap_or(1.0);
        if digamma::kills(after, max_health) {
            host.hurt(&p.id, digamma::DEATH_DAMAGE, digamma::DAMAGE);
        }
        true
    }

    /// Every tick, for each living player: a body that came in this tick takes the modifier its storage holds, and
    /// each cell of a digamma flame it touches gives it `DigammaFlame`'s 0.05 (`onEntityCollidedWithBlock`).
    pub(super) fn digamma_tick(&mut self, host: &mut impl ServerHost) {
        for p in self.players.clone() {
            if self.blasts.entered.get(&p.id) == Some(&self.tick) {
                host.sync_digamma(&p.id, &self.player_dose(&p.id));
            }
            for cell in touched_cells(p.pos) {
                if host.get_block(&p.dim, cell).is_some_and(|b| blockstate::id_of(&b) == digamma::FLAME_BLOCK) {
                    self.digamma_dose(host, &p, digamma::FLAME);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Body {
        kv: BTreeMap<String, Vec<u8>>,
        blocks: BTreeMap<Pos, String>,
        digamma: BTreeMap<String, f32>,
        reject: bool,
        stability: bool,
        worn: Option<[Option<String>; 4]>,
        hurt: Vec<(String, f32, String)>,
        synced: Vec<String>,
    }
    impl Kv for Body {
        fn get(&mut self, k: &str) -> Option<Vec<u8>> {
            self.kv.get(k).cloned()
        }
        fn set(&mut self, k: &str, v: &[u8]) -> Result<(), String> {
            self.kv.insert(k.into(), v.into());
            Ok(())
        }
        fn delete(&mut self, k: &str) {
            self.kv.remove(k);
        }
    }
    impl ServerHost for Body {
        fn get_block(&mut self, _: &str, p: Pos) -> Option<String> {
            Some(self.blocks.get(&p).cloned().unwrap_or_else(|| "minecraft:air".into()))
        }
        fn set_blocks(&mut self, _: &str, _: &[(Pos, String)]) -> Result<(), String> {
            Ok(())
        }
        fn tell(&mut self, _: &str, _: &str) {}
        fn log(&mut self, _: &str) {}
        fn player_digamma(&mut self, id: &str, _: &crate::radiation::Dose) -> Option<f32> {
            Some(self.digamma.get(id).copied().unwrap_or(0.0))
        }
        fn commit_player_digamma(&mut self, id: &str, _: &crate::radiation::Dose, expected: f32, desired: f32) -> bool {
            if self.reject || self.digamma.get(id).copied().unwrap_or(0.0) != expected {
                return false;
            }
            self.digamma.insert(id.into(), desired);
            true
        }
        fn sync_digamma(&mut self, id: &str, _: &crate::radiation::Dose) {
            self.synced.push(id.into());
        }
        fn max_health(&mut self, id: &str) -> Option<f64> {
            Some(20.0 * (1.0 + f64::from(digamma::health_modifier(self.digamma.get(id).copied().unwrap_or(0.0)))))
        }
        fn worn_armor(&mut self, _: &str) -> Option<[Option<String>; 4]> {
            self.worn.clone()
        }
        fn effect_state(&mut self, _: &str, effect: &str) -> Option<Option<(u32, u8)>> {
            Some((self.stability && effect == digamma::STABILITY).then_some((100, 0)))
        }
        fn hurt(&mut self, id: &str, amount: f32, kind: &str) {
            self.hurt.push((id.into(), amount, kind.into()));
        }
    }

    fn player(id: &str, pos: [f64; 3], creative: bool) -> PlayerInfo {
        PlayerInfo { id: id.into(), name: id.into(), dim: "minecraft:overworld".into(), pos, creative }
    }

    fn server(host: &mut Body, players: &[PlayerInfo]) -> Server {
        let mut s = Server::native(host, 1000);
        s.tick = 1000;
        for p in players {
            s.blasts.entered.insert(p.id.clone(), 0);
        }
        s.set_players(players.to_vec());
        s
    }

    #[test]
    fn a_player_box_touches_its_two_cells_or_up_to_twelve_on_the_edges() {
        assert_eq!(touched_cells([0.5, 64.0, 0.5]), vec![[0, 64, 0], [0, 65, 0]]);
        // 1.8 high from 64.5: three cells high.
        assert_eq!(touched_cells([0.5, 64.5, 0.5]).len(), 3);
        // On a corner: 2 x 2 columns.
        assert_eq!(touched_cells([1.0, 64.0, 1.0]).len(), 8);
        // 0.001 in: a box 0.3 from a face reaches past it only from 0.701.
        assert_eq!(touched_cells([0.7, 64.0, 0.5]), vec![[0, 64, 0], [0, 65, 0]]);
        assert_eq!(touched_cells([0.7015, 64.0, 0.5]).len(), 4);
        // And the same in from the low side, and at the head.
        assert_eq!(touched_cells([1.2995, 64.0, 0.5]), vec![[1, 64, 0], [1, 65, 0]]);
        assert_eq!(touched_cells([0.5, 63.9995, 0.5]), vec![[0, 64, 0], [0, 65, 0]]);
        assert_eq!(touched_cells([0.5, 64.2, 0.5]), vec![[0, 64, 0], [0, 65, 0]]);
        assert_eq!(touched_cells([0.5, 64.2015, 0.5]).len(), 3);
    }

    #[test]
    fn the_dose_sums_and_kills_at_ten_through_the_storage() {
        let p = player("p", [0.5, 64.0, 0.5], false);
        let mut host = Body::default();
        let mut s = server(&mut host, std::slice::from_ref(&p));
        assert!(s.digamma_dose(&mut host, &p, digamma::FEL_DRX));
        assert!(s.digamma_dose(&mut host, &p, digamma::FEL_DRX));
        assert_eq!(host.digamma["p"], 0.1f32 + 0.1f32);
        assert!(host.hurt.is_empty());
        host.digamma.insert("p".into(), 9.95);
        assert!(s.digamma_dose(&mut host, &p, digamma::FEL_DRX));
        assert_eq!(host.digamma["p"], digamma::MAX);
        assert_eq!(host.hurt, vec![("p".into(), 500.0, "ntm:digamma".into())]);
    }

    #[test]
    fn creative_joining_stable_and_armoured_players_take_none() {
        let creative = player("c", [0.5, 64.0, 0.5], true);
        let new = player("n", [0.5, 64.0, 0.5], false);
        let mut host = Body::default();
        let mut s = server(&mut host, &[creative.clone(), new.clone()]);
        s.blasts.entered.insert("n".into(), 1000 - 199);
        assert!(!s.digamma_dose(&mut host, &creative, 1.0));
        assert!(!s.digamma_dose(&mut host, &new, 1.0));
        s.blasts.entered.insert("n".into(), 1000 - 200);
        host.stability = true;
        assert!(!s.digamma_dose(&mut host, &new, 1.0));
        host.stability = false;
        host.worn = Some(digamma::DNS_SET.map(|i| Some(i.to_string())));
        assert!(!s.digamma_dose(&mut host, &new, 1.0));
        host.worn = Some(digamma::FAU_SET.map(|i| Some(i.to_string())));
        assert!(!s.digamma_dose(&mut host, &new, 1.0));
        let mut half = digamma::FAU_SET.map(|i| Some(i.to_string()));
        half[1] = None;
        host.worn = Some(half);
        assert!(s.digamma_dose(&mut host, &new, 1.0));
        assert!(host.digamma.get("c").is_none());
        assert_eq!(host.digamma["n"], 1.0);
    }

    #[test]
    fn a_refused_write_kills_nobody() {
        let p = player("p", [0.5, 64.0, 0.5], false);
        let mut host = Body { reject: true, ..Default::default() };
        host.digamma.insert("p".into(), 9.95);
        let mut s = server(&mut host, std::slice::from_ref(&p));
        assert!(!s.digamma_dose(&mut host, &p, 1.0));
        assert_eq!(host.digamma["p"], 9.95);
        assert!(host.hurt.is_empty());
    }

    #[test]
    fn each_flame_cell_touched_gives_its_share_every_tick() {
        let p = player("p", [0.5, 64.0, 0.5], false);
        let away = player("a", [10.5, 64.0, 0.5], false);
        let mut host = Body::default();
        host.blocks.insert([0, 64, 0], "ntm:fire_digamma".into());
        host.blocks.insert([0, 65, 0], "ntm:fire_digamma".into());
        host.blocks.insert([0, 66, 0], "ntm:fire_digamma".into());
        host.blocks.insert([10, 64, 0], "ntm:ash_digamma".into());
        let mut s = server(&mut host, &[p, away]);
        s.digamma_tick(&mut host);
        assert_eq!(host.digamma["p"], 0.05f32 + 0.05f32);
        s.digamma_tick(&mut host);
        assert_eq!(host.digamma["p"], 0.05f32 + 0.05f32 + 0.05f32 + 0.05f32);
        assert!(host.digamma.get("a").is_none());
        assert!(host.synced.is_empty());
    }

    #[test]
    fn a_body_that_comes_in_takes_its_modifier_back() {
        let p = player("p", [0.5, 64.0, 0.5], false);
        let mut host = Body::default();
        let mut s = server(&mut host, std::slice::from_ref(&p));
        s.blasts.entered.insert("p".into(), 1000);
        s.digamma_tick(&mut host);
        assert_eq!(host.synced, vec!["p".to_string()]);
        s.tick += 1;
        s.digamma_tick(&mut host);
        assert_eq!(host.synced.len(), 1);
    }
}
