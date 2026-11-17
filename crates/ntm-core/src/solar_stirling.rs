//! Energy batch 4: `TileEntitySolarBoiler`, `TileEntitySolarMirror` and
//! `TileEntityStirling`. The server supplies the real sky sample, mirror
//! targets and heat source. Solar heat is an accumulator, never a thermal
//! reservoir. A Stirling offers yesterday's buffer before replacing it.
use crate::{
    Pos,
    energy::{EnergyNode, Role},
    fluid::{FluidNode, Tank},
    machine::Logic,
    server::container_bridge::{Slots, Slotted},
    view::ViewModel,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolarBoiler {
    pub water: Tank,
    pub steam: Tank,
    #[serde(skip)]
    pub heat: i32,
    #[serde(skip)]
    pub display: i32,
    #[serde(skip)]
    pub slots: Slots,
}
impl Default for SolarBoiler {
    fn default() -> Self {
        Self {
            water: Tank::typed("water", 100),
            steam: Tank::typed("steam", 10_000),
            heat: 0,
            display: 0,
            slots: Vec::new(),
        }
    }
}
impl SolarBoiler {
    pub fn step(&mut self) {
        self.display = self.heat / 50;
        let n = self.display.max(0) as u32;
        let n = n.min(self.water.mb).min(self.steam.free() / 100);
        self.water.drain(n);
        self.steam.fill("steam", n * 100);
        self.heat = 0;
    }
}
impl FluidNode for SolarBoiler {
    fn fluid_types(&self) -> Vec<String> {
        vec!["water".into(), "steam".into()]
    }
    fn fluid_offer(&self, f: &str) -> u64 {
        if f == "steam" { u64::from(self.steam.mb) } else { 0 }
    }
    fn fluid_demand(&self, f: &str) -> u64 {
        if f == "water" { u64::from(self.water.free()) } else { 0 }
    }
    fn fluid_take(&mut self, f: &str, mb: u64) {
        if f == "steam" {
            self.steam.drain(mb.min(u64::from(self.steam.mb)) as u32);
        }
    }
    fn fluid_give(&mut self, f: &str, mb: u64) {
        if f == "water" {
            self.water.fill(f, mb.min(u64::from(self.water.free())) as u32);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mirror {
    pub target: Pos,
    #[serde(skip)]
    pub on: bool,
    #[serde(skip)]
    pub slots: Slots,
}
impl Mirror {
    /// Tool-only validation. The ticking mirror deliberately doesn't repeat
    /// the distance/angle checks (`ItemMirrorTool.onItemUse`).
    pub fn link(&mut self, at: Pos, target: Pos) -> Result<(), &'static str> {
        let [dx, dy, dz] = std::array::from_fn(|i| i64::from(at[i]) - i64::from(target[i]));
        if dx * dx + dy * dy + dz * dz > 10_000 {
            return Err("Target is out of reach (100 blocks).");
        }
        if dx * dx + dz * dz > dy * dy {
            return Err("Target is outside the mirror's angle.");
        }
        self.target = target;
        Ok(())
    }
    pub fn sun(&mut self, y: i32, sample: Option<(i32, i32, bool)>) -> i32 {
        let (sky, darken, visible) = sample.unwrap_or((0, 0, false));
        let sun = sky - darken - 11;
        self.on = self.target[1] >= y && sun > 0 && visible;
        if self.on { sun } else { 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Iron,
    Steel,
    Creative,
}
impl Shape {
    pub fn gear(self) -> Option<&'static str> {
        match self {
            Self::Iron => Some("ntm:gear_large__iron"),
            Self::Steel => Some("ntm:gear_large__steel"),
            Self::Creative => None,
        }
    }
    pub fn max_heat(self) -> i32 {
        if self == Self::Iron { 300 } else { 1500 }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stirling {
    pub shape: Shape,
    pub power: u64,
    pub has_cog: bool,
    pub overspeed: u32,
    #[serde(skip)]
    pub heat: i32,
    #[serde(skip)]
    pub warn_cooldown: u32,
    #[serde(skip)]
    pub slots: Slots,
}
impl Stirling {
    pub fn new(shape: Shape) -> Self {
        Self {
            shape,
            power: 0,
            has_cog: true,
            overspeed: 0,
            heat: 0,
            warn_cooldown: 0,
            slots: Vec::new(),
        }
    }
    /// The immediately lower IHeatSource loses precisely the truncated 10%.
    pub fn pull_heat(&mut self, source: Option<u32>) -> u32 {
        if !self.has_cog {
            return 0;
        }
        let n = (f64::from(source.unwrap_or(0)) * 0.1) as i32;
        if n > 0 {
            self.heat = self.heat.wrapping_add(n);
            n as u32
        } else {
            self.heat = (self.heat - (self.heat / 1000).max(1)).max(0);
            0
        }
    }
    /// Whether this tick's `finish` breaks the gear (TileEntityStirling.java:90-92): the step that
    /// also explodes, so it goes out through the server's post-phase publisher with its blast.
    pub fn fails_on_finish(&self) -> bool {
        self.has_cog
            && self.shape != Shape::Creative
            && self.heat > self.shape.max_heat()
            && self.overspeed.saturating_add(1) > 300
    }
    /// After the network took the previous buffer. Failure is strictly
    /// after 300 hot ticks; even on the failure tick the buffer loses one.
    pub fn finish(&mut self) {
        if self.has_cog {
            self.power = (f64::from(self.heat)
                * if self.shape == Shape::Creative {
                    1.0
                } else {
                    0.5
                })
            .max(0.0) as u64;
            self.warn_cooldown = self.warn_cooldown.saturating_sub(1);
            if self.heat > self.shape.max_heat() && self.shape != Shape::Creative {
                self.overspeed += 1;
                if self.overspeed > 60 && self.warn_cooldown == 0 {
                    self.warn_cooldown = 100;
                }
                if self.overspeed > 300 {
                    self.has_cog = false;
                }
            } else {
                self.overspeed = 0;
            }
        } else {
            self.overspeed = 0;
            self.warn_cooldown = 0;
        }
        if !self.has_cog {
            self.power = self.power.saturating_sub(1);
        }
        self.heat = 0;
    }
}
impl EnergyNode for Stirling {
    fn role(&self) -> Role {
        Role::Provider
    }
    fn offer(&self) -> u64 {
        if self.has_cog { self.power } else { 0 }
    }
    fn demand(&self) -> u64 {
        0
    }
    fn give(&mut self, _: u64) {}
    fn take(&mut self, n: u64) {
        self.power = self.power.saturating_sub(n);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SolarStirling {
    Boiler(SolarBoiler),
    Mirror(Mirror),
    Stirling(Stirling),
}
impl SolarStirling {
    pub fn of(block: &str) -> Option<Self> {
        Some(match block {
            "ntm:machine_solar_boiler" => Self::Boiler(SolarBoiler::default()),
            "ntm:solar_mirror" => Self::Mirror(Mirror::default()),
            "ntm:machine_stirling" => Self::Stirling(Stirling::new(Shape::Iron)),
            "ntm:machine_stirling_steel" => Self::Stirling(Stirling::new(Shape::Steel)),
            "ntm:machine_stirling_creative" => Self::Stirling(Stirling::new(Shape::Creative)),
            _ => return None,
        })
    }
}
impl Logic for SolarStirling {
    fn energy(&mut self) -> Option<&mut dyn EnergyNode> {
        if let Self::Stirling(s) = self {
            Some(s)
        } else {
            None
        }
    }
    fn fluid(&mut self) -> Option<&mut dyn FluidNode> {
        if let Self::Boiler(b) = self {
            Some(b)
        } else {
            None
        }
    }
    fn level(&self, _: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Solar / Stirling")
    }
}
impl Slotted for SolarStirling {
    fn slots_mut(&mut self) -> &mut Slots {
        match self {
            Self::Boiler(b) => &mut b.slots,
            Self::Mirror(m) => &mut m.slots,
            Self::Stirling(s) => &mut s.slots,
        }
    }
    fn screen_data(&self) -> Vec<i32> {
        match self {
            Self::Stirling(s) => vec![i32::from(s.has_cog), s.power.min(i32::MAX as u64) as i32],
            Self::Mirror(m) => vec![m.target[0], m.target[1], m.target[2], i32::from(m.on)],
            Self::Boiler(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solar_conversion_discards_remainder_and_reports_uncapped_heat() {
        let mut b = SolarBoiler::default();
        b.water.fill("water", 3);
        b.heat = 549;
        b.step();
        assert_eq!((b.display, b.water.mb, b.steam.mb, b.heat), (10, 0, 300, 0));
        b.heat = 49;
        b.water.fill("water", 100);
        b.step();
        assert_eq!((b.steam.mb, b.heat), (300, 0));
        b.steam.fill("steam", 9650);
        b.heat = 5000;
        b.step();
        assert_eq!((b.water.mb, b.steam.mb, b.display), (100, 9950, 100));
    }
    #[test]
    fn mirrors_validate_links_only_at_the_tool_and_read_sun_and_sky() {
        let mut m = Mirror::default();
        assert!(m.link([0, 0, 0], [0, 100, 0]).is_ok());
        assert!(m.link([0, 0, 0], [1, 100, 0]).is_err());
        assert!(m.link([0, 0, 0], [10, 10, 0]).is_ok());
        assert!(m.link([0, 0, 0], [11, 10, 0]).is_err());
        assert_eq!(m.sun(0, Some((15, 0, true))), 4);
        assert_eq!(m.sun(0, Some((15, 4, true))), 0);
        assert_eq!(m.sun(0, Some((15, 0, false))), 0);
        assert_eq!(m.sun(11, Some((15, 0, true))), 0);
        m.target = [0, 1000, 0]; // no repeat of the tool's reach check
        assert_eq!(m.sun(0, Some((15, 0, true))), 4);
    }
    #[test]
    fn stirling_conversion_overspeed_and_creative_need_heat() {
        for shape in [Shape::Iron, Shape::Steel, Shape::Creative] {
            let mut s = Stirling::new(shape);
            s.finish();
            assert_eq!(s.power, 0);
            assert_eq!(s.pull_heat(Some(19)), 1);
            s.finish();
            assert_eq!(s.power, u64::from(shape == Shape::Creative));
            for _ in 0..300 {
                s.pull_heat(Some(16000));
                assert!(!s.fails_on_finish());
                s.finish();
            }
            assert!(s.has_cog);
            s.pull_heat(Some(16000));
            assert_eq!(s.fails_on_finish(), shape != Shape::Creative);
            s.finish();
            assert!(!s.fails_on_finish(), "a lost gear does not fail again");
            assert_eq!(s.has_cog, shape == Shape::Creative);
            assert_eq!(s.power, if shape == Shape::Creative { 1600 } else { 799 });
            assert_eq!(s.offer(), if shape == Shape::Creative { 1600 } else { 0 });
        }
    }
    #[test]
    fn cool_tick_resets_consecutive_overspeed_and_transients_arent_saved() {
        let mut s = Stirling::new(Shape::Iron);
        s.pull_heat(Some(3010));
        s.finish();
        assert_eq!(s.overspeed, 1);
        s.pull_heat(Some(3000));
        s.finish();
        assert_eq!(s.overspeed, 0);
        s.pull_heat(Some(101));
        let loaded: Stirling = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!((loaded.heat, loaded.power, loaded.has_cog), (0, 150, true));
    }
}
