//! Efeitos secundários nas bordas expostas da cratera.
//!
//! Uma borda é um voxel lido, não-ar e sem energia para ser destruído; ela é
//! "exposta" quando o voxel de cima ainda tinha energia (ar ou removido). As
//! bordas saem de graça da varredura (nenhuma leitura extra), em ordem do
//! centro para fora. Regras padrão: `FalloutConfigJSON` do NTM (linhas 54-120)
//! e `EntityFalloutRain.stomp` (fogo 1/5 até 65% do raio, linhas 182-185).
use crate::crater::{Kind, rand01};

/// Uma borda exposta, para o perfil ou para um gancho.
#[derive(Clone, Copy, Debug)]
pub struct EdgeHit {
    pub pos: [i32; 3],
    pub kind: Kind,
    /// Distância ao centro / raio.
    pub f: f32,
}

/// O que fazer numa borda: o bloco dela e o bloco logo acima (fogo, cinza...).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdgeEffect {
    pub here: Option<String>,
    pub above: Option<String>,
}

/// Perfil de efeitos de uma bomba. Os blocos são estados (`ntm:waste_log`,
/// `minecraft:fire`...); `None` deixa o tipo de borda como está.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectProfile {
    pub name: String,
    /// Escória de pedra/terra por faixa: (até f, bloco), em ordem crescente.
    pub slag: Vec<(f32, String)>,
    /// Areia: chance de trinitite e o bloco (o resto vira escória).
    pub trinitite: Option<(f32, String)>,
    pub wood: Option<String>,
    pub planks: Option<String>,
    /// Folhas e plantas expostas: este bloco (normalmente ar).
    pub foliage: Option<String>,
    /// Fogo acima de madeira exposta: (chance, até f, bloco).
    pub fire: Option<(f32, f32, String)>,
    /// Segunda fase: fallout até `scale`·R (None: só a cratera).
    pub fallout: Option<crate::fallout::FalloutProfile>,
}

impl EffectProfile {
    /// Regras do NTM com blocos vanilla no lugar dos do NTM, até o content mod
    /// ter `ntm:sellafield_slaked`, `ntm:waste_trinitite`, `ntm:waste_log`...
    pub fn ntm_placeholder() -> Self {
        Self {
            name: "ntm-placeholder".into(),
            slag: vec![
                (0.4, "minecraft:blackstone".into()),
                (0.75, "minecraft:deepslate".into()),
                (f32::INFINITY, "minecraft:tuff".into()),
            ],
            trinitite: Some((0.05, "minecraft:green_stained_glass".into())),
            wood: Some("minecraft:basalt".into()),
            planks: Some("minecraft:dark_oak_planks".into()),
            foliage: Some("minecraft:air".into()),
            fire: Some((0.2, 0.65, "minecraft:fire".into())),
            fallout: Some(crate::fallout::FalloutProfile::ntm_placeholder()),
        }
    }

    /// Só a cratera, sem efeitos.
    pub fn none() -> Self {
        Self {
            name: "none".into(),
            slag: Vec::new(),
            trinitite: None,
            wood: None,
            planks: None,
            foliage: None,
            fire: None,
            fallout: None,
        }
    }

    pub fn is_none(&self) -> bool {
        self.slag.is_empty() && self.trinitite.is_none() && self.wood.is_none() && self.planks.is_none() && self.foliage.is_none() && self.fire.is_none() && self.fallout.is_none()
    }

    fn slag_at(&self, f: f32) -> Option<String> {
        self.slag.iter().find(|(max, _)| f < *max).map(|(_, b)| b.clone())
    }

    /// Efeito padrão numa borda exposta (determinístico pela posição).
    pub fn apply(&self, hit: &EdgeHit) -> EdgeEffect {
        let [x, y, z] = hit.pos;
        let fire = self
            .fire
            .as_ref()
            .filter(|(chance, max_f, _)| hit.f <= *max_f && rand01(x, y, z, 11) < *chance)
            .map(|(_, _, b)| b.clone());
        match hit.kind {
            Kind::Rock | Kind::Soil => EdgeEffect { here: self.slag_at(hit.f), above: None },
            Kind::Sand => {
                let here = match &self.trinitite {
                    Some((chance, b)) if rand01(x, y, z, 7) < *chance => Some(b.clone()),
                    _ => self.slag_at(hit.f),
                };
                EdgeEffect { here, above: None }
            }
            Kind::Wood => EdgeEffect { here: self.wood.clone(), above: fire },
            Kind::Planks => EdgeEffect { here: self.planks.clone(), above: fire },
            Kind::Leaves | Kind::Plant => EdgeEffect { here: self.foliage.clone(), above: None },
            _ => EdgeEffect::default(),
        }
    }

