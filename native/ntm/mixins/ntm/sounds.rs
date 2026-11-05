// Client-owned AudioWrapper instances. Only the compact audio snapshot travels
// through the existing block-entity update tag; inventories stay server-only.
use horizon_client::{
    containers::BlockEntity,
    sound_engine::{Attenuation, InstanceId, SoundEngine, SoundInstance, Sounds, Ticking},
};
use horizon_core::random::RandomSource;
use horizon_world::nbt::{JavaString, Tag};
use ntm_core::sounds::{Loop, Motion};
use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

type Key = ([i32; 3], u64, u8);
struct Playing {
    id: Option<InstanceId>,
    event: String,
    motion: Motion,
    retry: u8,
}
#[derive(Default)]
struct Client {
    pending: Vec<(u32, [f64; 3])>,
    dimension: String,
    last: Option<Instant>,
    playing: HashMap<Key, Playing>,
}
static CLIENTS: LazyLock<Mutex<HashMap<[i32; 4], Client>>> = LazyLock::new(Default::default);

fn diagnostic() -> bool {
    std::env::var("HORIZON_AUDIO_TEST_MIXER").as_deref() == Ok("1")
}

fn stop(game: &mut crate::game::Game<'_>, p: &mut Playing) {
    if let Some(id) = p.id.take() {
        if diagnostic() { eprintln!("ntm audio release instance={id} channel={:?} event={}", game.audio.engine().channel(id), p.event); }
        game.audio.stop_and_release(id);
    }
}
pub(super) fn clear(game: &mut crate::game::Game<'_>) {
    if !game.native_window() {
        return;
    }
    let owner = game.level.ensure_player_uuid();
    if let Some(mut client) = CLIENTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&owner)
    {
        for p in client.playing.values_mut() {
            stop(game, p);
        }
    }
}
pub(super) fn frame(game: &mut crate::game::Game<'_>) {
    if !game.native_window() {
        return;
    }
    if game.title.is_some() {
        clear(game);
        return;
    }
    let owner = game.level.ensure_player_uuid();
    let mut clients = CLIENTS.lock().unwrap_or_else(|e| e.into_inner());
    let client = clients.entry(owner).or_default();
    let dimension = format!("{:?}", game.dim);
    if client.dimension != dimension {
        for p in client.playing.values_mut() {
            stop(game, p);
        }
        client.playing.clear();
        client.pending.clear();
        client.dimension = dimension;
        client.last = None;
    }
    // Preserve fixed 20 Hz ticks and their remainder even at a low frame rate.
    // Audio's vanilla pause keeps retained channels at their sample offsets.
    let now = Instant::now();
    if game.pause {
        client.last = Some(now);
        return;
    }
    let last = client.last.unwrap_or(now - Duration::from_millis(50));
    let steps = (now.duration_since(last).as_millis() / 50).min(20) as u32;
    client.last = Some(last + Duration::from_millis(u64::from(steps) * 50));
    for _ in 0..steps {
        tick(game, client);
    }
}
fn tick(game: &mut crate::game::Game<'_>, client: &mut Client) {
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let at = [player.position.x, player.position.y, player.position.z];
    client.pending.retain_mut(|(age, position)| {
        *age += 1;
        let distance = (0..3)
            .map(|i| (position[i] - at[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if ntm_core::sounds::nuclear_arrived(*age, distance) {
            if diagnostic() { eprintln!("ntm nuclear arrived age={age} distance={distance} at={position:?}"); }
            game.audio.play_on(
                4,
                "ntm:weapon.nuclearexplosion",
                Some(*position),
                10_000.,
                1.,
            );
            false
        } else {
            *age < 149
        }
    });
    let mut wanted = HashSet::new();
    let mut requests = Vec::new();
    for (pos, be) in &game.world.block_entities {
        let BlockEntity::Mod(be) = be else { continue };
        let Some(Tag::String(json)) = be.nbt.get(&JavaString::from("ntm_audio")) else {
            continue;
        };
        let Ok(loops) = serde_json::from_str::<Vec<Loop>>(&json.to_string_lossy()) else {
            continue;
        };
        let key = (
            [pos.0, pos.1, pos.2],
            game.world.block_entities.identity(pos).unwrap_or(0),
        );
        for spec in loops {
            requests.push(((key.0, key.1, spec.slot), spec));
        }
    }
    for (key, spec) in requests {
        wanted.insert(key);
        let p = client.playing.entry(key).or_insert_with(|| Playing {
            id: None,
            event: spec.event.clone(),
            motion: Motion::new(SoundEngine::unseeded().next_long()),
            retry: 0,
        });
        if p.event != spec.event {
            stop(game, p);
            p.event = spec.event.clone();
            p.motion = Motion::new(SoundEngine::unseeded().next_long());
        }
        let position = [0, 1, 2].map(|i| f64::from(key.0[i]) + spec.offset[i]);
        let distance = (0..3)
            .map(|i| (position[i] - at[i]).powi(2))
            .sum::<f64>()
            .sqrt() as f32;
        let (volume, pitch, cues) = p.motion.step(&spec);
        for cue in cues {
            let source = ntm_core::sounds::category(cue.event)
                .and_then(|category| {
                    horizon_client::settings::SOUND_SOURCES
                        .iter()
                        .position(|s| *s == category)
                })
                .unwrap_or_else(|| horizon_client::settings::sound_source(cue.event));
            game.audio
                .play_on(source, cue.event, Some(position), cue.volume, cue.pitch);
        }
        let volume = ntm_core::sounds::gain(volume, spec.range, distance);
        if volume <= 0. {
            stop(game, p);
            continue;
        }
        if let Some(id) = p.id {
            if let Some(instance) = game.audio.engine().instance_mut(id) {
                instance.position = position;
                instance.volume = volume;
                instance.pitch = pitch;
            } else {
                p.id = None;
            }
        }
        p.retry = p.retry.saturating_sub(1);
        if p.retry == 0 && p.id.is_none_or(|id| !game.audio.is_active(id)) {
            stop(game, p);
            let source = ntm_core::sounds::category(&spec.event)
                .and_then(|category| {
                    horizon_client::settings::SOUND_SOURCES
                        .iter()
                        .position(|s| *s == category)
                })
                .unwrap_or(4);
            let mut instance = SoundInstance::at(
                &spec.event,
                source,
                volume,
                pitch,
                SoundEngine::unseeded(),
                position,
            );
            instance.looping = true;
            instance.attenuation = Attenuation::None;
            instance.ticking = Ticking::External;
            instance.can_start_silent = true;
            p.id = Some(game.audio.play_instance(instance, true).0);
            if diagnostic() { eprintln!("ntm audio owner key={key:?} instance={:?} event={}", p.id, p.event); }
            p.retry = 20;
        }
    }
    client.playing.retain(|key, p| {
        if wanted.contains(key) {
            true
        } else {
            stop(game, p);
            false
        }
    });
}

/// Publish a committed item cue to nearby players in the actor's dimension.
/// K2 passes its original Java event key here; unregistered originals stay silent.
pub(super) fn player(game: &mut crate::game::Game<'_>, java: &str, volume: f32, pitch: f32) {
    let Some(event) = ntm_core::sounds::event(java) else {
        return;
    };
    let source = ntm_core::sounds::category(event)
        .and_then(|category| {
            horizon_client::settings::SOUND_SOURCES
                .iter()
                .position(|s| *s == category)
        })
        .unwrap_or(7);
    let Some(player) = game.player.as_ref() else {
        return;
    };
    let at = [player.position.x, player.position.y, player.position.z];
    game.with_level(|_, level, _| {
        level.native_play_sound(event, at, source, volume, pitch);
    });
}

/// The official mod-message hook delivers the beginning of a Torex effect.
pub(super) fn receive(game: &mut crate::game::Game<'_>, channel: &str, data: &[u8]) {
    if channel != "ntm:nuclear_sound" || data.len() > 2048 {
        return;
    }
    let Ok(cue) = serde_json::from_slice::<ntm_core::sounds::NuclearCue>(data) else {
        return;
    };
    let dimension = format!("{:?}", game.dim);
    if cue.dimension != dimension
        || cue
            .at
            .iter()
            .any(|n| !n.is_finite() || n.abs() > 30_000_000.)
    {
        return;
    }
    let owner = game.level.ensure_player_uuid();
    let changed = CLIENTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&owner)
        .is_some_and(|c| c.dimension != dimension);
    if changed {
        clear(game);
    }
    let mut clients = CLIENTS.lock().unwrap_or_else(|e| e.into_inner());
    let client = clients.entry(owner).or_default();
    client.dimension = dimension;
    client.pending.push((0, cue.at));
    if diagnostic() { eprintln!("ntm nuclear received dimension={} at={:?}", cue.dimension, cue.at); }
}
