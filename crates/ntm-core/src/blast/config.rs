//! Bomb settings, global or per world. Only optimized visuals are active.
//! Legacy enhanced settings deserialize as optimized without losing other settings.

use serde::{Deserialize, Serialize};

/// The storage key of the global settings.
pub const KEY: &str = "bombcfg";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    #[serde(alias = "Aprimorado", alias = "aprimorado", alias = "Enhanced", alias = "enhanced")]
    Otimizado,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Otimizado => "otimizado",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub mode: Mode,
    /// The 2.5R fallout after a nuke's crater.
    pub fallout: bool,
    /// Sounds and particles (vanilla `playsound`/`particle`).
    pub effects: bool,
    /// Particles per effect line (0..=200).
    pub particles: u32,
    /// Blast damage to players (`damage`).
    pub damage: bool,
    /// The blast's radiation pulse (`EntityNukeExplosionMK5.radiate`).
    pub radiation: bool,
    /// Block writes per tick of a nuke (256..=60 000).
    pub budget: usize,
    /// Largest radius a nuke gets (bigger ones are cut to it).
    pub max_radius: i32,
    /// Fuel (millions, estimated) an explosion may spend a tick. Horizon
    /// stops a server mod past 50 M in a tick; the engine's worst tick was
    /// measured at ~1.4× this (docs/bombas.md, "Fuel"), and the rest of
    /// the mod needs its share.
    pub fuel: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Otimizado,
            fallout: true,
            effects: true,
            particles: 24,
            damage: true,
            radiation: true,
            budget: crate::explosion::DEFAULT_BUDGET,
            max_radius: crate::explosion::MAX_RADIUS,
            fuel: 22,
        }
    }
}

fn on_off(v: &str) -> Result<bool, String> {
    match v {
        "on" | "sim" | "true" | "1" | "ligado" => Ok(true),
        "off" | "nao" | "não" | "false" | "0" | "desligado" => Ok(false),
        _ => Err(format!("{v}: use on/off")),
    }
}

impl Config {
    /// `chave valor` from the command.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let num = |v: &str| v.parse::<i64>().map_err(|e| format!("{v}: {e}"));
        match key {
            "modo" | "mode" => {
                self.mode = match value {
                    "otimizado" | "optimized" => Mode::Otimizado,
                    _ => return Err("modo: otimizado (aprimorado arquivado)".into()),
                }
            }
            "fallout" => self.fallout = on_off(value)?,
            "efeitos" | "effects" => self.effects = on_off(value)?,
            "particulas" | "partículas" => self.particles = num(value)?.clamp(0, 200) as u32,
            "dano" | "damage" => self.damage = on_off(value)?,
            "radiacao" | "radiação" => self.radiation = on_off(value)?,
            "orcamento" | "orçamento" => self.budget = num(value)?.clamp(256, 60_000) as usize,
            "fuel" | "combustivel" => self.fuel = num(value)?.clamp(4, 34) as u32,
            "raio" | "raiomax" => self.max_radius = num(value)?.clamp(1, i64::from(crate::explosion::MAX_RADIUS)) as i32,
            _ => return Err(format!("chave desconhecida {key}: modo fallout efeitos particulas dano radiacao orcamento raiomax fuel")),
        }
        Ok(())
    }

    pub fn describe(&self) -> String {
        let b = |v: bool| if v { "on" } else { "off" };
        format!(
            "modo {} | fallout {} | efeitos {} (partículas {}) | dano {} | radiação {} | orçamento {}/tick | raiomax {} | fuel {} M",
            self.mode.name(),
            b(self.fallout),
            b(self.effects),
            self.particles,
            b(self.damage),
            b(self.radiation),
            self.budget,
            self.max_radius,
            self.fuel
        )
    }

    pub fn load(bytes: Option<Vec<u8>>) -> Self {
        bytes.and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archived_mode_keeps_existing_world_settings() {
        let cfg = Config::load(Some(br#"{"mode":"Aprimorado","effects":false,"max_radius":17,"budget":1024}"#.to_vec()));
        assert_eq!(cfg.mode, Mode::Otimizado);
        assert!(!cfg.effects);
        assert_eq!(cfg.max_radius, 17);
        assert_eq!(cfg.budget, 1024);
        assert_eq!(serde_json::to_value(&cfg).unwrap()["mode"], "Otimizado");
    }

    /// The zones' settings (slice 4e: the zones are gone) in a world saved before: read past, the rest kept.
    #[test]
    fn the_zones_settings_of_an_old_world_are_read_past() {
        let cfg = Config::load(Some(br#"{"radiation":false,"zone_peak":5000,"zone_half_life":600,"max_radius":17}"#.to_vec()));
        assert!(!cfg.radiation);
        assert_eq!(cfg.max_radius, 17);
        assert!(serde_json::to_value(&cfg).unwrap().get("zone_peak").is_none());
        assert!(Config::default().set("zona", "5000").is_err());
    }
}