    /// Texto de uma linha (vai no cabeçalho persistido).
    pub fn encode(&self) -> String {
        let o = |v: &Option<String>| v.clone().unwrap_or_default();
        let slag: Vec<String> = self.slag.iter().map(|(f, b)| format!("{f}:{b}")).collect();
        let fallout = self.fallout.as_ref().map(|f| format!("|{}", f.encode())).unwrap_or_default();
        format!(
            "{}|{}|{}|{}|{}|{}|{}{fallout}",
            self.name,
            slag.join(" "),
            self.trinitite.as_ref().map(|(c, b)| format!("{c}:{b}")).unwrap_or_default(),
            o(&self.wood),
            o(&self.planks),
            o(&self.foliage),
            self.fire.as_ref().map(|(c, m, b)| format!("{c}:{m}:{b}")).unwrap_or_default(),
        )
    }

    pub fn decode(s: &str) -> Result<Self, String> {
        let p: Vec<&str> = s.split('|').collect();
        if p.len() != 7 && p.len() != 8 {
            return Err(format!("perfil: {s}"));
        }
        let opt = |v: &str| (!v.is_empty()).then(|| v.to_string());
        // "0.4:minecraft:blackstone": o número vai até o primeiro ':'.
        let num_block = |v: &str| -> Result<(f32, String), String> {
            let (n, b) = v.split_once(':').ok_or(format!("perfil: {v}"))?;
            Ok((n.parse().map_err(|e| format!("{n}: {e}"))?, b.to_string()))
        };
        let slag = p[1].split(' ').filter(|v| !v.is_empty()).map(num_block).collect::<Result<_, _>>()?;
        let trinitite = if p[2].is_empty() { None } else { Some(num_block(p[2])?) };
        let fire = if p[6].is_empty() {
            None
        } else {
            let (c, rest) = num_block(p[6])?;
            let (m, b) = num_block(&rest)?;
            Some((c, m, b))
        };
        Ok(Self {
            name: p[0].into(),
            slag,
            trinitite,
            wood: opt(p[3]),
            planks: opt(p[4]),
            foliage: opt(p[5]),
            fire,
            fallout: match p.get(7) {
                Some(f) => Some(crate::fallout::FalloutProfile::decode(f)?),
                None => None,
            },
        })
    }
}

/// Eventos de uma explosão, para o mod reagir (mensagem, som, dano...).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Primeira escrita no mundo.
    FirstWrite { tick: u64 },
    /// Todas as seções removidas (efeitos ainda podem estar rodando).
    CraterDone { tick: u64 },
    /// Tudo pronto, inclusive efeitos e a passada do fallout (chunks que não
    /// estavam carregados ficam para quando um jogador chegar perto).
    Done { tick: u64 },
    /// Cratera e bordas prontas: começa o fallout.
    FalloutStart { tick: u64 },
    /// A persistência falhou (quota?): a explosão continua sem ela.
    PersistFailed { reason: String },
    /// Retomada depois de reinício.
    Resumed { columns: u32 },
    /// A retomada falhou (estado ausente ou ilegível): as chaves saem do storage.
    ResumeFailed { reason: String },
}

/// Ganchos opcionais de quem usa o motor.
pub trait Hooks {
    /// Troca o efeito de uma borda exposta; `None` usa o perfil da explosão.
    fn edge(&mut self, _id: &str, _hit: &EdgeHit) -> Option<EdgeEffect> {
        None
    }
    fn event(&mut self, _id: &str, _event: &Event) {}
}

/// Sem ganchos.
pub struct NoHooks;
impl Hooks for NoHooks {}
