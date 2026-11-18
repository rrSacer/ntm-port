//! The wire endpoints: the connectors, the pylons and the large pylon (`ntm:red_connector`,
//! `ntm:red_connector_super`, `ntm:red_pylon`, `ntm:red_pylon_steel`, `ntm:red_pylon_medium_*`,
//! `ntm:red_pylon_large`): `TileEntityPylonBase.java`, `TileEntityConnector.java`,
//! `TileEntityConnectorSuper.java`, `TileEntityPylon.java`, `TileEntityPylonMedium.java`,
//! `TileEntityPylonLarge.java` and the wire item `ItemWiring.java`.
//!
//! An endpoint is a node of the power net open on a few faces (what `createNode` lists) and, besides
//! them, linked by wire to other endpoints of the same connection type within the shorter range
//! (`canConnect`). The links are kept in the endpoint (`connected`) and in the other one: a link
//! counts when both ends list each other (`UniNodespace.checkConnection`). Breaking an endpoint
//! takes its links off the others (`disconnectAll`). The rules (ranges, types, mounts, faces) are
//! [`crate::energy_network`]'s, proved against the Java.

use crate::{
    Pos,
    energy_network::{Dir, LinkCheck, WireEnd, WireKind, can_connect_wires},
    machine::Logic,
    view::{self, ViewModel},
};
use serde::{Deserialize, Serialize};

pub const WIRE_ITEM: &str = "ntm:wiring_red_copper";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wire {
    /// `connected`: the endpoints wired to this one (a repeat is kept, as in the Java).
    #[serde(default)]
    pub connected: Vec<Pos>,
    /// `color`, painted with a dye (`setColor`).
    #[serde(default)]
    pub color: i32,
}

/// The `ForgeDirection` of a horizontal facing: the block metadata the Java reads.
pub fn dir_of(facing: Option<&str>) -> Dir {
    match facing {
        Some("north") => Dir::North,
        Some("south") => Dir::South,
        Some("west") => Dir::West,
        Some("east") => Dir::East,
        _ => Dir::Unknown,
    }
}

/// The block metadata of an endpoint. The connectors and the medium pylons read it as a direction
/// (the medium, plus `BlockDummyable.offset`, 10); the large pylon's is the core's, 10 + direction.
pub fn meta_of(kind: WireKind, facing: Option<&str>) -> i32 {
    let d = dir_of(facing).ordinal() as i32;
    match kind {
        WireKind::Connector | WireKind::ConnectorSuper => d,
        WireKind::Pylon | WireKind::PylonSteel => 12,
        // `PylonLarge.getMetaForCore`: the player's yaw in 45 degree steps, folded to four (`& 3`). A player looking
        // along an axis gets north (12) or south (13): the pylon turned 0 or 90 degrees. East and west are not reachable
        // from the four directions the engine places by, so a block placed east or west is the turned one.
        WireKind::Large => match dir_of(facing) {
            Dir::North | Dir::South => 12,
            _ => 13,
        },
        _ => d + 10,
    }
}

/// The faces of the endpoint's own cell that other nodes may join, from `createNode`'s neighbours
/// with a direction (`canConnect`).
pub fn faces(kind: WireKind, facing: Option<&str>) -> Vec<Pos> {
    let node = kind.node([0, 0, 0], meta_of(kind, facing), &[]);
    node.connections
        .iter()
        .chain(&node.extra)
        .filter(|(_, d)| *d != Dir::Unknown)
        .map(|(_, d)| d.offset())
        .collect()
}

/// The ports of the machine (one, at the core, open on [`faces`]).
pub fn ports(block: &str, facing: Option<&str>) -> Vec<crate::energy::Port> {
    WireKind::from_id(block).map_or_else(Vec::new, |k| vec![faces(k, facing)])
}

/// `ItemWiring.onItemUse`: what a click with the wire did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireUse {
    /// The click is not on an endpoint: the item is not used.
    NotTaken,
    /// The first click: the item remembers this endpoint.
    Started(Pos),
    /// The second click: the item forgets (whatever came of it); the text tells.
    Finished(&'static str),
}

impl Wire {
    pub fn new() -> Self {
        Self::default()
    }

    /// `addConnection`.
    pub fn add_connection(&mut self, at: Pos) {
        self.connected.push(at);
    }

    /// `disconnectAll`, on the other end: every link to `at` is taken off.
    pub fn forget(&mut self, at: Pos) {
        self.connected.retain(|c| *c != at);
    }

    /// Whether the endpoint lists `at`.
    pub fn lists(&self, at: Pos) -> bool {
        self.connected.contains(&at)
    }
}

/// `TileEntityPylonBase.canConnect(first, second)`.
pub fn can_connect(first: (WireKind, Pos, i32), second: (WireKind, Pos, i32)) -> LinkCheck {
    can_connect_wires(
        &WireEnd { kind: first.0, pos: first.1, meta: first.2 },
        &WireEnd { kind: second.0, pos: second.1, meta: second.2 },
    )
}

/// The wire's message for each outcome (`ItemWiring`).
pub fn message(check: LinkCheck) -> &'static str {
    match check {
        LinkCheck::Ok => "Wire end",
        LinkCheck::TypeMismatch => "Wire error - Pylons are not the same type",
        LinkCheck::SameEndpoint => "Wire error - Cannot connect to the same pylon",
        LinkCheck::TooFar => "Wire error - Pylon is too far away",
    }
}

impl Logic for Wire {
    fn level(&self, _: u8) -> u8 {
        0
    }
    fn view(&self) -> ViewModel {
        ViewModel::new("Torre de energia").text(format!("Ligações: {}", self.connected.len()), view::GREY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_large_pylons_meta_is_one_of_the_two_a_player_along_an_axis_gets() {
        // `PylonLarge.getMetaForCore`: yaw in 45 degree steps `& 3`; the four axis yaws give north (12) or south (13).
        for facing in ["north", "south"] {
            assert_eq!(meta_of(WireKind::Large, Some(facing)), 12, "{facing}");
        }
        for facing in ["east", "west"] {
            assert_eq!(meta_of(WireKind::Large, Some(facing)), 13, "{facing}");
        }
        // The others keep reading their facing as the direction.
        assert_eq!(meta_of(WireKind::Pylon, Some("east")), 12);
        assert_eq!(meta_of(WireKind::MediumWood, Some("east")), 15);
        assert_eq!(meta_of(WireKind::Connector, Some("east")), 5);
    }
}
