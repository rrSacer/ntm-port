//! The RBMK crane (`TileEntityCraneConsole`, T11) and what it carries rods between: the storage column's queue
//! (`TileEntityRBMKStorage`) and the columns it loads and unloads (`IRBMKLoadable`). Pure: the server finds the
//! columns, reads the players' keys and publishes a transfer as one commit of both blocks.
use crate::rbmk::persist;
use crate::server::container_bridge::{Slotted, Slots, Stack};
use serde::{Deserialize, Serialize};

/// The crane console's block.
pub const BLOCK: &str = "ntm:rbmk_crane_console";

/// The storage column's block.
pub const STORAGE: &str = "ntm:rbmk_storage";

/// `TileEntityRBMKStorage.updateEntity` every 10 ticks: one ascending pass, each successor into the gap before it
/// (an item moves one place a pass; a queue with gaps closes over several).
pub fn storage_pass<T>(slots: &mut [Option<T>]) {
    for i in 0..slots.len().saturating_sub(1) {
        if slots[i].is_none() && slots[i + 1].is_some() {
            slots[i] = slots[i + 1].take();
        }
    }
}

/// Which slot a column is loaded into and unloaded from (`IRBMKLoadable`): the channels' one slot; the storage takes
/// in at 11 and gives out from 0. None: not loadable.
pub fn loadable_slots(block: &str) -> Option<(usize, usize)> {
    if persist::is_channel(block) {
        Some((0, 0))
    } else if block == STORAGE {
        Some((11, 0))
    } else {
        None
    }
}

/// `canLoad`: the slot it loads into is empty (and, for a channel, there is something to load).
pub fn can_load<T>(block: &str, slots: &[Option<T>]) -> bool {
    loadable_slots(block).is_some_and(|(into, _)| slots.get(into).is_some_and(Option::is_none))
}

/// `canUnload`: the slot it gives from holds something.
pub fn can_unload<T>(block: &str, slots: &[Option<T>]) -> bool {
    loadable_slots(block).is_some_and(|(_, from)| slots.get(from).is_some_and(Option::is_some))
}

/// `TileEntityCraneConsole.speed`: what the crane travels a tick a key is held.
pub const SPEED: f64 = 0.05;
/// The hook's step down or up a tick.
pub const LIFT: f64 = 0.04;
/// `findRoomExtent`'s reach.
pub const MAX_SPAN: i32 = 16;

/// The crane keys a player holds (`HbmKeybinds.EnumKeybind.CRANE_*`), as the client sends them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub load: bool,
}

impl Keys {
    pub fn bits(self) -> u8 {
        u8::from(self.up) | u8::from(self.down) << 1 | u8::from(self.left) << 2 | u8::from(self.right) << 3 | u8::from(self.load) << 4
    }
    pub fn from_bits(b: u8) -> Keys {
        Keys { up: b & 1 != 0, down: b & 2 != 0, left: b & 4 != 0, right: b & 8 != 0, load: b & 16 != 0 }
    }
}

fn one() -> f64 {
    1.0
}

/// The crane console's state. Saved as `writeToNBT` saves it: whether a crane is set up, its turn (only the bridge's
/// look), the centre over the reactor, the four spans, its height, where it stands; the item it holds is its one
/// hidden slot (`held`, T11 decision 2). In memory only, as in the Java (a reopened world has the hook up and still):
/// the hook's height (`progress`, 1 up, 0 down), whether it goes down, the joystick's tilts; and what the server sets
/// before a step (the keys of the player at the console) and reads after it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Crane {
    pub set_up: bool,
    pub rotation_offset: i32,
    pub center: [i32; 3],
    pub span_f: i32,
    pub span_b: i32,
    pub span_l: i32,
    pub span_r: i32,
    pub height: i32,
    #[serde(with = "persist::bits")]
    pub pos_front: f64,
    #[serde(with = "persist::bits")]
    pub pos_left: f64,
    /// The engine's one slot: the item the crane holds.
    pub slots: Slots,
    #[serde(skip, default = "one")]
    pub progress: f64,
    #[serde(skip)]
    pub goes_down: bool,
    #[serde(skip)]
    pub tilt: [f64; 2],
    /// The keys of the first player in the console's area this tick, none when nobody stands there.
    #[serde(skip)]
    pub input: Option<Keys>,
    /// The hook reached the bottom in the last step: the server tries the transfer.
    #[serde(skip)]
    pub arrived: bool,
    /// What the console's lamps and meters show (`isAboveValidTarget`, `loadedHeat`, `loadedEnrichment`).
    #[serde(skip)]
    pub valid_target: bool,
    #[serde(skip)]
    pub meters: [f64; 2],
}

