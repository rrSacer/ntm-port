//! The bombs' settings: one global (the mod's folder, `bombcfg`) and, if
//! set, one per world (in its save), which wins. Two modes: `otimizado`
//! (vanilla commands only: what this milestone does) and `aprimorado`, a
//! placeholder for visuals that need engine APIs still being researched
//! (it falls back to `otimizado`, see [`super::visuals`]).

use serde::{Deserialize, Serialize};

/// The storage key of the global settings.
pub const KEY: &str = "bombcfg";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Otimizado,
    Aprimorado,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Otimizado => "otimizado",
            Mode::Aprimorado => "aprimorado",
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
    /// The blast's radiation pulse and the fallout's area radiation.
    pub radiation: bool,
    /// Area radiation at ground zero, milliRAD/s per 100 blocks of radius.
    pub zone_peak: u32,
    /// Ticks for the area radiation to halve.
    pub zone_half_life: u64,
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
            zone_peak: 10_000,
            zone_half_life: 12_000,
            budget: ntm_explosion::DEFAULT_BUDGET,
            max_radius: ntm_explosion::MAX_RADIUS,
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
                    "aprimorado" | "enhanced" => Mode::Aprimorado,
                    _ => return Err("modo: otimizado | aprimorado".into()),
                }
            }
            "fallout" => self.fallout = on_off(value)?,
            "efeitos" | "effects" => self.effects = on_off(value)?,
            "particulas" | "partículas" => self.particles = num(value)?.clamp(0, 200) as u32,
            "dano" | "damage" => self.damage = on_off(value)?,
            "radiacao" | "radiação" => self.radiation = on_off(value)?,
            "zona" => self.zone_peak = num(value)?.clamp(0, 1_000_000) as u32,
            "meiavida" | "meia-vida" => self.zone_half_life = num(value)?.clamp(20, 10_000_000) as u64,
            "orcamento" | "orçamento" => self.budget = num(value)?.clamp(256, 60_000) as usize,
            "fuel" | "combustivel" => self.fuel = num(value)?.clamp(4, 34) as u32,
            "raio" | "raiomax" => self.max_radius = num(value)?.clamp(1, i64::from(ntm_explosion::MAX_RADIUS)) as i32,
            _ => return Err(format!("chave desconhecida {key}: modo fallout efeitos particulas dano radiacao zona meiavida orcamento raiomax fuel")),
        }
        Ok(())
    }

    pub fn describe(&self) -> String {
        let b = |v: bool| if v { "on" } else { "off" };
        format!(
            "modo {} | fallout {} | efeitos {} (partículas {}) | dano {} | radiação {} (zona {} mRAD/s por 100 de raio, meia-vida {} ticks) | orçamento {}/tick | raiomax {} | fuel {} M",
            self.mode.name(),
            b(self.fallout),
            b(self.effects),
            self.particles,
            b(self.damage),
            b(self.radiation),
            self.zone_peak,
            self.zone_half_life,
            self.budget,
            self.max_radius,
            self.fuel
        )
    }

    pub fn load(bytes: Option<Vec<u8>>) -> Self {
        bytes.and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
}
