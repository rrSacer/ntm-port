//! Doors, hatches, seals and locks of the K3 set: the pure logic (state,
//! timing, PIN pairing, redstone, multiblock footprint, dimensions, offsets).
//!
//! The Java this ports, under `ntm-src/src/main/java/com/hbm/`:
//! - `tileentity/DoorDecl.java`: the 14 door types (`DoorSpec` here). Only
//!   what drives the game: opening time, moving ranges, dimensions, extra
//!   boxes, offset, remote control, redstone. Sound, animation and skins are
//!   client-side and not ported.
//! - `blocks/generic/BlockDoorGeneric.java` and
//!   `tileentity/TileEntityDoorGeneric.java`: `Door`, its tick and redstone.
//! - `blocks/machine/BlastDoor.java` and `tileentity/machine/TileEntityBlastDoor.java`:
//!   `BlastDoorGroup` (the 6-block-tall blast door and its neighbours, which
//!   open and close together).
//! - `blocks/machine/BlockSeal.java` and `BlockHatch.java` (`seal_controller`,
//!   `seal_hatch`): the square-frame seal.
//! - `tileentity/machine/TileEntityLockableBase.java`, `items/tool/ItemKeyPin.java`,
//!   `ItemLock.java`, `ItemCounterfeitKeys.java`: `Lock`, pairing, counterfeit keys.
//! - `blocks/generic/BlockKeyhole.java`: the keyhole for `key_red`.
//!
//! Pure calculation only: the registry, the server tick, save and menus are
//! wired later. Coordinates are relative to the door's core block.

use std::collections::BTreeMap;

/// A block position, relative to the door's core (or the controller).
pub type Cell = (i32, i32, i32);

/// The horizontal `ForgeDirection` a door or seal faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    North,
    South,
    East,
    West,
}

/// `util/fauxpointtwelve/Rotation`: quarter turns, clockwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    None,
    Cw90,
    Cw180,
    Ccw90,
}

impl Turn {
    fn quarter(self) -> u8 {
        match self {
            Turn::None => 0,
            Turn::Cw90 => 1,
            Turn::Cw180 => 2,
            Turn::Ccw90 => 3,
        }
    }

    fn from_quarter(q: u8) -> Turn {
        match q % 4 {
            0 => Turn::None,
            1 => Turn::Cw90,
            2 => Turn::Cw180,
            _ => Turn::Ccw90,
        }
    }

    /// `Rotation.add`: the turns compose by adding the quarters.
    pub fn compose(self, other: Turn) -> Turn {
        Turn::from_quarter(self.quarter() + other.quarter())
    }
}

/// `Rotation.getBlockRotation(dir)`.
pub fn block_rotation(facing: Facing) -> Turn {
    match facing {
        Facing::North => Turn::None,
        Facing::South => Turn::Cw180,
        Facing::East => Turn::Ccw90,
        Facing::West => Turn::Cw90,
    }
}

/// `BlockPos.rotate(Rotation)`.
pub fn rotate_pos(p: Cell, turn: Turn) -> Cell {
    let (x, y, z) = p;
    match turn {
        Turn::None => (x, y, z),
        Turn::Cw90 => (-z, y, x),
        Turn::Cw180 => (-x, y, -z),
        Turn::Ccw90 => (z, y, -x),
    }
}

/// `MultiblockHandlerXR.rotate(dim, dir)`. A dimension array is
/// `[up, down, north, south, west, east]`, in the frame of a block facing south
/// (the identity). Each facing swaps the slots as the Java table does.
pub fn rotate_dims(dims: [i32; 6], facing: Facing) -> [i32; 6] {
    let d = dims;
    match facing {
        Facing::South => d,
        Facing::North => [d[0], d[1], d[3], d[2], d[5], d[4]],
        Facing::East => [d[0], d[1], d[5], d[4], d[2], d[3]],
        Facing::West => [d[0], d[1], d[4], d[5], d[3], d[2]],
    }
}

/// `MultiblockHandlerXR.checkSpace` / `fillSpace`: every cell of the box that
/// the dimensions describe, from `origin` (the position already offset along
/// the facing). A side with a negative extent makes its range empty or
/// shifted, as the Java loops do.
pub fn footprint(origin: Cell, dims: [i32; 6], facing: Facing) -> Vec<Cell> {
    let d = rotate_dims(dims, facing);
    let (x, y, z) = origin;
    let mut out = Vec::new();
    for a in (x - d[4])..=(x + d[5]) {
        for b in (y - d[1])..=(y + d[0]) {
            for c in (z - d[2])..=(z + d[3]) {
                out.push((a, b, c));
            }
        }
    }
    out
}