impl Default for Crane {
    fn default() -> Self {
        Crane {
            set_up: false,
            rotation_offset: 0,
            center: [0; 3],
            span_f: 0,
            span_b: 0,
            span_l: 0,
            span_r: 0,
            height: 0,
            pos_front: 0.0,
            pos_left: 0.0,
            slots: vec![None],
            progress: 1.0,
            goes_down: false,
            tilt: [0.0; 2],
            input: None,
            arrived: false,
            valid_target: false,
            meters: [0.0; 2],
        }
    }
}

/// `ForgeDirection.getRotation(UP)` of a horizontal direction (x, z): north → east → south → west.
fn rot_up([x, z]: [i32; 2]) -> [i32; 2] {
    [-z, x]
}

/// `getRotation(DOWN)`: the other way round.
fn rot_down([x, z]: [i32; 2]) -> [i32; 2] {
    [z, -x]
}

impl Crane {
    /// `isCraneLoading`: the hook is not all the way up.
    pub fn loading(&self) -> bool {
        self.progress != 1.0
    }

    /// The item it holds.
    pub fn held(&self) -> Option<&Stack> {
        self.slots.first().and_then(Option::as_ref)
    }

    /// `updateEntity` on the server: the hook down (to the bottom: `arrived`, and the server tries the transfer) or up;
    /// then, the hook up, the keys of the player at the console move the crane (a key against its opposite does
    /// nothing) or send the hook down; the position held within the spans.
    pub fn step(&mut self) {
        self.arrived = false;
        if self.goes_down {
            if self.progress > 0.0 {
                self.progress -= LIFT;
            } else {
                self.progress = 0.0;
                self.goes_down = false;
                self.arrived = true;
            }
        } else if self.progress != 1.0 {
            self.progress += LIFT;
            if self.progress > 1.0 {
                self.progress = 1.0;
            }
        }
        self.tilt = [0.0; 2];
        if let Some(k) = self.input.filter(|_| !self.loading()) {
            if k.up && !k.down {
                self.tilt[0] = 30.0;
                self.pos_front += SPEED;
            }
            if !k.up && k.down {
                self.tilt[0] = -30.0;
                self.pos_front -= SPEED;
            }
            if k.left && !k.right {
                self.tilt[1] = 30.0;
                self.pos_left += SPEED;
            }
            if !k.left && k.right {
                self.tilt[1] = -30.0;
                self.pos_left -= SPEED;
            }
            if k.load {
                self.goes_down = true;
            }
        }
        // `MathHelper.clamp_double`: a NaN stays.
        self.pos_front = clamp(self.pos_front, -f64::from(self.span_b), f64::from(self.span_f));
        self.pos_left = clamp(self.pos_left, -f64::from(self.span_r), f64::from(self.span_l));
    }

    /// `getColumnAtPos`'s cell: under the hook, a cell below the centre's height, at `floor(pos + 0.5)` from the console
    /// facing `facing` (x, z).
    pub fn column_cell(&self, facing: [i32; 2]) -> [i32; 3] {
        let left = rot_down(facing);
        let at = |c: i32, d: i32, l: i32| (f64::from(c) - f64::from(d) * self.pos_front - f64::from(l) * self.pos_left + 0.5).floor() as i32;
        [at(self.center[0], facing[0], left[0]), self.center[1] - 1, at(self.center[2], facing[1], left[1])]
    }

