//! Parity of `energy_network` with the Java it ports.
//!
//! `tools/energy-net-oracle/fixtures/java.json` is written by running the real
//! `PowerNetMK2`/`NodeNet` classes and the verbatim-sliced tile-entity code
//! (see that folder's README); every observation here is compared exactly,
//! doubles by their bits.

use ntm_core::energy_network::*;
use serde_json::Value;

fn read(rel: &str) -> Value {
    let path = format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")))
        .unwrap()
}

fn fixture() -> Value {
    read("tools/energy-net-oracle/fixtures/java.json")
}

fn arr<'a>(v: &'a Value, key: &str) -> &'a Vec<Value> {
    v[key]
        .as_array()
        .unwrap_or_else(|| panic!("no array {key}"))
}

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an i64: {v}"))
}

// ------------------------------------------------------------------ packs

#[test]
fn packs_match_the_enum_for_every_damage() {
    for p in arr(&fixture(), "packs") {
        let spec = BatteryPack::from_damage(i(&p["damage"]) as i32).spec();
        assert_eq!(spec.capacity, i(&p["capacity"]), "{p}");
        assert_eq!(spec.charge_rate, i(&p["charge_rate"]), "{p}");
        assert_eq!(spec.discharge_rate, i(&p["discharge_rate"]), "{p}");
    }
}

#[test]
fn packs_match_the_content_table() {
    let data = read("content/machine-recipes/energy_storage.json");
    let packs = arr(&data, "battery_packs");
    assert_eq!(packs.len(), 12);
    for (n, p) in packs.iter().enumerate() {
        assert_eq!(i(&p["metadata"]) as usize, n);
        let spec = &PACKS[n];
        assert_eq!(p["id"].as_str().unwrap(), spec.id);
        assert_eq!(i(&p["capacity_he"]), spec.capacity);
        assert_eq!(i(&p["charge_he_per_tick"]), spec.charge_rate);
        assert_eq!(i(&p["discharge_he_per_tick"]), spec.discharge_rate);
        assert_eq!(p["capacitor"].as_bool().unwrap(), spec.capacitor);
        assert_eq!(BatteryPack::ALL[n].is_capacitor(), spec.capacitor);
        // duration = capacity / discharge
        assert_eq!(i(&p["duration_ticks"]), spec.capacity / spec.discharge_rate);
    }
}

// ------------------------------------------------------------------ power net

/// A plain provider/receiver, like the oracle's `TN`.
struct Plain {
    power: i64,
    max: i64,
    prov: i64,
    rec: i64,
    pri: usize,
    loaded: bool,
}

impl EnergyEntity for Plain {
    fn is_loaded(&self) -> bool {
        self.loaded
    }
    fn power(&self) -> i64 {
        self.power
    }
    fn set_power(&mut self, p: i64) {
        self.power = p;
    }
    fn max_power(&self) -> i64 {
        self.max
    }
    fn provider_speed(&self) -> i64 {
        if self.prov < 0 {
            self.max
        } else {
            self.prov
        }
    }
    fn receiver_speed(&self) -> i64 {
        if self.rec < 0 {
            self.max
        } else {
            self.rec
        }
    }
    fn priority(&self) -> ConnectionPriority {
        ConnectionPriority::ALL[self.pri]
    }
}

/// The REDD, with the oracle's `loaded` switch.
struct LoadedRedd {
    redd: BatteryRedd,
    loaded: bool,
}

impl EnergyEntity for LoadedRedd {
    fn is_loaded(&self) -> bool {
        self.loaded
    }
    fn power(&self) -> i64 {
        self.redd.power()
    }
    fn set_power(&mut self, p: i64) {
        self.redd.set_power(p)
    }
    fn max_power(&self) -> i64 {
        self.redd.max_power()
    }
    fn priority(&self) -> ConnectionPriority {
        self.redd.priority()
    }
    fn transfer_power(&mut self, p: i64) -> i64 {
        self.redd.transfer_power(p)
    }
    fn use_power(&mut self, p: i64) {
        self.redd.use_power(p)
    }
}

enum Node {
    Plain(Plain),
    Redd(Box<LoadedRedd>),
}