/// `DoorDecl.getNormTime(time, min, max)` (`BobMathUtil.remap01_clamp`, which
/// is `MathHelper.clamp_float`). `f32::clamp` keeps NaN, as Java's clamp does.
fn norm_time(ticks: i32, min: f32, max: f32) -> f32 {
    ((ticks as f32 - min) / (max - min)).clamp(0.0, 1.0)
}

/// One of the 14 `DoorDecl` types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorKind {
    TransitionSeal,
    VaultDoor,
    FireDoor,
    SlidingBlastDoor,
    SlidingSealDoor,
    SecureAccessDoor,
    RoundAirlockDoor,
    QeSlidingDoor,
    QeContainment,
    WaterDoor,
    SiloHatch,
    SiloHatchLarge,
    LargeVehicleDoor,
    CargoDoor,
}

/// The numbers of one `DoorDecl`.
///
/// `open_ranges` rows are `[x, y, z, len, width, axis]`: a range starts at
/// `(x, y, z)` and takes `|len|` steps. Each step covers `width` cells across
/// the axis: axis 0 steps along z and spans y, axis 1 steps along y and spans
/// x, axis 2 steps along x and spans y (the Java `switch` on `range[5]`). The
/// walk is then rotated by the facing.
#[derive(Debug, PartialEq)]
pub struct DoorSpec {
    pub kind: DoorKind,
    /// The registry id (`ntm:...`).
    pub id: &'static str,
    /// `timeToOpen()`, in ticks.
    pub time_to_open: u32,
    pub open_ranges: &'static [[i32; 6]],
    /// `getDimensions()`: the box the closed door takes.
    pub dimensions: [i32; 6],
    /// `getExtraDimensions()`: more boxes the door also takes (only the vault).
    pub extra_dimensions: &'static [[i32; 6]],
    /// `getBlockOffset()`: the `BlockDummyable` offset of the core.
    pub block_offset: i32,
    /// `remoteControllable()`: a bomb or a logic block can toggle it.
    pub remote_controllable: bool,
    /// `neverDemandRedstone()`: a player can use it in ultra LARP mode.
    pub never_demand_redstone: bool,
}

