//! Client animation timing that drives AudioWrapper pitch/volume and assembler
//! cues, translated from the matching Java onClientTick/updateEntity methods.
use super::{Cue, Envelope, Loop, cue};
use crate::bedrock::JavaRandom;

pub struct Motion {
    pub ramp: i32,
    spin: f32,
    hold: u32,
    desync: f32,
    active: bool,
    lid: Option<f64>,
    cycle: u32,
    random: JavaRandom,
    arms: [Arm; 2],
    ring: f64,
    target: f64,
    speed: f64,
    delay: i32,
}
impl Motion {
    pub fn new(seed: i64) -> Self {
        let mut random = JavaRandom::new(seed);
        Self {
            ramp: 0,
            spin: 0.,
            hold: 0,
            desync: random.next_float() * 0.05,
            active: false,
            lid: None,
            cycle: 0,
            random,
            arms: [
                Arm::new(seed.wrapping_add(1)),
                Arm::new(seed.wrapping_add(2)),
            ],
            ring: 0.,
            target: 0.,
            speed: 0.,
            delay: 0,
        }
    }
    /// Returns the live loop's unattenuated gain/pitch and this tick's cues.
    pub fn step(&mut self, spec: &Loop) -> (f32, f32, Vec<Cue>) {
        let active = spec.volume > 0.;
        let mut cues = Vec::new();
        let pair = match spec.envelope {
            Envelope::ArcLid => {
                let old = self.lid.replace(spec.value);
                let moved = old.is_some_and(|old| old != spec.value);
                if moved && (spec.value == 0. || spec.value == 1.) && old != Some(0.) {
                    cues.push(cue("hbm:door.wgh_stop", 1., 1., [0.; 3]));
                }
                (if moved { spec.volume } else { 0. }, spec.pitch)
            }
            Envelope::Plain => (spec.volume, spec.pitch),
            Envelope::Centrifuge => {
                self.ramp = (self.ramp + if active { 2 } else { -3 }).clamp(0, 60);
                (
                    if self.ramp > 10 { 1. } else { 0. },
                    (self.ramp - 10) as f32 / 100. + 0.5,
                )
            }
            Envelope::Turbofan => {
                self.ramp = (self.ramp + if active { 1 } else { -1 }).clamp(0, 100);
                (
                    self.ramp as f32 / 50.,
                    self.ramp as f32 / 200. + 0.5 + spec.pitch,
                )
            }
            Envelope::Leviathan => {
                self.hold = self.hold.saturating_sub(1);
                if active {
                    self.hold = 25;
                }
                self.spin = (self.spin
                    + if self.hold > 0 {
                        0.075 + self.desync
                    } else {
                        -0.1
                    })
                .clamp(0., 25.);
                (0.5 * self.spin / 25., 0.25 + 0.75 * self.spin / 25.)
            }
            Envelope::Refinery => {
                if active {
                    self.hold = 20;
                }
                let volume = if self.hold > 0 {
                    self.hold -= 1;
                    1.
                } else {
                    0.
                };
                (volume, spec.pitch)
            }
            Envelope::Industrial => (spec.volume, spec.pitch + self.desync),
            Envelope::Assembler => {
                if self.active && !active {
                    cues.push(cue("hbm:block.assemblerStop", 0.25, 1.5, [0.; 3]));
                }
                for arm in &mut self.arms {
                    if arm.step(active) {
                        cues.push(cue("hbm:block.assemblerStrike", 0.5, 1., [0.; 3]));
                    }
                }
                if active {
                    if self.ring != self.target {
                        if (self.target - self.ring).abs() <= self.speed {
                            self.ring = self.target;
                        }
                        if self.target > self.ring {
                            self.ring += self.speed;
                        }
                        if self.target < self.ring {
                            self.ring -= self.speed;
                        }
                        if self.target == self.ring {
                            let offset = if self.target >= 360. { -360. } else { 360. };
                            self.target += offset;
                            self.ring += offset;
                            self.delay = 20 + self.random.next_int(21);
                        }
                    } else {
                        self.delay -= 1;
                        if self.delay <= 0 {
                            self.target += (self.random.next_double() * 2. - 1.) * 135.;
                            self.speed = 10. + self.random.next_double() * 5.;
                            cues.push(cue(
                                "hbm:block.assemblerStart",
                                0.25,
                                1.25 + self.random.next_float() * 0.25,
                                [0.; 3],
                            ));
                        }
                    }
                }
                (spec.volume, spec.pitch)
            }
            Envelope::Shredder => {
                if !active {
                    self.cycle = 0;
                }
                if active {
                    if self.cycle == 0 {
                        cues.push(Cue {
                            event: "minecraft:entity.minecart.riding",
                            volume: 1.,
                            pitch: 0.75,
                            offset: [0.; 3],
                        });
                    }
                    self.cycle = (self.cycle + 1) % 50;
                }
                (0., spec.pitch)
            }
        };
        self.active = active;
        (pair.0, pair.1, cues)
    }
}
struct Arm {
    angles: [f64; 4],
    target: [f64; 4],
    state: u8,
    delay: i32,
    random: JavaRandom,
}
impl Arm {
    fn new(seed: i64) -> Self {
        Self {
            angles: [0.; 4],
            target: [0.; 4],
            state: 0,
            delay: 0,
            random: JavaRandom::new(seed),
        }
    }
    fn advance(&mut self, speed: [f64; 4]) -> bool {
        let mut moved = false;
        for (i, speed) in speed.into_iter().enumerate() {
            if self.angles[i] == self.target[i] {
                continue;
            }
            moved = true;
            let delta = self.target[i] - self.angles[i];
            self.angles[i] += delta.clamp(-speed, speed);
        }
        !moved
    }
    fn step(&mut self, active: bool) -> bool {
        let before = self.angles[3];
        if !active {
            self.target = [0.; 4];
            self.state = 2;
            self.advance([3., 3., 3., 0.25]);
        } else if self.delay > 0 {
            self.delay -= 1;
        } else {
            let reached = self.advance([15., 15., 15., 0.5]);
            match self.state {
                0 if reached => {
                    self.delay = 2;
                    self.state = 1;
                    self.target[3] = -0.75;
                }
                1 if reached => {
                    self.state = 2;
                    self.target[3] = 0.;
                }
                2 if reached => {
                    self.delay = 2 + self.random.next_int(5);
                    const POS: [[f64; 3]; 5] = [
                        [45., -15., -5.],
                        [15., 15., -15.],
                        [25., 10., -15.],
                        [30., 0., -10.],
                        [70., -10., -25.],
                    ];
                    self.target[..3].copy_from_slice(&POS[self.random.next_int(5) as usize]);
                    self.state = 0;
                }
                _ => {}
            }
        }
        before != self.angles[3] && self.angles[3] == -0.75
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn centrifuge_spools_up_and_down_in_java_steps() {
        let mut s = super::super::sound("hbm:block.centrifugeOperate", 1., 1., 10.);
        s.envelope = Envelope::Centrifuge;
        let mut m = Motion::new(5);
        for _ in 0..5 {
            assert_eq!(m.step(&s).0, 0.);
        }
        assert_eq!(m.step(&s).0, 1.);
        for _ in 0..24 {
            m.step(&s);
        }
        assert_eq!(m.step(&s).1, 1.);
        s.volume = 0.;
        assert_eq!(m.step(&s).1, 0.97);
        for _ in 0..16 {
            m.step(&s);
        }
        assert_eq!(m.step(&s).0, 0.);
    }
    #[test]
    fn assembler_strikes_only_when_its_striker_reaches_the_workpiece() {
        let mut s = super::super::sound("hbm:block.motor", 0.5, 0.75, 15.);
        s.envelope = Envelope::Assembler;
        let mut m = Motion::new(1);
        for _ in 0..4 {
            assert!(
                m.step(&s)
                    .2
                    .iter()
                    .all(|c| c.event != "ntm:block.assemblerstrike")
            );
        }
        assert_eq!(
            m.step(&s)
                .2
                .iter()
                .filter(|c| c.event == "ntm:block.assemblerstrike")
                .count(),
            2
        );
        s.volume = 0.;
        assert_eq!(m.step(&s).2[0].event, "ntm:block.assemblerstop");
        assert!(m.step(&s).2.is_empty());
    }
}

#[cfg(test)]
mod arc_tests {
    use super::*;
    #[test]
    fn arc_lid_does_not_thud_on_initial_load_but_thuds_at_the_end_of_a_stroke() {
        let mut spec = super::super::sound("hbm:door.wgh_start", 0.75, 1., 15.);
        spec.envelope = Envelope::ArcLid;
        spec.value = 1.;
        let mut motion = Motion::new(1);
        assert_eq!(motion.step(&spec).0, 0.);
        spec.value = 0.5;
        assert_eq!(motion.step(&spec).0, 0.75);
        spec.value = 0.;
        let (_, _, cues) = motion.step(&spec);
        assert_eq!(cues[0].event, "ntm:door.wgh_stop");
        assert!(motion.step(&spec).2.is_empty());
    }
}