impl Node {
    fn entity(&mut self) -> &mut dyn EnergyEntity {
        match self {
            Node::Plain(p) => p,
            Node::Redd(r) => r.as_mut(),
        }
    }

    fn shown(&self) -> String {
        match self {
            Node::Plain(p) => p.power.to_string(),
            Node::Redd(r) => r.redd.power.to_string(),
        }
    }
}

fn run_net_case(case: &Value) {
    let mut net = PowerNet::new();
    let mut nodes: Vec<Node> = Vec::new();
    for (idx, spec) in arr(case, "nodes").iter().enumerate() {
        let loaded = spec["loaded"].as_bool().unwrap();
        if spec["kind"] == "plain" {
            nodes.push(Node::Plain(Plain {
                power: i(&spec["power"]),
                max: i(&spec["max"]),
                prov: i(&spec["prov"]),
                rec: i(&spec["rec"]),
                pri: i(&spec["pri"]) as usize,
                loaded,
            }));
        } else {
            let mut redd = BatteryRedd::new(12);
            redd.power = BigInt::parse(spec["power"].as_str().unwrap()).unwrap();
            nodes.push(Node::Redd(Box::new(LoadedRedd { redd, loaded })));
        }
        if spec["provider"].as_bool().unwrap() {
            net.add_provider(idx);
        }
        if spec["receiver"].as_bool().unwrap() {
            net.add_receiver(idx);
        }
    }
    let mut rng = JavaRandom::new(i(&case["seed"]));
    for after in arr(case, "after") {
        let mut refs: Vec<&mut dyn EnergyEntity> = nodes.iter_mut().map(Node::entity).collect();
        net.update(&mut rng, &mut refs);
        let powers: Vec<String> = nodes.iter().map(Node::shown).collect();
        let want: Vec<String> = arr(after, "powers")
            .iter()
            .map(|p| p.as_str().unwrap().to_string())
            .collect();
        assert_eq!(powers, want, "powers, case {}", case["seed"]);
        assert_eq!(
            net.energy_tracker,
            i(&after["tracker"]),
            "tracker, case {}",
            case["seed"]
        );
        assert_eq!(
            net.providers().len() as i64,
            i(&after["providers"]),
            "providers, case {}",
            case["seed"]
        );
        assert_eq!(
            net.receivers().len() as i64,
            i(&after["receivers"]),
            "receivers, case {}",
            case["seed"]
        );
    }
    // the random stream was consumed exactly as many times as in Java
    assert_eq!(
        rng.next_int(1_000_000) as i64,
        i(&case["next_int"]),
        "rng, case {}",
        case["seed"]
    );
}

#[test]
fn power_net_matches_java_update() {
    let fx = fixture();
    let cases = arr(&fx, "net_cases");
    assert!(cases.len() >= 400);
    for case in cases {
        run_net_case(case);
    }
}

#[test]
fn energy_used_is_subtracted_cumulatively_across_priority_groups() {
    // One 1000 HE provider and four 300 HE receivers, one per priority.
    // Original: after the HIGHEST group (300) toTransfer = 700; after HIGH,
    // energyUsed = 600, and the next subtraction leaves 100 - not the 400
    // that "subtract only this group" would. The lower groups then see less.
    let mut net = PowerNet::new();
    let mut nodes: Vec<Plain> = Vec::new();
    nodes.push(Plain {
        power: 1000,
        max: 1000,
        prov: -1,
        rec: -1,
        pri: 2,
        loaded: true,
    });
    for pri in [4, 3, 1, 0] {
        nodes.push(Plain {
            power: 0,
            max: 300,
            prov: -1,
            rec: -1,
            pri,
            loaded: true,
        });
    }
    net.add_provider(0);
    for n in 1..5 {
        net.add_receiver(n);
    }
    let mut refs: Vec<&mut dyn EnergyEntity> = nodes
        .iter_mut()
        .map(|n| n as &mut dyn EnergyEntity)
        .collect();
    net.update(&mut JavaRandom::new(2), &mut refs);
    let got: Vec<i64> = nodes.iter().map(|n| n.power).collect();
    assert_eq!(got, vec![400, 300, 300, 0, 0]);
    assert_eq!(net.energy_tracker, 600);
}