    /// `setTarget` from the RBMK tool, linked to the column whose core is `core`: the centre a cell over the column's
    /// top, the spans the free room around it at the girders' height (`centerY + 6`), forwards (away from the console),
    /// right, back, left; the height 7. `air` says whether a cell is air; an unknown one (a chunk not loaded) stops a
    /// span (D4: never air). Where the crane stands is kept (and held in the new spans at its next step).
    pub fn link(&mut self, core: [i32; 3], column_height: i32, facing: [i32; 2], mut air: impl FnMut([i32; 3]) -> Option<bool>) {
        self.center = [core[0], core[1] + column_height + 1, core[2]];
        let girder = self.center[1] + 6;
        let mut extent = |d: [i32; 2]| (1..MAX_SPAN).find(|i| air([core[0] + d[0] * i, girder, core[2] + d[1] * i]) != Some(true)).map_or(MAX_SPAN, |i| i - 1);
        let mut d = [-facing[0], -facing[1]];
        self.span_f = extent(d);
        d = rot_up(d);
        self.span_r = extent(d);
        d = rot_up(d);
        self.span_b = extent(d);
        d = rot_up(d);
        self.span_l = extent(d);
        self.height = 7;
        self.set_up = true;
    }

    /// `cycleCraneRotation`: the bridge a quarter turn (only its look).
    pub fn rotate(&mut self) {
        self.rotation_offset = (self.rotation_offset + 90) % 360;
    }

    /// The model's values (`horizon_model/rbmk_crane_console.json`): [2] the heat meter and [3] the enrichment meter by
    /// a thousandth, [4] the loading lamp (0 off, 1 loading, 2 loaded), [5] the target lamp.
    pub fn model_data(&self) -> Vec<i32> {
        let lamp = if self.loading() { 1 } else if self.held().is_some() { 2 } else { 0 };
        vec![0, 0, (self.meters[0] * 1000.0) as i32, (self.meters[1] * 1000.0) as i32, lamp, i32::from(self.valid_target)]
    }
}

/// A crane console's waiting link or turns, applied in its transaction (T11).
pub fn apply(crane: &mut Crane, pending: &crate::rbmk::screen::Pending) {
    use crate::rbmk::screen::Pending;
    match pending {
        Pending::CraneLink(center, [f, r, b, l]) => {
            (crane.center, crane.span_f, crane.span_r, crane.span_b, crane.span_l) = (*center, *f, *r, *b, *l);
            crane.height = 7;
            crane.set_up = true;
        }
        Pending::Rotate(n) => {
            for _ in 0..*n {
                crane.rotate();
            }
        }
        _ => {}
    }
}

fn clamp(v: f64, min: f64, max: f64) -> f64 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// The box a player stands in to work the crane console at `core` facing `facing`: two cells deep in front of it, three
/// wide, two high (the Java's `getEntitiesWithinAABB` box, min and max sorted).
pub fn area(core: [i32; 3], facing: [i32; 2]) -> [f64; 6] {
    let side = rot_up(facing);
    let x0 = f64::from(core[0]) + 0.5 - f64::from(side[0]) * 1.5;
    let x1 = f64::from(core[0]) + 0.5 + f64::from(side[0]) * 1.5 + f64::from(facing[0]) * 2.0;
    let z0 = f64::from(core[2]) + 0.5 - f64::from(side[1]) * 1.5;
    let z1 = f64::from(core[2]) + 0.5 + f64::from(side[1]) * 1.5 + f64::from(facing[1]) * 2.0;
    [x0.min(x1), f64::from(core[1]), z0.min(z1), x0.max(x1), f64::from(core[1]) + 2.0, z0.max(z1)]
}

/// Whether a player standing at `pos` (0.6 wide, 1.8 high) meets `area` (`AxisAlignedBB.intersectsWith`: strictly).
pub fn in_area(pos: [f64; 3], a: [f64; 6]) -> bool {
    let p = [pos[0] - 0.3, pos[1], pos[2] - 0.3, pos[0] + 0.3, pos[1] + 1.8, pos[2] + 0.3];
    (0..3).all(|i| p[i + 3] > a[i] && p[i] < a[i + 3])
}

impl crate::machine::Logic for Crane {
    fn tick(&mut self) -> bool {
        let before = (self.pos_front.to_bits(), self.pos_left.to_bits());
        self.step();
        before != (self.pos_front.to_bits(), self.pos_left.to_bits())
    }
    fn level(&self, _: u8) -> u8 {
        0
    }
    fn view(&self) -> crate::view::ViewModel {
        crate::view::ViewModel::new("RBMK crane console")
    }
}

