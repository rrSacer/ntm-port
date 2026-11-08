//! `ItemRBMKRod`: the 33 fuel rods, their nine burn curves, five depletion
//! curves, xenon, and the core/hull heat exchange. The rod item is stateless;
//! its state lives in the stack's tag (`yield`, `xenon`, `core`, `hull`).

use super::dials::Dials;
use super::jmath;
use std::collections::BTreeMap;
use std::f64::consts::PI;

pub use super::rods_data::RODS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BurnFunc {
    Passive,
    LogTen,
    Plateu,
    Arch,
    Sigmoid,
    SquareRoot,
    Linear,
    Quadratic,
    Experimental,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepleteFunc {
    Linear,
    RaisingSlope,
    BoostedSlope,
    GentleSlope,
    Static,
}

/// `IRBMKFluxReceiver.NType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NType {
    Slow,
    Fast,
    Any,
}

/// One rod item (the fields of `ItemRBMKRod`). `specialFluxCurve` and its
/// closures are not modelled: no registered rod uses them (0 of 33).
#[derive(Clone, Copy, Debug)]
pub struct RodType {
    pub variable: &'static str,
    pub full_name: &'static str,
    pub pellet: &'static str,
    pub yield_: f64,
    pub reactivity: f64,
    pub self_rate: f64,
    pub function: BurnFunc,
    pub depletion: DepleteFunc,
    pub x_gen: f64,
    pub x_burn: f64,
    pub heat: f64,
    pub melting_point: f64,
    pub diffusion: f64,
    pub heat_coeff_start: f64,
    pub heat_coeff_length: f64,
    pub n_type: NType,
    pub r_type: NType,
    pub tint: u32,
}

impl RodType {
    /// A rod with the Java field initializers (`new ItemRBMKRod(name)`).
    pub const fn blank() -> RodType {
        RodType {
            variable: "",
            full_name: "",
            pellet: "",
            yield_: 0.0,
            reactivity: 0.0,
            self_rate: 0.0,
            function: BurnFunc::LogTen,
            depletion: DepleteFunc::GentleSlope,
            x_gen: 0.5,
            x_burn: 50.0,
            heat: 1.0,
            melting_point: 1000.0,
            diffusion: 0.02,
            heat_coeff_start: 0.0,
            heat_coeff_length: 0.0,
            n_type: NType::Slow,
            r_type: NType::Fast,
            tint: 0x304825,
        }
    }

    pub fn by_variable(variable: &str) -> Option<&'static RodType> {
        RODS.iter().find(|r| r.variable == variable)
    }

    /// `reactivityModByEnrichment`.
    pub fn reactivity_mod_by_enrichment(&self, e: f64) -> f64 {
        match self.depletion {
            DepleteFunc::Linear => e,
            DepleteFunc::Static => 1.0,
            DepleteFunc::BoostedSlope => e + jmath::sin((e - 1.0) * (e - 1.0) * PI),
            DepleteFunc::RaisingSlope => e + (jmath::sin(e * PI) / 2.0),
            DepleteFunc::GentleSlope => e + (jmath::sin(e * PI) / 3.0),
        }
    }

    /// `reactivityFunc`: reactivity yielded, unmodified by xenon.
    pub fn reactivity_func(&self, input: f64, enrichment: f64) -> f64 {
        let flux = input * self.reactivity_mod_by_enrichment(enrichment);
        let r = self.reactivity;
        match self.function {
            BurnFunc::Passive => self.self_rate * enrichment,
            BurnFunc::LogTen => jmath::log10(flux + 1.0) * 0.5 * r,
            BurnFunc::Plateu => (1.0 - jmath::pow_e(-flux / 25.0)) * r,
            BurnFunc::Arch => java_max((flux - (flux * flux / 10000.0)) / 100.0 * r, 0.0),
            BurnFunc::Sigmoid => r / (1.0 + jmath::pow_e(-(flux - 50.0) / 10.0)),
            BurnFunc::SquareRoot => flux.sqrt() * r / 10.0,
            BurnFunc::Linear => flux / 100.0 * r,
            BurnFunc::Quadratic => flux * flux / 10000.0 * r,
            BurnFunc::Experimental => flux * (jmath::sin(flux) + 1.0) * r,
        }
    }

    /// `xenonGenFunc`.
    pub fn xenon_gen(&self, flux: f64) -> f64 {
        flux * self.x_gen
    }

    /// `xenonBurnFunc`.
    pub fn xenon_burn(&self, flux: f64) -> f64 {
        (flux * flux) / self.x_burn
    }
}

/// `Math.max(double, double)`: NaN wins.
pub fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { b } else { a }
    } else if a >= b {
        a
    } else {
        b
    }
}

/// `rectify`: clamp to [20, 1e6], NaN to 20.
fn rectify(mut num: f64) -> f64 {
    if num > 1_000_000.0 {
        num = 1_000_000.0;
    }
    if num < 20.0 || num.is_nan() {
        num = 20.0;
    }
    num
}

/// The tag of a rod stack. Keys the item does not know are kept (`foreign`);
/// a key that was never written reads as 0, as `NBTTagCompound.getDouble`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RodStack {
    pub has_tag: bool,
    pub yield_: Option<f64>,
    pub xenon: Option<f64>,
    pub core: Option<f64>,
    pub hull: Option<f64>,
    pub foreign: BTreeMap<String, f64>,
}