#[test]
fn java_random_matches_the_jdk() {
    let fx = fixture();
    for stream in arr(&fx, "randoms") {
        let mut r = JavaRandom::new(i(&stream["seed"]));
        for d in arr(stream, "draws") {
            assert_eq!(
                r.next_int(i(&d[0]) as i32) as i64,
                i(&d[1]),
                "seed {}",
                stream["seed"]
            );
        }
    }
}

// ------------------------------------------------------------------ BigInt

#[test]
fn big_int_matches_i128() {
    let samples: [i128; 12] = [
        0,
        1,
        -1,
        4_294_967_295,
        4_294_967_296,
        -4_294_967_296,
        i64::MAX as i128,
        i64::MIN as i128,
        92_233_720_368_547_758,
        123_456_789_012_345_678_901_234_567_890,
        -123_456_789_012_345_678_901_234_567_890,
        (1i128 << 100) + 12_345,
    ];
    for &a in &samples {
        let x = BigInt::parse(&a.to_string()).unwrap();
        assert_eq!(x.to_string(), a.to_string());
        assert_eq!(x.half().to_string(), (a / 2).to_string(), "half {a}");
        assert_eq!(x.long_value(), a as i64, "longValue {a}");
        assert_eq!(x.double_value(), a as f64, "doubleValue {a}");
        for &b in &samples {
            let y = BigInt::parse(&b.to_string()).unwrap();
            assert_eq!(x.add(&y).to_string(), (a + b).to_string(), "{a}+{b}");
            assert_eq!(x.subtract(&y).to_string(), (a - b).to_string(), "{a}-{b}");
            assert_eq!(x.cmp(&y), a.cmp(&b));
            assert_eq!(x.min_of(&y).to_string(), a.min(b).to_string());
        }
    }
    // beyond i128: a 200-bit value halves, adds and rounds like a double should
    let big = BigInt::parse(&format!("1{}", "0".repeat(60))).unwrap();
    assert_eq!(big.double_value(), 1e60);
    assert_eq!(big.add(&big).half(), big);
}

#[test]
fn big_int_bytes_match_java_to_byte_array() {
    let fx = fixture();
    for pair in arr(&fx, "big_bytes") {
        let value = BigInt::parse(pair[0].as_str().unwrap()).unwrap();
        let hex: String = value
            .to_java_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(hex, pair[1].as_str().unwrap(), "{}", pair[0]);
        let bytes: Vec<u8> = (0..hex.len() / 2)
            .map(|k| u8::from_str_radix(&hex[2 * k..2 * k + 2], 16).unwrap())
            .collect();
        assert_eq!(BigInt::from_java_bytes(&bytes), Some(value));
    }
    assert_eq!(BigInt::from_java_bytes(&[]), None);
}

// ------------------------------------------------------------------ REDD, socket

fn slot_of(spec: &Value) -> Option<ItemSlot> {
    if spec.is_null() {
        return None;
    }
    let item = match spec["kind"].as_str().unwrap() {
        "creative" => SlotItem::Creative,
        "other" => SlotItem::Other,
        _ => SlotItem::Pack(i(&spec["damage"]) as i32),
    };
    Some(ItemSlot::new(item, spec["charge"].as_i64()))
}

fn slot_state(slot: &Option<ItemSlot>) -> Value {
    match slot {
        None => Value::Null,
        Some(s) => match s.item {
            SlotItem::Pack(_) => {
                serde_json::json!({"tag": s.tag().is_some(), "charge": s.tag().unwrap_or(0)})
            }
            _ => serde_json::json!({"kind": "nonpack"}),
        },
    }
}

fn powered_ports(powered: bool) -> bool {
    powered
}