impl Slotted for Crane {
    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
    fn screen_data(&self) -> Vec<i32> {
        self.model_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One pass moves each item one place down into a gap, ascending: an item lands where the one before it left.
    #[test]
    fn the_storage_closes_its_gaps_one_place_a_pass() {
        let mut s = [None, Some(1), None, Some(2), Some(3), None, None, None, None, None, None, Some(4)];
        storage_pass(&mut s);
        // 1 into 0; 2 into 2, and then 3 into 3 (each successor into the gap the step before left); 4 into 10.
        assert_eq!(s, [Some(1), None, Some(2), Some(3), None, None, None, None, None, None, Some(4), None]);
        storage_pass(&mut s);
        assert_eq!(s, [Some(1), Some(2), Some(3), None, None, None, None, None, None, Some(4), None, None]);
        let mut full = [Some(1); 12];
        storage_pass(&mut full);
        assert_eq!(full, [Some(1); 12]);
        storage_pass::<i32>(&mut []);
    }

    /// A channel loads into and unloads from its slot; the storage loads at 11 and unloads from 0; other columns are not
    /// loadable.
    #[test]
    fn what_the_crane_may_load_and_unload() {
        let empty: Vec<Option<i32>> = vec![None];
        assert!(can_load("ntm:rbmk_rod_reasim_mod", &empty) && !can_unload("ntm:rbmk_rod", &empty));
        assert!(!can_load("ntm:rbmk_rod", &[Some(1)]) && can_unload("ntm:rbmk_rod", &[Some(1)]));
        let mut st: Vec<Option<i32>> = vec![None; 12];
        st[0] = Some(1);
        assert!(can_load(STORAGE, &st) && can_unload(STORAGE, &st));
        st[11] = Some(2);
        assert!(!can_load(STORAGE, &st));
        st[0] = None;
        assert!(!can_unload(STORAGE, &st), "only slot 0 gives out");
        assert!(!can_load("ntm:rbmk_blank", &empty) && !can_unload("ntm:rbmk_boiler", &[Some(1)]));
        assert!(!can_load::<i32>("ntm:rbmk_rod", &[]), "no slot: nothing");
    }

    fn linked() -> Crane {
        Crane { set_up: true, center: [10, 80, 10], span_f: 3, span_b: 2, span_l: 1, span_r: 4, height: 7, ..Crane::default() }
    }

    /// The hook: 26 steps down by 0.04 (the 26th stops a hair under 0, the Java's doubles), the next at 0 arrives; then
    /// up again to 1; no key moves the crane while it is not all the way up.
    #[test]
    fn the_hook_goes_down_arrives_once_and_comes_up() {
        let mut c = linked();
        c.input = Some(Keys { load: true, ..Keys::default() });
        c.step();
        assert!(c.goes_down && !c.loading(), "the key sets it going; it moves from the next step");
        c.input = Some(Keys { up: true, load: true, ..Keys::default() });
        let mut arrived = Vec::new();
        for i in 1..=60 {
            c.step();
            if c.arrived {
                arrived.push(i);
            }
            assert_eq!(c.pos_front, 0.0, "no travel while loading");
            if c.arrived {
                // The key still held: the next step up starts a new descent only once it is all the way up.
                c.input = None;
            }
        }
        let mut probe = linked();
        probe.goes_down = true;
        let mut n = 0;
        while probe.progress > 0.0 {
            probe.step();
            n += 1;
        }
        assert_eq!(arrived, vec![n + 1], "it arrives once, the step after it reached 0 or less");
        assert_eq!(c.progress, 1.0, "back up");
        assert!(!c.goes_down);
    }

    /// Keys: a key against its opposite does nothing; 0.05 a step; held in the spans (front 3, back 2, left 1, right 4);
    /// nobody at the console: nothing moves.
    #[test]
    fn keys_move_the_crane_within_its_spans() {
        let mut c = linked();
        c.input = Some(Keys { up: true, down: true, left: true, ..Keys::default() });
        c.step();
        assert_eq!((c.pos_front, c.pos_left, c.tilt), (0.0, 0.05, [0.0, 30.0]));
        c.input = Some(Keys { up: true, right: true, ..Keys::default() });
        for _ in 0..200 {
            c.step();
        }
        assert_eq!((c.pos_front, c.pos_left, c.tilt), (3.0, -4.0, [30.0, -30.0]));
        c.input = Some(Keys { down: true, left: true, ..Keys::default() });
        for _ in 0..200 {
            c.step();
        }
        assert_eq!((c.pos_front, c.pos_left), (-2.0, 1.0));
        c.input = None;
        c.step();
        assert_eq!((c.pos_front, c.pos_left, c.tilt), (-2.0, 1.0, [0.0; 2]));
        // Not set up: the spans are 0, it stays at the centre.
        let mut free = Crane { input: Some(Keys { up: true, ..Keys::default() }), ..Crane::default() };
        free.step();
        assert_eq!(free.pos_front, 0.0);
    }

    /// The cell under the hook for the four facings of the console, as `getColumnAtPos` computes it:
    /// `floor(centre - dir·posFront - left·posLeft + 0.5)`, `left` the facing's `getRotation(DOWN)`.
    #[test]
    fn the_hook_is_over_the_column_rounded() {
        let mut c = linked();
        (c.pos_front, c.pos_left) = (1.49, 0.5);
        // North (0, -1), left (-1, 0): x 10 + 0.5 + 0.5 = 11, z 10 + 1.49 + 0.5 = 11.99.
        assert_eq!(c.column_cell([0, -1]), [11, 79, 11]);
        (c.pos_front, c.pos_left) = (2.0, 1.0);
        // East (1, 0), left (0, -1): x 10 - 2 + 0.5, z 10 + 1 + 0.5.
        assert_eq!(c.column_cell([1, 0]), [8, 79, 11]);
        // South (0, 1), left (1, 0): x 10 - 1 + 0.5, z 10 - 2 + 0.5. West (-1, 0), left (0, 1): x 10 + 2 + 0.5, z 10 - 1 + 0.5.
        assert_eq!(c.column_cell([0, 1]), [9, 79, 8]);
        assert_eq!(c.column_cell([-1, 0]), [12, 79, 9]);
        // Halves go up: 10 - 0.5 + 0.5 = 10.
        (c.pos_front, c.pos_left) = (0.5, 0.0);
        assert_eq!(c.column_cell([0, 1])[2], 10);
    }

    /// `setTarget`: the centre a cell over the column's top; each span the free cells (up to 16) at `centerY + 6`, a cell
    /// of an unknown chunk stopping it; forwards is away from the console.
    #[test]
    fn a_link_measures_the_room() {
        let mut c = Crane::default();
        // The console faces north: forwards is south (+z). A wall 5 south, unknown 3 east, nothing west or north.
        c.link([0, 64, 0], 4, [0, -1], |p| {
            assert_eq!(p[1], 64 + 4 + 1 + 6);
            if p[2] == 5 && p[0] == 0 {
                Some(false)
            } else if p[0] == 3 && p[2] == 0 {
                None
            } else {
                Some(true)
            }
        });
        assert_eq!(c.center, [0, 69, 0]);
        // Right of south is west; back is north; left is east.
        assert_eq!((c.span_f, c.span_r, c.span_b, c.span_l, c.height, c.set_up), (4, 16, 16, 2, 7, true));
    }

    /// The console's area: in front of it (towards its facing) two deep, three wide, two high; a player touching it is in.
    #[test]
    fn the_console_area_is_in_front_of_it() {
        let a = area([0, 64, 0], [0, -1]);
        assert_eq!(a, [-1.0, 64.0, -1.5, 2.0, 66.0, 0.5]);
        assert!(in_area([0.5, 64.0, -1.0], a));
        assert!(in_area([2.2, 64.0, 0.0], a), "its edge at 2.0, the player's at 1.9");
        assert!(!in_area([2.4, 64.0, 0.0], a), "past its edge");
        assert!(!in_area([-1.3, 64.0, 0.0], a), "edges touching are not in (strictly): -1.0 against -1.0");
        assert!(!in_area([0.5, 64.0, 1.0], a), "behind the console");
        assert!(!in_area([0.5, 66.0, -1.0], a), "above it");
    }

    /// The model's values and the bridge's turn.
    #[test]
    fn the_console_shows_its_state() {
        let mut c = linked();
        assert_eq!(c.model_data(), vec![0, 0, 0, 0, 0, 0]);
        c.slots = vec![Some(Stack::new("ntm:rbmk_fuel_ueu", 1))];
        (c.meters, c.valid_target) = ([0.25, 0.5], true);
        assert_eq!(c.model_data(), vec![0, 0, 250, 500, 2, 1]);
        c.progress = 0.5;
        assert_eq!(c.model_data()[4], 1);
        for _ in 0..5 {
            c.rotate();
        }
        assert_eq!(c.rotation_offset, 90);
    }
}