impl RodStack {
    /// A stack with no tag yet (`new ItemStack(rod)`).
    pub fn bare() -> Self {
        Self::default()
    }

    /// `onCreated` / `setNBTDefaults`.
    pub fn fresh(rod: &RodType) -> Self {
        let mut s = Self::bare();
        s.set_defaults(rod);
        s
    }

    fn set_defaults(&mut self, rod: &RodType) {
        *self = Self { has_tag: true, yield_: Some(rod.yield_), core: Some(20.0), hull: Some(20.0), ..Self::default() };
    }

    fn ensure(&mut self, rod: &RodType) {
        if !self.has_tag {
            self.set_defaults(rod);
        }
    }

    pub fn yield_(&mut self, rod: &RodType) -> f64 {
        self.ensure(rod);
        self.yield_.unwrap_or(0.0)
    }

    pub fn poison(&mut self, rod: &RodType) -> f64 {
        self.ensure(rod);
        self.xenon.unwrap_or(0.0)
    }

    pub fn core_heat(&mut self, rod: &RodType) -> f64 {
        self.ensure(rod);
        self.core.unwrap_or(0.0)
    }

    pub fn hull_heat(&mut self, rod: &RodType) -> f64 {
        self.ensure(rod);
        self.hull.unwrap_or(0.0)
    }

    pub fn set_yield(&mut self, rod: &RodType, v: f64) {
        self.ensure(rod);
        self.yield_ = Some(v);
    }

    pub fn set_poison(&mut self, rod: &RodType, v: f64) {
        self.ensure(rod);
        self.xenon = Some(v);
    }

    pub fn set_core_heat(&mut self, rod: &RodType, v: f64) {
        self.ensure(rod);
        self.core = Some(v);
    }

    pub fn set_hull_heat(&mut self, rod: &RodType, v: f64) {
        self.ensure(rod);
        self.hull = Some(v);
    }

    /// `getEnrichment`: yield over the rod's full yield [0;1].
    pub fn enrichment(&mut self, rod: &RodType) -> f64 {
        self.yield_(rod) / rod.yield_
    }

    /// `getPoisonLevel` [0;1].
    pub fn poison_level(&mut self, rod: &RodType) -> f64 {
        self.poison(rod) / 100.0
    }

    /// `getDurabilityForDisplay`.
    pub fn durability_for_display(&mut self, rod: &RodType) -> f64 {
        1.0 - self.enrichment(rod)
    }

    /// `burn`: adjusts the flux by poison, generates then burns xenon,
    /// computes the outflux and depletes the yield. Heats the core.
    pub fn burn(&mut self, rod: &RodType, dials: &Dials, mut in_flux: f64) -> f64 {
        in_flux += rod.self_rate;

        if dials.xenon() {
            let mut xenon = self.poison(rod);
            xenon -= rod.xenon_burn(in_flux);
            in_flux *= 1.0 - self.poison_level(rod);
            xenon += rod.xenon_gen(in_flux);
            if xenon < 0.0 {
                xenon = 0.0;
            }
            if xenon > 100.0 {
                xenon = 100.0;
            }
            self.set_poison(rod, xenon);
        }

        let mut mult = 1.0;
        let mut core_heat = self.core_heat(rod);

        if rod.heat_coeff_start != 0.0 && core_heat >= rod.heat_coeff_start {
            let mut prog = (core_heat - rod.heat_coeff_start) / rod.heat_coeff_length;
            if prog > 1.0 {
                prog = 1.0;
            }
            mult = jmath::sin((prog * PI + PI) / 2.0);
        }

        let out_flux = rod.reactivity_func(in_flux, self.enrichment(rod) * mult) * dials.reactivity_mod;

        if dials.depletion() {
            let mut y = self.yield_(rod);
            y -= in_flux;
            if y < 0.0 {
                y = 0.0;
            }
            self.set_yield(rod, y);
        }

        core_heat += out_flux * rod.heat;
        self.set_core_heat(rod, rectify(core_heat));
        out_flux
    }

    /// `updateHeat`: core heat moves into the hull.
    pub fn update_heat(&mut self, rod: &RodType, dials: &Dials, m: f64) {
        let mut core = self.core_heat(rod);
        let mut hull = self.hull_heat(rod);

        if core > hull {
            let mid = (core - hull) / 2.0;
            core -= mid * rod.diffusion * dials.fuel_diffusion_mod * m;
            hull += mid * rod.diffusion * dials.fuel_diffusion_mod * m;
            self.set_core_heat(rod, rectify(core));
            self.set_hull_heat(rod, rectify(hull));
        }
    }

    /// `provideHeat`: one tick's worth of hull heat for the column. Past the
    /// melting point, core and hull jump to a third of the sum each and the
    /// excess goes to the column (always fatal).
    pub fn provide_heat(&mut self, rod: &RodType, dials: &Dials, heat: f64, m: f64) -> f64 {
        let mut hull = self.hull_heat(rod);

        if hull > rod.melting_point {
            let core = self.core_heat(rod);
            let avg = (heat + hull + core) / 3.0;
            self.set_core_heat(rod, avg);
            self.set_hull_heat(rod, avg);
            return avg - heat;
        }

        if hull <= heat {
            return 0.0;
        }

        let mut ret = (hull - heat) / 2.0;
        ret *= dials.heat_provision * m;
        hull -= ret;
        self.set_hull_heat(rod, hull);
        ret
    }
}