#[test]
fn redd_matches_java_update_loop() {
    let fx = fixture();
    let cases = arr(&fx, "redd_cases");
    assert_eq!(cases.len(), 70);
    for case in cases {
        let mut r = BatteryRedd::new(i(&case["meta"]) as i32);
        r.power = BigInt::parse(case["power0"].as_str().unwrap()).unwrap();
        r.slots = [slot_of(&case["slot0"]), slot_of(&case["slot1"])];
        r.base.red_low = i(&case["red_low"]) as i16;
        r.base.red_high = i(&case["red_high"]) as i16;
        let powered = powered_ports(case["powered"].as_bool().unwrap());
        let mut members = Membership {
            provider: false,
            receiver: false,
        };
        for (op, obs) in arr(case, "ops").iter().zip(arr(case, "obs")) {
            let op = op.as_str().unwrap();
            if op == "tick" {
                if let Some(m) = r.update(powered) {
                    members = m;
                }
            } else if let Some(n) = op.strip_prefix("transfer:") {
                r.transfer_power(n.parse().unwrap());
            } else {
                r.use_power(op.strip_prefix("use:").unwrap().parse().unwrap());
            }
            let ctx = format!("{} / {op}", case["power0"]);
            assert_eq!(
                r.power.to_string(),
                obs["power"].as_str().unwrap(),
                "power {ctx}"
            );
            assert_eq!(
                r.delta.to_string(),
                obs["delta"].as_str().unwrap(),
                "delta {ctx}"
            );
            let log: Vec<Value> = r
                .log
                .iter()
                .map(|l| {
                    l.as_ref()
                        .map_or(Value::Null, |b| Value::String(b.to_string()))
                })
                .collect();
            assert_eq!(Value::Array(log), obs["log"], "log {ctx}");
            assert_eq!(r.power(), i(&obs["get_power"]), "get_power {ctx}");
            assert_eq!(r.max_power(), i(&obs["get_max"]), "get_max {ctx}");
            assert_eq!(slot_state(&r.slots[0]), obs["slot0"], "slot0 {ctx}");
            assert_eq!(slot_state(&r.slots[1]), obs["slot1"], "slot1 {ctx}");
            assert_eq!(
                serde_json::json!({"provider": members.provider, "receiver": members.receiver}),
                obs["members"],
                "members {ctx}"
            );
            assert_eq!(
                r.base.priority.ordinal() as i64,
                i(&obs["priority"]),
                "priority {ctx}"
            );
            assert_eq!(
                comparator_power(r.power(), r.max_power()) as i64,
                i(&obs["comparator"]),
                "comparator {ctx}"
            );
            assert_eq!(r.base.cached_mode() as i64, i(&obs["mode"]), "mode {ctx}");
            // the rotor speed is a float animation value; compare with the
            // original exactly (float bits)
            let want = f32::from_bits(i(&obs["speed_bits"]) as u32);
            let got = r.speed();
            if want.is_nan() {
                assert!(got.is_nan(), "speed {ctx}: {got} vs NaN");
            } else {
                let ulps = (got.to_bits() as i64 - want.to_bits() as i64).abs();
                assert!(ulps == 0, "speed {ctx}: {got} vs {want}");
            }
        }
    }
}

#[test]
fn redd_holds_more_than_a_long() {
    let mut r = BatteryRedd::new(12);
    r.power = BigInt::from_i64(i64::MAX);
    r.transfer_power(i64::MAX);
    r.transfer_power(i64::MAX);
    assert_eq!(r.power.to_string(), "27670116110564327421");
    // the network only sees half the connection window
    assert_eq!(r.power(), i64::MAX / 100 / 2);
    assert_eq!(r.max_power(), 92_233_720_368_547_758);
    r.use_power(i64::MAX);
    assert_eq!(r.power.to_string(), "18446744073709551614");
}