pub const TRANSITION_SEAL: DoorSpec = DoorSpec {
    kind: DoorKind::TransitionSeal,
    id: "ntm:transition_seal",
    time_to_open: 480,
    open_ranges: &[[-9, 2, 0, 20, 20, 1]],
    dimensions: [23, 0, 0, 0, 13, 12],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

pub const VAULT_DOOR: DoorSpec = DoorSpec {
    kind: DoorKind::VaultDoor,
    id: "ntm:vault_door",
    time_to_open: 120,
    open_ranges: &[[-1, 1, 0, 3, 3, 2]],
    dimensions: [4, 0, 0, 0, 2, 2],
    extra_dimensions: &[[0, 0, 1, -1, 2, 2]],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

pub const FIRE_DOOR: DoorSpec = DoorSpec {
    kind: DoorKind::FireDoor,
    id: "ntm:fire_door",
    time_to_open: 160,
    open_ranges: &[[-1, 0, 0, 3, 4, 1]],
    dimensions: [2, 0, 0, 0, 2, 1],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

pub const SLIDE_DOOR: DoorSpec = DoorSpec {
    kind: DoorKind::SlidingBlastDoor,
    id: "ntm:sliding_blast_door",
    time_to_open: 24,
    open_ranges: &[[-2, 0, 0, 4, 5, 1]],
    dimensions: [3, 0, 0, 0, 3, 3],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

pub const SLIDING_SEAL_DOOR: DoorSpec = DoorSpec {
    kind: DoorKind::SlidingSealDoor,
    id: "ntm:sliding_seal_door",
    time_to_open: 20,
    open_ranges: &[[0, 0, 0, 1, 2, 2]],
    dimensions: [1, 0, 0, 0, 0, 0],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

pub const SECURE_ACCESS_DOOR: DoorSpec = DoorSpec {
    kind: DoorKind::SecureAccessDoor,
    id: "ntm:secure_access_door",
    time_to_open: 120,
    open_ranges: &[[-2, 1, 0, 4, 5, 1]],
    dimensions: [4, 0, 0, 0, 2, 2],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

pub const ROUND_AIRLOCK_DOOR: DoorSpec = DoorSpec {
    kind: DoorKind::RoundAirlockDoor,
    id: "ntm:round_airlock_door",
    time_to_open: 60,
    open_ranges: &[[0, 0, 0, -2, 4, 2], [0, 0, 0, 3, 4, 2]],
    dimensions: [3, 0, 0, 0, 2, 1],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

pub const QE_SLIDING_DOOR: DoorSpec = DoorSpec {
    kind: DoorKind::QeSlidingDoor,
    id: "ntm:qe_sliding_door",
    time_to_open: 10,
    open_ranges: &[[0, 0, 0, 2, 2, 2]],
    dimensions: [1, 0, 0, 0, 1, 0],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

pub const QE_CONTAINMENT: DoorSpec = DoorSpec {
    kind: DoorKind::QeContainment,
    id: "ntm:qe_containment",
    time_to_open: 160,
    open_ranges: &[[-1, 0, 0, 3, 3, 1]],
    dimensions: [2, 0, 0, 0, 1, 1],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

pub const WATER_DOOR: DoorSpec = DoorSpec {
    kind: DoorKind::WaterDoor,
    id: "ntm:water_door",
    time_to_open: 60,
    open_ranges: &[[1, 0, 0, -3, 3, 2]],
    dimensions: [2, 0, 0, 0, 1, 1],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: true,
};

pub const SILO_HATCH: DoorSpec = DoorSpec {
    kind: DoorKind::SiloHatch,
    id: "ntm:silo_hatch",
    time_to_open: 60,
    open_ranges: &[
        [1, 0, 1, -3, 3, 0],
        [0, 0, 1, -3, 3, 0],
        [-1, 0, 1, -3, 3, 0],
    ],
    dimensions: [0, 0, 2, 2, 2, 2],
    extra_dimensions: &[],
    block_offset: 2,
    remote_controllable: true,
    never_demand_redstone: false,
};

pub const SILO_HATCH_LARGE: DoorSpec = DoorSpec {
    kind: DoorKind::SiloHatchLarge,
    id: "ntm:silo_hatch_large",
    time_to_open: 60,
    open_ranges: &[
        [2, 0, 1, -3, 3, 0],
        [1, 0, 2, -5, 3, 0],
        [0, 0, 2, -5, 3, 0],
        [-1, 0, 2, -5, 3, 0],
        [-2, 0, 1, -3, 3, 0],
    ],
    dimensions: [0, 0, 3, 3, 3, 3],
    extra_dimensions: &[],
    block_offset: 3,
    remote_controllable: true,
    never_demand_redstone: false,
};

pub const LARGE_VEHICLE_DOOR: DoorSpec = DoorSpec {
    kind: DoorKind::LargeVehicleDoor,
    id: "ntm:large_vehicle_door",
    time_to_open: 60,
    open_ranges: &[[0, 0, 0, -4, 6, 2], [0, 0, 0, 4, 6, 2]],
    dimensions: [5, 0, 0, 0, 3, 3],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

pub const CARGO_DOOR: DoorSpec = DoorSpec {
    kind: DoorKind::CargoDoor,
    id: "ntm:cargo_door",
    time_to_open: 60,
    open_ranges: &[[-1, -1, 0, 3, 3, 1]],
    dimensions: [2, 0, 0, 0, 1, 1],
    extra_dimensions: &[],
    block_offset: 0,
    remote_controllable: false,
    never_demand_redstone: false,
};

/// The `DoorDecl` of a kind.
pub fn spec(kind: DoorKind) -> &'static DoorSpec {
    match kind {
        DoorKind::TransitionSeal => &TRANSITION_SEAL,
        DoorKind::VaultDoor => &VAULT_DOOR,
        DoorKind::FireDoor => &FIRE_DOOR,
        DoorKind::SlidingBlastDoor => &SLIDE_DOOR,
        DoorKind::SlidingSealDoor => &SLIDING_SEAL_DOOR,
        DoorKind::SecureAccessDoor => &SECURE_ACCESS_DOOR,
        DoorKind::RoundAirlockDoor => &ROUND_AIRLOCK_DOOR,
        DoorKind::QeSlidingDoor => &QE_SLIDING_DOOR,
        DoorKind::QeContainment => &QE_CONTAINMENT,
        DoorKind::WaterDoor => &WATER_DOOR,
        DoorKind::SiloHatch => &SILO_HATCH,
        DoorKind::SiloHatchLarge => &SILO_HATCH_LARGE,
        DoorKind::LargeVehicleDoor => &LARGE_VEHICLE_DOOR,
        DoorKind::CargoDoor => &CARGO_DOOR,
    }
}

/// The time fraction (0..=1) a door's range `ticks` in: `getDoorRangeOpenTime`.
/// Water doors and silo hatches have their own windows; the rest use the whole
/// `timeToOpen`.
pub fn range_time(spec: &DoorSpec, ticks: i32) -> f32 {
    match spec.kind {
        DoorKind::WaterDoor => norm_time(ticks, 35.0, 40.0),
        DoorKind::SiloHatch | DoorKind::SiloHatchLarge => norm_time(ticks, 20.0, 20.0),
        _ => norm_time(ticks, 0.0, spec.time_to_open as f32),
    }
}

/// The cells the moving part passes at `ticks`, as `TileEntityDoorGeneric.updateEntity`
/// walks them: when opening, the cells up to the time fraction, from the start
/// of each range; when closing, the same from the far end back. The core is
/// left out (`shouldUseBB` in Java).
///
/// The fraction is `j / |len - 1|` (Java's `Math.abs(range[3] - 1)`, not
/// `|len| - 1`). When `len == 1` that is `0/0`: NaN, so no step breaks. The
/// silo windows are `20..20`, so exactly at tick 20 the fraction is NaN and the
/// whole hatch opens at once. That is the Java behaviour, kept on purpose.
pub fn moving_cells(spec: &DoorSpec, facing: Facing, ticks: i32, opening: bool) -> Vec<Cell> {
    let mut turn = block_rotation(facing);
    if matches!(facing, Facing::East | Facing::West) {
        turn = turn.compose(Turn::Cw180);
    }
    let time = range_time(spec, ticks);
    let mut out = Vec::new();
    for range in spec.open_ranges {
        let [sx, sy, sz, len, width, axis] = *range;
        let steps = len.abs();
        let denom = (len - 1).abs() as f32;
        let sign = len.signum();
        let order: Vec<i32> = if opening {
            (0..steps).collect()
        } else {
            (0..steps).rev().collect()
        };
        for j in order {
            let frac = j as f32 / denom;
            if opening && frac > time {
                break;
            }
            if !opening && frac < time {
                break;
            }
            for k in 0..width {
                let add = match axis {
                    0 => (0, k, sign * j),
                    1 => (k, sign * j, 0),
                    2 => (sign * j, k, 0),
                    _ => (0, 0, 0),
                };
                let p = rotate_pos((sx + add.0, sy + add.1, sz + add.2), turn);
                if p != (0, 0, 0) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// `Lock` (`TileEntityLockableBase`): the pins of the lock, whether it is
/// locked, whether a counterfeit key can be cut from it, and the lock-pick
/// modifier (`lockMod`, 0.1 by default).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lock {
    pub pins: i32,
    pub locked: bool,
    pub cheesable: bool,
    pub modifier: f64,
}

impl Default for Lock {
    fn default() -> Self {
        Lock {
            pins: 0,
            locked: false,
            cheesable: true,
            modifier: 0.1,
        }
    }
}

/// What the player's hand gives to `canAccess`. `None` where the caller has no
/// player (a bomb, a redstone pulse).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Holding {
    /// An empty hand or an item that opens nothing.
    Nothing,
    /// An `ItemKey` with these pins.
    Key(i32),
    /// `key_red`: opens every lock.
    RedKey,
    /// A lock-pick try. The caller checked that the player has a pin and a
    /// screwdriver. `roll` is in 0..100 (`nextDouble() * 100`).
    Pick { jacket: bool, roll: f64 },
}

/// The chance, in percent, that a lock-pick try works: `lockMod * 100`, and
/// times 100 with the jacket (`ArmorUtil.checkArmorPiece(jackt, 2)`).
pub fn pick_chance(modifier: f64, jacket: bool) -> f64 {
    let base = modifier * 100.0;
    if jacket { base * 100.0 } else { base }
}

impl Lock {
    /// `canAccess`: an unlocked lock opens; a locked one needs a matching key,
    /// `key_red`, or a successful pick. With no player, a locked one stays shut.
    pub fn can_access(&self, holding: Option<Holding>) -> bool {
        if !self.locked {
            return true;
        }
        match holding {
            None | Some(Holding::Nothing) => false,
            Some(Holding::Key(pins)) => pins == self.pins,
            Some(Holding::RedKey) => true,
            Some(Holding::Pick { jacket, roll }) => pick_chance(self.modifier, jacket) > roll,
        }
    }

    /// `ItemLock.onItemUse` on a lock: a padlock with pins locks an unlocked
    /// one. Returns whether the padlock is used up.
    pub fn apply_padlock(&mut self, pins: i32, modifier: f64) -> bool {
        if pins == 0 || self.locked {
            return false;
        }
        self.pins = pins;
        self.locked = true;
        self.modifier = modifier;
        true
    }
}

/// Why `ItemCounterfeitKeys` refuses to cut keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterfeitRefusal {
    /// The lock is not `cheesable`: "too elaborate".
    TooElaborate,
    /// Not locked: nothing to copy.
    NotLocked,
}

/// `ItemCounterfeitKeys`: the kit becomes two `key_fake`, both with the lock's
/// pins. Returns the pins of those two keys.
pub fn counterfeit_keys(lock: &Lock) -> Result<[i32; 2], CounterfeitRefusal> {
    if !lock.cheesable {
        return Err(CounterfeitRefusal::TooElaborate);
    }
    if !lock.locked {
        return Err(CounterfeitRefusal::NotLocked);
    }
    Ok([lock.pins, lock.pins])
}

/// `DoorDecl`-driven door: `BlockDoorGeneric` + `TileEntityDoorGeneric`.
#[derive(Clone, Debug, PartialEq)]
pub struct Door {
    pub spec: &'static DoorSpec,
    pub lock: Lock,
    pub state: DoorState,
    /// `openTicks`: 0 closed, `timeToOpen` open.
    pub open_ticks: i32,
    /// `redstonePower`: the powered sources. `-1` means every source went off
    /// and the next tick clears it. Opening is 0.
    pub redstone: i32,
    /// The cells (`activatedBlocks`) that feed the redstone.
    pub activated: std::collections::BTreeSet<Cell>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorState {
    Closed,
    Opening,
    Open,
    Closing,
}

/// The moving cells of one tick: exposed when opening, taken back when closing.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub opening: bool,
    pub cells: Vec<Cell>,
}

impl Door {
    pub fn new(spec: &'static DoorSpec) -> Door {
        Door {
            spec,
            lock: Lock::default(),
            state: DoorState::Closed,
            open_ticks: 0,
            redstone: 0,
            activated: Default::default(),
        }
    }

    /// `updateEntity`, one tick. Moves the stroke, reports the cells it passes,
    /// then applies the redstone rule (a powered closed door opens, an
    /// unpowered open one closes; a locked door refuses this, as Java does).
    pub fn tick(&mut self, facing: Facing) -> Option<Step> {
        let time = self.spec.time_to_open as i32;
        let step = match self.state {
            DoorState::Opening => {
                self.open_ticks = (self.open_ticks + 1).min(time);
                Some(Step {
                    opening: true,
                    cells: moving_cells(self.spec, facing, self.open_ticks, true),
                })
            }
            DoorState::Closing => {
                self.open_ticks = (self.open_ticks - 1).max(0);
                Some(Step {
                    opening: false,
                    cells: moving_cells(self.spec, facing, self.open_ticks, false),
                })
            }
            _ => None,
        };
        if self.state == DoorState::Opening && self.open_ticks == time {
            self.state = DoorState::Open;
        }
        if self.state == DoorState::Closing && self.open_ticks == 0 {
            self.state = DoorState::Closed;
        }
        if (self.redstone == -1 && self.state == DoorState::Open)
            || (self.redstone > 0 && self.state == DoorState::Closed)
        {
            self.toggle_passcode(-1);
        }
        if self.redstone == -1 {
            self.redstone = 0;
        }
        step
    }

    /// `tryToggle(player)`. Returns `true` even when the lock refused, as Java
    /// does: the click is used up either way. A closed door with redstone
    /// power does not move.
    pub fn try_toggle(&mut self, holding: Option<Holding>) -> bool {
        if self.lock.locked && holding.is_none() {
            return false;
        }
        if self.state == DoorState::Closed && self.redstone > 0 {
            return false;
        }
        match self.state {
            DoorState::Closed => {
                if self.lock.can_access(holding) {
                    self.state = DoorState::Opening;
                }
                true
            }
            DoorState::Open => {
                if self.lock.can_access(holding) {
                    self.state = DoorState::Closing;
                }
                true
            }
            _ => false,
        }
    }

    /// `tryToggle(passcode)`, used by redstone and logic blocks: a locked door
    /// moves only with its own pins.
    pub fn toggle_passcode(&mut self, passcode: i32) -> bool {
        if self.lock.locked && passcode != self.lock.pins {
            return false;
        }
        match self.state {
            DoorState::Closed => {
                self.state = DoorState::Opening;
                true
            }
            DoorState::Open => {
                self.state = DoorState::Closing;
                true
            }
            _ => false,
        }
    }

    /// `BlockDoorGeneric.onBlockActivated`: in ultra LARP mode only doors that
    /// never need redstone answer a player.
    pub fn player_can_click(&self, ultra_larp: bool) -> bool {
        !ultra_larp || self.spec.never_demand_redstone
    }

    /// `BlockDoorGeneric.explode`: a bomb toggles only the remote-controllable kinds.
    pub fn bomb_trigger(&mut self) -> bool {
        self.spec.remote_controllable && self.try_toggle(None)
    }

    /// `updateRedstonePower`: one neighbour cell (`pos`) went on or off.
    pub fn update_redstone(&mut self, pos: Cell, powered: bool) {
        let contained = self.activated.contains(&pos);
        if !contained && powered {
            self.activated.insert(pos);
            if self.redstone == -1 {
                self.redstone = 0;
            }
            self.redstone += 1;
        } else if contained && !powered {
            self.activated.remove(&pos);
            self.redstone -= 1;
            if self.redstone == 0 {
                self.redstone = -1;
            }
        }
    }

    /// `open()` / `close()`: the logic-block calls, which skip the lock.
    pub fn logic_open(&mut self) {
        if self.state == DoorState::Closed {
            self.state = DoorState::Opening;
        }
    }

    pub fn logic_close(&mut self) {
        if self.state == DoorState::Open {
            self.state = DoorState::Closing;
        }
    }
}

/// One step of a blast door's moving frame: a block placed or removed `dy`
/// blocks above the controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dummy {
    pub place: bool,
    pub dy: i32,
}

/// `TileEntityBlastDoor.state`: 0 closed, 1 moving, 2 open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlastState {
    Closed,
    Moving,
    Open,
}

/// One blast door (`BlastDoor` + `TileEntityBlastDoor`).
#[derive(Clone, Debug, PartialEq)]
pub struct BlastDoor {
    pub lock: Lock,
    pub state: BlastState,
    /// `isOpening`: which way it is moving while `Moving`.
    pub opening: bool,
    pub timer: i32,
    pub redstoned: bool,
}

const NEIGHBOURS: [Cell; 4] = [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)];

/// The blast doors of one wall. They open and close together, and a lock
/// spreads to the neighbours.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlastDoorGroup {
    pub doors: BTreeMap<Cell, BlastDoor>,
}

/// `TileEntityBlastDoor.updateEntity` moves in these steps (`timer` ≥ value).
const OPEN_STEPS: [(i32, i32); 5] = [(0, 1), (20, 2), (40, 3), (60, 4), (80, 5)];
const CLOSE_STEPS: [(i32, i32); 5] = [(20, 5), (40, 4), (60, 3), (80, 2), (100, 1)];

impl BlastDoorGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, pos: Cell) {
        self.doors.insert(
            pos,
            BlastDoor {
                lock: Lock::default(),
                state: BlastState::Closed,
                opening: false,
                timer: 0,
                redstoned: false,
            },
        );
    }

    fn offset(pos: Cell, d: Cell) -> Cell {
        (pos.0 + d.0, pos.1 + d.1, pos.2 + d.2)
    }

    fn start_open(&mut self, pos: Cell) {
        if let Some(d) = self.doors.get_mut(&pos) {
            d.opening = true;
            d.state = BlastState::Moving;
        }
    }

    fn start_close(&mut self, pos: Cell) {
        if let Some(d) = self.doors.get_mut(&pos) {
            d.opening = false;
            d.state = BlastState::Moving;
        }
    }

    /// `openNeigh`: opens the unlocked (or same-lock) neighbours, and theirs.
    fn open_neighbours(&mut self, pos: Cell) {
        let pins = self.doors[&pos].lock.pins;
        for d in NEIGHBOURS {
            let n = Self::offset(pos, d);
            let ok = self.doors.get(&n).is_some_and(|t| {
                t.state == BlastState::Closed && (!t.lock.locked || t.lock.pins == pins)
            });
            if ok {
                self.start_open(n);
                self.open_neighbours(n);
            }
        }
    }

    /// `closeNeigh`: the same, for closing (state Open).
    fn close_neighbours(&mut self, pos: Cell) {
        let pins = self.doors[&pos].lock.pins;
        for d in NEIGHBOURS {
            let n = Self::offset(pos, d);
            let ok = self.doors.get(&n).is_some_and(|t| {
                t.state == BlastState::Open && (!t.lock.locked || t.lock.pins == pins)
            });
            if ok {
                self.start_close(n);
                self.close_neighbours(n);
            }
        }
    }

    /// `tryToggle`: opens a closed door (and its neighbours) or closes an open one.
    pub fn toggle(&mut self, pos: Cell) {
        let Some(d) = self.doors.get(&pos) else {
            return;
        };
        match d.state {
            BlastState::Closed => {
                self.start_open(pos);
                self.open_neighbours(pos);
            }
            BlastState::Open => {
                self.start_close(pos);
                self.close_neighbours(pos);
            }
            BlastState::Moving => {}
        }
    }

    /// A click (`onBlockActivated`). A lock tool in hand does nothing here (the
    /// item takes it). Returns whether the click was used.
    pub fn click(&mut self, pos: Cell, holding: Holding, lock_tool: bool, sneaking: bool) -> bool {
        if lock_tool || sneaking {
            return false;
        }
        let Some(d) = self.doors.get(&pos) else {
            return true;
        };
        if d.lock.locked {
            if d.lock.can_access(Some(holding)) {
                self.toggle(pos);
            }
        } else {
            self.toggle(pos);
        }
        true
    }

    /// `TileEntityBlastDoor.updateEntity`, one tick. `powered_base` is the
    /// redstone at the controller, `powered_top` the one six blocks up. The
    /// base sensor is ignored when locked, but the top one is not: a Java
    /// precedence rule (`!locked && base || top`), kept.
    pub fn tick(&mut self, pos: Cell, powered_base: bool, powered_top: bool) -> Vec<Dummy> {
        let Some(d) = self.doors.get(&pos) else {
            return Vec::new();
        };
        let signal = (!d.lock.locked && powered_base) || powered_top;
        let was = d.redstoned;
        if signal && !was {
            self.toggle(pos);
        }
        if let Some(d) = self.doors.get_mut(&pos) {
            d.redstoned = signal;
        }
        let Some(d) = self.doors.get_mut(&pos) else {
            return Vec::new();
        };
        if d.state != BlastState::Moving {
            d.timer = 0;
            return Vec::new();
        }
        d.timer += 1;
        let timer = d.timer;
        let opening = d.opening;
        let steps = if opening { &OPEN_STEPS } else { &CLOSE_STEPS };
        let out: Vec<Dummy> = steps
            .iter()
            .filter(|(t, _)| timer >= *t)
            .map(|&(_, dy)| Dummy {
                place: !opening,
                dy,
            })
            .collect();
        if timer >= 100 {
            let state = if opening {
                BlastState::Open
            } else {
                BlastState::Closed
            };
            self.doors.entry(pos).and_modify(|d| d.state = state);
        }
        out
    }

    /// `ItemLock` on a blast door, and `lock()` → `lockNeigh`. The neighbours get
    /// the pins at once but the door's *old* modifier (Java sets `setMod` after
    /// `lock()`). Only the door itself takes `modifier`.
    pub fn apply_padlock(&mut self, pos: Cell, pins: i32, modifier: f64) -> bool {
        let Some(d) = self.doors.get_mut(&pos) else {
            return false;
        };
        if pins == 0 || d.lock.locked {
            return false;
        }
        d.lock.pins = pins;
        d.lock.locked = true;
        self.lock_neighbours(pos);
        if let Some(d) = self.doors.get_mut(&pos) {
            d.lock.modifier = modifier;
        }
        true
    }

    fn lock_neighbours(&mut self, pos: Cell) {
        let (pins, modifier) = {
            let d = &self.doors[&pos].lock;
            (d.pins, d.modifier)
        };
        for d in NEIGHBOURS {
            let n = Self::offset(pos, d);
            let Some(t) = self.doors.get_mut(&n) else {
                continue;
            };
            if t.lock.locked {
                continue;
            }
            t.lock.pins = pins;
            t.lock.locked = true;
            self.lock_neighbours(n);
            if let Some(t) = self.doors.get_mut(&n) {
                t.lock.modifier = modifier;
            }
        }
    }
}

/// Seal (`BlockSeal`): the controller's frame, and the hatches in its square.
/// The controller's metadata gives the facing (`meta % 4`); bit 4 is the latched
/// redstone.
pub const SEAL_MAX_SIZE: i32 = 6;

/// The offset of the square from the controller, for a size and a facing.
pub fn seal_offset(meta: u8, size: i32) -> (i32, i32) {
    let (mut xo, mut zo) = (0, 0);
    match meta % 4 {
        2 => zo += size,
        3 => zo -= size,
        0 => xo += size,
        1 => xo -= size,
        _ => {}
    }
    (xo, zo)
}

/// `BlockSeal.getFrameSize`: the smallest size (1..=6) whose ring is all frame
/// (or controller) blocks. Zero when no ring closes. `is_frame` says if a cell
/// is a `seal_frame` or the `seal_controller`.
pub fn frame_size(controller: Cell, meta: u8, is_frame: impl Fn(Cell) -> bool) -> i32 {
    let (x, y, z) = controller;
    for size in 1..=SEAL_MAX_SIZE {
        let (xo, zo) = seal_offset(meta, size);
        let ok = (x - size..=x + size).all(|bx| is_frame((bx + xo, y, z + size + zo)))
            && (x - size..=x + size).all(|bx| is_frame((bx + xo, y, z - size + zo)))
            && (z - size..=z + size).all(|bz| is_frame((x - size + xo, y, bz + zo)))
            && (z - size..=z + size).all(|bz| is_frame((x + size + xo, y, bz + zo)));
        if ok {
            return size;
        }
    }
    0
}

/// The interior square a seal of this size closes.
pub fn seal_interior(controller: Cell, meta: u8, size: i32) -> Vec<Cell> {
    let (x, y, z) = controller;
    let (xo, zo) = seal_offset(meta, size);
    let mut out = Vec::new();
    for bx in x - size + 1..=x + size - 1 {
        for bz in z - size + 1..=z + size - 1 {
            out.push((bx + xo, y, bz + zo));
        }
    }
    out
}

/// `BlockSeal.closeSeal`: the air cells of the interior that become hatches.
pub fn seal_close_cells(
    controller: Cell,
    meta: u8,
    size: i32,
    is_air: impl Fn(Cell) -> bool,
) -> Vec<Cell> {
    seal_interior(controller, meta, size)
        .into_iter()
        .filter(|c| is_air(*c))
        .collect()
}

/// `BlockSeal.openSeal`: the hatches of the interior that become air.
pub fn seal_open_cells(
    controller: Cell,
    meta: u8,
    size: i32,
    is_hatch: impl Fn(Cell) -> bool,
) -> Vec<Cell> {
    seal_interior(controller, meta, size)
        .into_iter()
        .filter(|c| is_hatch(*c))
        .collect()
}

/// `BlockSeal.isSealClosed`: true when any interior cell is a hatch. (The name
/// is Java's and reads backwards: a hatch present means the seal is closed.)
pub fn seal_is_closed(
    controller: Cell,
    meta: u8,
    size: i32,
    is_hatch: impl Fn(Cell) -> bool,
) -> bool {
    seal_interior(controller, meta, size)
        .into_iter()
        .any(is_hatch)
}

/// What a seal's neighbour redstone change does (`BlockSeal.onNeighborBlockChange`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SealRedstone {
    pub meta: u8,
    /// Whether the seal toggles (open if closed, close if open).
    pub toggle: bool,
}

pub fn seal_redstone(meta: u8, powered: bool) -> SealRedstone {
    if powered {
        if meta < 4 {
            return SealRedstone {
                meta: meta + 4,
                toggle: true,
            };
        }
        return SealRedstone {
            meta,
            toggle: false,
        };
    }
    if meta >= 4 {
        return SealRedstone {
            meta: meta % 4,
            toggle: false,
        };
    }
    SealRedstone {
        meta,
        toggle: false,
    }
}

/// `TileEntityHatch.updateEntity`: a hatch stays only while its controller is a
/// `seal_controller` and the seal frame still closes.
pub fn hatch_survives(controller_is_seal: bool, frame: i32) -> bool {
    controller_is_seal && frame != 0
}

/// `BlockKeyhole` / `BlockRedBrickKeyhole` with a red key: the orientation
/// of the door it opens to, for a clicked side (ForgeDirection 0..=5). Up and
/// down do nothing. A cracked key is used up (`cracked`), the normal one is not.
pub fn keyhole_orientation(side: u8) -> Option<u8> {
    match side {
        2 => Some(1),
        5 => Some(2),
        3 => Some(3),
        4 => Some(0),
        _ => None,
    }
}