#[test]
fn socket_matches_java() {
    let fx = fixture();
    let cases = arr(&fx, "socket_cases");
    assert!(cases.len() > 500);
    for case in cases {
        let mut s = BatterySocket::new(i(&case["meta"]) as i32);
        s.slot = slot_of(&case["slot"]);
        s.base.red_low = i(&case["red_low"]) as i16;
        s.base.red_high = i(&case["red_high"]) as i16;
        s.base.priority = ConnectionPriority::ALL[i(&case["priority_in"]) as usize];
        let powered = case["powered"].as_bool().unwrap();
        let members = s
            .update(powered)
            .map(|m| serde_json::json!({"provider": m.provider, "receiver": m.receiver}));
        let want = &case["obs"];
        let ctx = case.to_string();
        assert_eq!(
            s.base.priority.ordinal() as i64,
            i(&want["priority"]),
            "{ctx}"
        );
        assert_eq!(s.power(), i(&want["get_power"]), "{ctx}");
        assert_eq!(s.max_power(), i(&want["get_max"]), "{ctx}");
        assert_eq!(s.provider_speed(), i(&want["provider_speed"]), "{ctx}");
        assert_eq!(s.receiver_speed(), i(&want["receiver_speed"]), "{ctx}");
        assert_eq!(s.base.cached_mode() as i64, i(&want["mode"]), "{ctx}");
        assert_eq!(
            comparator_power(s.power(), s.max_power()) as i64,
            i(&want["comparator"]),
            "{ctx}"
        );
        assert_eq!(members, Some(want["members"].clone()), "{ctx}");
        assert_eq!(
            s.can_extract_item(powered),
            want["can_extract"].as_bool().unwrap(),
            "{ctx}"
        );
        assert_eq!(
            s.slot
                .as_ref()
                .is_some_and(|x| BatterySocket::is_item_valid(x.item)),
            want["valid"].as_bool().unwrap(),
            "{ctx}"
        );
        assert_eq!(slot_state(&s.slot), want["slot"], "{ctx}");
    }
}

#[test]
fn battery_controls_match_java() {
    let fx = fixture();
    for case in arr(&fx, "control_cases") {
        let mut b = BatteryBase::default();
        b.red_low = i(&case["low"]) as i16;
        b.red_high = i(&case["high"]) as i16;
        b.priority = ConnectionPriority::ALL[i(&case["priority"]) as usize];
        for (key, obs) in arr(case, "keys").iter().zip(arr(case, "obs")) {
            let key = key.as_str().unwrap();
            b.receive_control(BatteryControl {
                low: key == "low",
                high: key == "high",
                priority: key == "priority",
            });
            assert_eq!(b.red_low as i64, i(&obs["low"]));
            assert_eq!(b.red_high as i64, i(&obs["high"]));
            assert_eq!(b.priority.ordinal() as i64, i(&obs["priority"]));
        }
    }
}

#[test]
fn defaults_are_input_when_off_output_when_powered_and_low_priority() {
    let b = BatteryBase::default();
    assert_eq!(
        (b.red_low, b.red_high, b.priority),
        (MODE_INPUT, MODE_OUTPUT, ConnectionPriority::Low)
    );
    let mut b = b;
    assert_eq!(b.relevant_mode(false), MODE_INPUT);
    assert_eq!(b.relevant_mode(true), MODE_OUTPUT);
}

#[test]
fn invalid_priority_resets_to_low_on_update() {
    for bad in [ConnectionPriority::Lowest, ConnectionPriority::Highest] {
        let mut b = BatteryBase::default();
        b.priority = bad;
        b.base_update(false);
        assert_eq!(b.priority, ConnectionPriority::Low);
    }
}

#[test]
fn creative_battery_drains_the_redd_by_its_window() {
    // chargeItemsFromTE returns 0 for the creative battery, so the REDD
    // "removes" its whole visible power each tick (a quirk of the original).
    let mut r = BatteryRedd::new(12);
    r.power = BigInt::from_i64(1_000);
    r.slots[1] = Some(ItemSlot::new(SlotItem::Creative, None));
    r.update(false);
    assert_eq!(r.power.to_string(), "0");
}

#[test]
fn ports_and_connections_match_java() {
    let fx = fixture();
    let pos = [5, 64, -7];
    for g in arr(&fx, "geometry") {
        let meta = i(&g["meta"]) as i32;
        let (ports, cons) = if g["kind"] == "socket" {
            let s = BatterySocket::new(meta);
            (s.port_positions(pos), s.connection_positions(pos))
        } else {
            let r = BatteryRedd::new(meta);
            (r.port_positions(pos), r.connection_positions(pos))
        };
        let want_ports: Vec<Vec<i64>> = arr(g, "ports")
            .iter()
            .map(|p| p.as_array().unwrap().iter().map(i).collect())
            .collect();
        let got_ports: Vec<Vec<i64>> = ports
            .iter()
            .map(|p| p.iter().map(|&c| c as i64).collect())
            .collect();
        assert_eq!(got_ports, want_ports, "{g}");
        let want_cons: Vec<Vec<i64>> = arr(g, "cons")
            .iter()
            .map(|p| p.as_array().unwrap().iter().map(i).collect())
            .collect();
        let got_cons: Vec<Vec<i64>> = cons
            .iter()
            .map(|(p, d)| vec![p[0] as i64, p[1] as i64, p[2] as i64, d.ordinal() as i64])
            .collect();
        assert_eq!(got_cons, want_cons, "{g}");
    }
}

// ------------------------------------------------------------------ wires

fn kind_of(name: &str, block: i64) -> WireKind {
    match (name, block) {
        ("connector", _) => WireKind::Connector,
        ("connector_super", _) => WireKind::ConnectorSuper,
        ("pylon", _) => WireKind::Pylon,
        ("large", _) => WireKind::Large,
        ("medium", 2) => WireKind::MediumWoodTransformer,
        ("medium", 4) => WireKind::MediumSteelTransformer,
        ("medium", 3) => WireKind::MediumSteel,
        _ => WireKind::MediumWood,
    }
}

fn bits(v: f64) -> i64 {
    v.to_bits() as i64
}

fn pos_list(v: &Value) -> Vec<(Vec<i64>, i64)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let c: Vec<i64> = c.as_array().unwrap().iter().map(i).collect();
            (c[..3].to_vec(), c.get(3).copied().unwrap_or(-1))
        })
        .collect()
}

#[test]
fn wires_match_java() {
    let fx = fixture();
    let at = [3, 70, -9];
    for w in arr(&fx, "wires") {
        let meta = i(&w["meta"]) as i32;
        let kind = kind_of(w["kind"].as_str().unwrap(), i(&w["block"]));
        let ctx = w.to_string();
        let ty = match kind.connection_type() {
            ConnectionType::Single => 0,
            ConnectionType::Triple => 1,
            ConnectionType::Quad => 2,
        };
        assert_eq!(ty, i(&w["type"]), "{ctx}");
        assert_eq!(bits(kind.max_wire_length()), i(&w["max_len_bits"]), "{ctx}");
        let mounts: Vec<Vec<i64>> = kind
            .mount_points(meta)
            .iter()
            .map(|m| m.iter().map(|&c| bits(c)).collect())
            .collect();
        let want: Vec<Vec<i64>> = arr(w, "mounts")
            .iter()
            .map(|m| m.as_array().unwrap().iter().map(i).collect())
            .collect();
        assert_eq!(mounts, want, "mounts {ctx}");
        let point: Vec<i64> = kind
            .connection_point(at, meta)
            .iter()
            .map(|&c| bits(c))
            .collect();
        let want: Vec<i64> = w["point"].as_array().unwrap().iter().map(i).collect();
        assert_eq!(point, want, "point {ctx}");
        for (d, ok) in Dir::ALL.iter().zip(arr(w, "can_connect_dir")) {
            assert_eq!(
                kind.can_connect_dir(meta, *d),
                ok.as_bool().unwrap(),
                "dir {d:?} {ctx}"
            );
        }
        let node = kind.node(at, meta, &[[1, 2, 3], [-40, 5, 99]]);
        let flat = |l: &[([i32; 3], Dir)]| -> Vec<(Vec<i64>, i64)> {
            l.iter()
                .map(|(p, d)| (p.iter().map(|&c| c as i64).collect(), d.ordinal() as i64))
                .collect()
        };
        assert_eq!(
            flat(&node.connections),
            pos_list(&w["node"]["connections"]),
            "connections {ctx}"
        );
        assert_eq!(flat(&node.extra), pos_list(&w["node_extra"]), "extra {ctx}");
        assert_eq!(node.positions, vec![at], "{ctx}");
    }
}

#[test]
fn both_plain_pylons_are_the_same_endpoint_class() {
    for meta in 0..16 {
        assert_eq!(
            WireKind::Pylon.mount_points(meta),
            WireKind::PylonSteel.mount_points(meta)
        );
        assert_eq!(
            WireKind::Pylon.node([0, 0, 0], meta, &[]),
            WireKind::PylonSteel.node([0, 0, 0], meta, &[])
        );
    }
}

#[test]
fn links_match_java_validation() {
    let fx = fixture();
    let links = arr(&fx, "links");
    assert_eq!(links.len(), 1504);
    let mut seen = [0usize; 4];
    for l in links {
        let end = |e: &Value| WireEnd {
            kind: kind_of(e["kind"].as_str().unwrap(), 0),
            pos: [
                i(&e["pos"][0]) as i32,
                i(&e["pos"][1]) as i32,
                i(&e["pos"][2]) as i32,
            ],
            meta: i(&e["meta"]) as i32,
        };
        let a = end(&l["a"]);
        let b = if l["same"].as_bool().unwrap() {
            a
        } else {
            end(&l["b"])
        };
        // two different blocks never share a position, so equal positions
        // here can only be the oracle's randomly generated coincidences
        if !l["same"].as_bool().unwrap() && a.pos == b.pos {
            continue;
        }
        let got = can_connect_wires(&a, &b) as i64;
        assert_eq!(got, i(&l["code"]), "{l}");
        seen[got as usize] += 1;
    }
    assert!(
        seen.iter().all(|&n| n > 10),
        "every result is exercised: {seen:?}"
    );
}

#[test]
fn wire_ranges_and_types_match_the_content_table() {
    let data = read("content/machine-recipes/energy_storage.json");
    let ends = data["network"]["wire_endpoints"].as_object().unwrap();
    assert_eq!(ends.len(), WireKind::ALL.len());
    for kind in WireKind::ALL {
        let e = &ends[kind.id()];
        assert_eq!(
            e["max_wire_length_blocks"].as_f64().unwrap(),
            kind.max_wire_length(),
            "{}",
            kind.id()
        );
        let ty = match kind.connection_type() {
            ConnectionType::Single => "SINGLE",
            ConnectionType::Triple => "TRIPLE",
            ConnectionType::Quad => "QUAD",
        };
        assert_eq!(e["connection_type"].as_str().unwrap(), ty);
        assert_eq!(e["transformer"].as_bool().unwrap(), kind.has_transformer());
    }
    let controls: Vec<&str> = data["network"]["controls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert_eq!(controls, ["ntm:cable_switch", "ntm:cable_detector"]);
}

#[test]
fn link_length_is_rounded_to_float_before_the_range_check() {
    // Two large pylons whose true distance is 100.0000033 blocks: the original
    // rounds the length to float (exactly 100) and accepts the wire.
    let fx = fixture();
    let edges: Vec<&Value> = arr(&fx, "links")
        .iter()
        .filter(|l| l["float_edge"] == true)
        .collect();
    assert_eq!(edges.len(), 4);
    for l in edges {
        let end = |e: &Value| WireEnd {
            kind: WireKind::Large,
            pos: [
                i(&e["pos"][0]) as i32,
                i(&e["pos"][1]) as i32,
                i(&e["pos"][2]) as i32,
            ],
            meta: i(&e["meta"]) as i32,
        };
        let (a, b) = (end(&l["a"]), end(&l["b"]));
        let (pa, pb) = (
            a.kind.connection_point(a.pos, a.meta),
            b.kind.connection_point(b.pos, b.meta),
        );
        let sq: f64 = (0..3).map(|k| (pb[k] - pa[k]).powi(2)).sum();
        assert!(sq > 10_000.0 && (sq.sqrt() as f32) as f64 <= 100.0, "{sq}");
        assert_eq!(can_connect_wires(&a, &b), LinkCheck::Ok);
        assert_eq!(i(&l["code"]), 0);
    }
}

#[test]
fn fixture_exercises_long_overflow_and_the_compensation_cap() {
    let fx = fixture();
    let mut overflowing = 0;
    let mut capped = 0;
    for case in arr(&fx, "net_cases") {
        // does any priority group (or the supply) sum past Long.MAX in exact arithmetic?
        let mut demand = [0i128; 5];
        let mut supply = 0i128;
        for n in arr(case, "nodes")
            .iter()
            .filter(|n| n["kind"] == "plain" && n["loaded"] == true)
        {
            let (power, max) = (i(&n["power"]) as i128, i(&n["max"]) as i128);
            if n["receiver"] == true {
                let speed = if i(&n["rec"]) < 0 {
                    max
                } else {
                    i(&n["rec"]) as i128
                };
                let rec = (max - power).min(speed);
                if rec > 0 {
                    demand[i(&n["pri"]) as usize] += rec;
                }
            }
            if n["provider"] == true {
                let speed = if i(&n["prov"]) < 0 {
                    max
                } else {
                    i(&n["prov"]) as i128
                };
                supply += power.min(speed).max(0);
            }
        }
        if supply > i64::MAX as i128 || demand.iter().any(|&d| d > i64::MAX as i128) {
            overflowing += 1;
        }
        if case["label"] == "compensation_cap_200_providers_of_1_vs_receiver_of_199" {
            // the shape that forces more than 100 compensation draws: every share
            // truncates to 0 and 199 HE of "rounding error" are left over
            let nodes = arr(case, "nodes");
            let tiny = nodes
                .iter()
                .filter(|n| n["provider"] == true && i(&n["power"]) == 1 && i(&n["max"]) == 1)
                .count();
            let receivers: Vec<i64> = nodes
                .iter()
                .filter(|n| n["receiver"] == true)
                .map(|n| i(&n["max"]) - i(&n["power"]))
                .collect();
            assert_eq!((tiny, receivers), (200, vec![199]));
            capped += 1;
        }
    }
    assert!(
        overflowing >= 4,
        "overflow cases in the fixture: {overflowing}"
    );
    assert_eq!(capped, 1, "exactly one labelled cap case");
}

#[test]
fn link_range_is_inclusive_and_takes_the_smaller_limit() {
    // Two connectors 10 blocks apart in a line: exactly the range.
    let a = WireEnd {
        kind: WireKind::Connector,
        pos: [0, 0, 0],
        meta: 0,
    };
    let on_edge = WireEnd {
        kind: WireKind::Connector,
        pos: [10, 0, 0],
        meta: 0,
    };
    let past = WireEnd {
        kind: WireKind::Connector,
        pos: [11, 0, 0],
        meta: 0,
    };
    assert_eq!(can_connect_wires(&a, &on_edge), LinkCheck::Ok);
    assert_eq!(can_connect_wires(&a, &past), LinkCheck::TooFar);
    // different types never link, however close; the same block never links to itself
    let medium = WireEnd {
        kind: WireKind::MediumWood,
        pos: [1, 0, 0],
        meta: 12,
    };
    assert_eq!(can_connect_wires(&a, &medium), LinkCheck::TypeMismatch);
    assert_eq!(can_connect_wires(&a, &a), LinkCheck::SameEndpoint);
    // the limit is the smaller of the two: a super connector still stops at 10 against a plain one
    let sup = WireEnd {
        kind: WireKind::ConnectorSuper,
        pos: [50, 0, 0],
        meta: 0,
    };
    assert_eq!(can_connect_wires(&a, &sup), LinkCheck::TooFar);
}

// ------------------------------------------------------------------ switch, ids

#[test]
fn switch_and_detector_follow_the_source() {
    let fx = fixture();
    for s in arr(&fx, "switches") {
        assert_eq!(
            switch_joins_net(i(&s["meta"]) as u8),
            s["creates_node"].as_bool().unwrap()
        );
    }
    assert_eq!((switch_toggled(0), switch_toggled(1)), (1, 0));
    assert_eq!(detector_meta_after(true, 0), Some(1));
    assert_eq!(detector_meta_after(false, 1), Some(0));
    assert_eq!(detector_meta_after(true, 1), None);
    assert_eq!(detector_meta_after(false, 0), None);
    assert_eq!((pylon_migrated_meta(0), pylon_migrated_meta(13)), (12, 13));
}

#[test]
fn all_fourteen_ids_have_a_role_and_exist_in_the_catalog() {
    let ids = read("content/ids.json");
    let text = ids.to_string();
    assert_eq!(NET_BLOCK_IDS.len(), 14);
    for id in NET_BLOCK_IDS {
        assert!(NetBlock::from_id(id).is_some(), "{id}");
        assert!(
            text.contains(&format!("\"{id}\"")) || text.contains(id.trim_start_matches("ntm:")),
            "{id}"
        );
    }
    assert_eq!(
        NetBlock::from_id("ntm:machine_transformer"),
        Some(NetBlock::Transformer)
    );
    assert_eq!(NetBlock::from_id("ntm:nope"), None);
}
