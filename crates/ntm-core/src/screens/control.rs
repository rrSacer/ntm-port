//! What a machine's screen sends its block and what the block shows it back (RBMK T06): the 1.7.10
//! `NBTControlPacket` (a compound of typed values for `IControlReceiver.receiveControl`) and the TE's network pack,
//! as versioned messages on the engine's mod channels, each tied to the screen session it came from.
//!
//! A session starts when a player opens a screen (`Sessions::open`, its id given to the client with the menu's extra
//! data) and ends with the next one that player opens. A command is taken only from the player's current session, for
//! the place and the block entity generation it opened on, with a sequence number past the last one taken, and from a
//! player within Java's `hasPermission` reach (a distance below 20 from the block's corner, in its dimension).
//! Anything else is refused whole: nothing of the block or of the player changes.

use std::collections::{BTreeMap, HashMap};

/// The messages' format version.
pub const VERSION: u8 = 1;
/// Longest key, string and array a message carries (bytes of UTF-8, values).
pub const MAX_KEY: usize = 64;
pub const MAX_STRING: usize = 4096;
pub const MAX_ARRAY: usize = 4096;
pub const MAX_FIELDS: usize = 64;
/// The engine's cap on a mod message (`native_net_send_to_server`).
pub const MAX_MESSAGE: usize = 30_000;
/// `hasPermission`'s reach.
pub const REACH: f64 = 20.0;

/// An NBT value as the screens send them: the tags the RBMK's `receiveControl`s read.
#[derive(Clone, Debug)]
pub enum Value {
    Byte(i8),
    Int(i32),
    Long(i64),
    Double(f64),
    Str(String),
    IntArray(Vec<i32>),
}

impl PartialEq for Value {
    /// Doubles by their bits (a NaN equals itself, `-0.0` is not `0.0`).
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Byte(a), Value::Byte(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Long(a), Value::Long(b)) => a == b,
            (Value::Double(a), Value::Double(b)) => a.to_bits() == b.to_bits(),
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::IntArray(a), Value::IntArray(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    fn tag(&self) -> u8 {
        // NBT's own type ids.
        match self {
            Value::Byte(_) => 1,
            Value::Int(_) => 3,
            Value::Long(_) => 4,
            Value::Double(_) => 6,
            Value::Str(_) => 8,
            Value::IntArray(_) => 11,
        }
    }
}

/// An `NBTTagCompound` of a screen, read as the 1.7.10's getters read it: a missing key is 0 (or empty), and a numeric
/// tag reads as any number (`NBTPrimitive`: a double read as an int is floored).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fields(pub BTreeMap<String, Value>);

/// A getter the Java answers with a crash report (`getIntArray` of another tag): the command is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch(pub String);

fn floor_i64(d: f64) -> i64 {
    // `(long)Math.floor`, `MathHelper.floor_double` then widened: Java's saturating casts.
    d.floor() as i64
}

impl Fields {
    pub fn set(&mut self, key: &str, v: Value) -> &mut Self {
        self.0.insert(key.to_string(), v);
        self
    }

    pub fn has_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// `getLong` (`func_150291_c`).
    pub fn get_long(&self, key: &str) -> i64 {
        match self.0.get(key) {
            Some(Value::Byte(v)) => i64::from(*v),
            Some(Value::Int(v)) => i64::from(*v),
            Some(Value::Long(v)) => *v,
            Some(Value::Double(v)) => floor_i64(*v),
            _ => 0,
        }
    }

    /// `getInteger` (`func_150287_d`): a long cut to its low bits, a double floored (`MathHelper.floor_double`).
    pub fn get_integer(&self, key: &str) -> i32 {
        match self.0.get(key) {
            Some(Value::Byte(v)) => i32::from(*v),
            Some(Value::Int(v)) => *v,
            Some(Value::Long(v)) => *v as i32,
            Some(Value::Double(v)) => floor_double(*v),
            _ => 0,
        }
    }

    /// `getByte` (`func_150290_f`).
    pub fn get_byte(&self, key: &str) -> i8 {
        match self.0.get(key) {
            Some(Value::Byte(v)) => *v,
            Some(Value::Int(v)) => *v as i8,
            Some(Value::Long(v)) => *v as i8,
            Some(Value::Double(v)) => floor_double(*v) as i8,
            _ => 0,
        }
    }

    /// `getBoolean`: `getByte != 0`.
    pub fn get_boolean(&self, key: &str) -> bool {
        self.get_byte(key) != 0
    }

    /// `getDouble` (`func_150286_g`).
    pub fn get_double(&self, key: &str) -> f64 {
        match self.0.get(key) {
            Some(Value::Byte(v)) => f64::from(*v),
            Some(Value::Int(v)) => f64::from(*v),
            Some(Value::Long(v)) => *v as f64,
            Some(Value::Double(v)) => *v,
            _ => 0.0,
        }
    }

    /// `getString`: a string's text, another tag's `toString` (`5`, `5b`, `5L`), empty when missing. A double's
    /// `toString` is Java's `Double.toString`, which the screens never ask for: its text here is the shortest that
    /// reads back, with the `d`.
    pub fn get_string(&self, key: &str) -> String {
        match self.0.get(key) {
            Some(Value::Str(s)) => s.clone(),
            Some(Value::Byte(v)) => format!("{v}b"),
            Some(Value::Int(v)) => v.to_string(),
            Some(Value::Long(v)) => format!("{v}L"),
            Some(Value::Double(v)) => format!("{}d", crate::snbt::java_double(*v)),
            Some(Value::IntArray(a)) => format!("[{}]", a.iter().map(|v| format!("{v},")).collect::<String>()),
            None => String::new(),
        }
    }

    /// `getIntArray`: empty when missing; another tag is the Java's crash report.
    pub fn get_int_array(&self, key: &str) -> Result<Vec<i32>, Mismatch> {
        match self.0.get(key) {
            None => Ok(Vec::new()),
            Some(Value::IntArray(a)) => Ok(a.clone()),
            Some(_) => Err(Mismatch(key.to_string())),
        }
    }
}

/// `MathHelper.floor_double`: `(int) d`, less one when that is above `d`.
pub fn floor_double(d: f64) -> i32 {
    let i = d as i32;
    if d < f64::from(i) { i - 1 } else { i }
}

/// What a screen's client sends: its fields for the block at `pos`, in session `session` of the block entity's
/// `generation`, the `seq`th command of the session.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub session: u64,
    pub generation: u64,
    pub pos: [i32; 3],
    pub seq: u32,
    pub fields: Fields,
}

/// What the block shows the screen (its network pack), as of server tick `tick`.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub session: u64,
    pub generation: u64,
    pub pos: [i32; 3],
    pub tick: i64,
    pub fields: Fields,
}

/// What a client gets when its screen opens (the menu's extra data): its session and the block's first snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct Grant {
    pub session: u64,
    pub snapshot: Snapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Version(u8),
    Kind(u8),
    Short,
    Trailing,
    Tag(u8),
    Utf8,
    TooLong(&'static str),
    DuplicateKey(String),
}

const COMMAND: u8 = 1;
const SNAPSHOT: u8 = 2;
const GRANT: u8 = 3;

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u16(s.len() as u16);
        self.0.extend_from_slice(s.as_bytes());
    }
    fn head(&mut self, kind: u8, session: u64, generation: u64, pos: [i32; 3]) {
        self.u8(VERSION);
        self.u8(kind);
        self.u64(session);
        self.u64(generation);
        for v in pos {
            self.u32(v as u32);
        }
    }
    fn fields(&mut self, f: &Fields) {
        self.u16(f.0.len() as u16);
        for (k, v) in &f.0 {
            self.str(k);
            self.u8(v.tag());
            match v {
                Value::Byte(b) => self.u8(*b as u8),
                Value::Int(i) => self.u32(*i as u32),
                Value::Long(l) => self.u64(*l as u64),
                Value::Double(d) => self.u64(d.to_bits()),
                Value::Str(s) => self.str(s),
                Value::IntArray(a) => {
                    self.u16(a.len() as u16);
                    for i in a {
                        self.u32(*i as u32);
                    }
                }
            }
        }
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], DecodeError> {
        if self.0.len() < n {
            return Err(DecodeError::Short);
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn str(&mut self, max: usize, what: &'static str) -> Result<String, DecodeError> {
        let n = usize::from(self.u16()?);
        if n > max {
            return Err(DecodeError::TooLong(what));
        }
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| DecodeError::Utf8)
    }
    fn head(&mut self, kind: u8) -> Result<(u64, u64, [i32; 3]), DecodeError> {
        let v = self.u8()?;
        if v != VERSION {
            return Err(DecodeError::Version(v));
        }
        let k = self.u8()?;
        if k != kind {
            return Err(DecodeError::Kind(k));
        }
        Ok((self.u64()?, self.u64()?, [self.u32()? as i32, self.u32()? as i32, self.u32()? as i32]))
    }
    fn fields(&mut self) -> Result<Fields, DecodeError> {
        let n = usize::from(self.u16()?);
        if n > MAX_FIELDS {
            return Err(DecodeError::TooLong("fields"));
        }
        let mut f = Fields::default();
        for _ in 0..n {
            let key = self.str(MAX_KEY, "key")?;
            let v = match self.u8()? {
                1 => Value::Byte(self.u8()? as i8),
                3 => Value::Int(self.u32()? as i32),
                4 => Value::Long(self.u64()? as i64),
                6 => Value::Double(f64::from_bits(self.u64()?)),
                8 => Value::Str(self.str(MAX_STRING, "string")?),
                11 => {
                    let len = usize::from(self.u16()?);
                    if len > MAX_ARRAY {
                        return Err(DecodeError::TooLong("array"));
                    }
                    Value::IntArray((0..len).map(|_| self.u32().map(|v| v as i32)).collect::<Result<_, _>>()?)
                }
                t => return Err(DecodeError::Tag(t)),
            };
            if f.0.insert(key.clone(), v).is_some() {
                return Err(DecodeError::DuplicateKey(key));
            }
        }
        Ok(f)
    }
    fn end(&self) -> Result<(), DecodeError> {
        if self.0.is_empty() { Ok(()) } else { Err(DecodeError::Trailing) }
    }
}

/// Why fields cannot go out: a key, string or array past the format's limits, or the whole past the engine's cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodeError(pub &'static str);

fn check(f: &Fields) -> Result<(), EncodeError> {
    if f.0.len() > MAX_FIELDS {
        return Err(EncodeError("fields"));
    }
    for (k, v) in &f.0 {
        if k.len() > MAX_KEY {
            return Err(EncodeError("key"));
        }
        match v {
            Value::Str(s) if s.len() > MAX_STRING => return Err(EncodeError("string")),
            Value::IntArray(a) if a.len() > MAX_ARRAY => return Err(EncodeError("array")),
            _ => {}
        }
    }
    Ok(())
}

fn capped(w: Writer) -> Result<Vec<u8>, EncodeError> {
    if w.0.len() > MAX_MESSAGE { Err(EncodeError("message")) } else { Ok(w.0) }
}

impl Command {
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        check(&self.fields)?;
        let mut w = Writer(Vec::new());
        w.head(COMMAND, self.session, self.generation, self.pos);
        w.u32(self.seq);
        w.fields(&self.fields);
        capped(w)
    }

    pub fn decode(data: &[u8]) -> Result<Command, DecodeError> {
        let mut r = Reader(data);
        let (session, generation, pos) = r.head(COMMAND)?;
        let seq = r.u32()?;
        let fields = r.fields()?;
        r.end()?;
        Ok(Command { session, generation, pos, seq, fields })
    }
}

impl Snapshot {
    fn write(&self, w: &mut Writer, kind: u8) {
        w.head(kind, self.session, self.generation, self.pos);
        w.u64(self.tick as u64);
        w.fields(&self.fields);
    }

    fn read(r: &mut Reader<'_>, kind: u8) -> Result<Snapshot, DecodeError> {
        let (session, generation, pos) = r.head(kind)?;
        let tick = r.u64()? as i64;
        let fields = r.fields()?;
        Ok(Snapshot { session, generation, pos, tick, fields })
    }

    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        check(&self.fields)?;
        let mut w = Writer(Vec::new());
        self.write(&mut w, SNAPSHOT);
        capped(w)
    }

    pub fn decode(data: &[u8]) -> Result<Snapshot, DecodeError> {
        let mut r = Reader(data);
        let s = Snapshot::read(&mut r, SNAPSHOT)?;
        r.end()?;
        Ok(s)
    }
}

impl Grant {
    /// The session rides in the snapshot's head; a grant is a snapshot of its own kind.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        check(&self.snapshot.fields)?;
        let mut w = Writer(Vec::new());
        Snapshot { session: self.session, ..self.snapshot.clone() }.write(&mut w, GRANT);
        capped(w)
    }

    pub fn decode(data: &[u8]) -> Result<Grant, DecodeError> {
        let mut r = Reader(data);
        let snapshot = Snapshot::read(&mut r, GRANT)?;
        r.end()?;
        Ok(Grant { session: snapshot.session, snapshot })
    }
}

/// Why a command was not taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The player has no screen open on the server.
    NoSession,
    /// Not the session the player has now (an earlier screen's).
    OldSession,
    /// The session is for another block or dimension.
    OtherPlace,
    /// The block entity is not the one the session opened on (broken, replaced, or not adopted).
    Generation,
    /// A sequence number not past the last one taken (a repeat).
    Repeated,
    /// Beyond `hasPermission`'s reach, or in another dimension.
    TooFar,
    /// The block's `receiveControl` would throw (D2: refused whole).
    Invalid(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: u64,
    pub dim: String,
    pub pos: [i32; 3],
    pub generation: u64,
    pub last_seq: Option<u32>,
}

/// The screen sessions of the server's players: one current each.
#[derive(Clone, Debug, Default)]
pub struct Sessions {
    next: u64,
    by_player: HashMap<String, Session>,
}

impl Sessions {
    /// A player opens a screen on the block at `pos` of `generation`: a new session, the earlier one gone.
    pub fn open(&mut self, player: &str, dim: &str, pos: [i32; 3], generation: u64) -> u64 {
        self.next += 1;
        let id = self.next;
        self.by_player.insert(player.to_string(), Session { id, dim: dim.to_string(), pos, generation, last_seq: None });
        id
    }

    /// The player's session ends (they left).
    pub fn close(&mut self, player: &str) {
        self.by_player.remove(player);
    }

    pub fn current(&self, player: &str) -> Option<&Session> {
        self.by_player.get(player)
    }

    /// Whether `player`, standing at `at` in `player_dim`, may give `cmd` to a block entity now of `generation` (None:
    /// no machine there); on yes, the sequence number is taken.
    pub fn accept(&mut self, player: &str, player_dim: &str, at: [f64; 3], cmd: &Command, generation: Option<u64>) -> Result<(), Refusal> {
        let s = self.by_player.get_mut(player).ok_or(Refusal::NoSession)?;
        if s.id != cmd.session {
            return Err(Refusal::OldSession);
        }
        if s.pos != cmd.pos {
            return Err(Refusal::OtherPlace);
        }
        if generation != Some(s.generation) || cmd.generation != s.generation {
            return Err(Refusal::Generation);
        }
        if s.last_seq.is_some_and(|last| cmd.seq <= last) {
            return Err(Refusal::Repeated);
        }
        if player_dim != s.dim || !within_reach(cmd.pos, at) {
            return Err(Refusal::TooFar);
        }
        s.last_seq = Some(cmd.seq);
        Ok(())
    }
}

/// `Vec3.createVectorHelper(xCoord - posX, yCoord - posY, zCoord - posZ).lengthVector() < 20`: from the block's
/// corner to the player's feet.
pub fn within_reach(pos: [i32; 3], at: [f64; 3]) -> bool {
    let d = [f64::from(pos[0]) - at[0], f64::from(pos[1]) - at[1], f64::from(pos[2]) - at[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < REACH
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Fields {
        let mut f = Fields::default();
        f.set("levelUpper", Value::Double(100.0))
            .set("nan", Value::Double(f64::from_bits(0x7ff8_0000_0000_0001)))
            .set("neg0", Value::Double(-0.0))
            .set("inf", Value::Double(f64::NEG_INFINITY))
            .set("min", Value::Int(i32::MIN))
            .set("long", Value::Long(i64::MAX))
            .set("b", Value::Byte(-128))
            .set("label", Value::Str("ÄÖ 測試 §".into()))
            .set("empty", Value::Str(String::new()))
            .set("cols", Value::IntArray((0..225).collect()));
        f
    }

    #[test]
    fn every_message_reads_back_bit_for_bit() {
        let c = Command { session: u64::MAX, generation: 7, pos: [i32::MIN, -64, i32::MAX], seq: u32::MAX, fields: sample() };
        assert_eq!(Command::decode(&c.encode().unwrap()).unwrap(), c);
        let s = Snapshot { session: 3, generation: 9, pos: [1, 2, 3], tick: -1, fields: sample() };
        assert_eq!(Snapshot::decode(&s.encode().unwrap()).unwrap(), s);
        let g = Grant { session: 4, snapshot: Snapshot { session: 4, ..s.clone() } };
        assert_eq!(Grant::decode(&g.encode().unwrap()).unwrap(), g);
        // Each kind reads only as itself.
        assert_eq!(Snapshot::decode(&c.encode().unwrap()), Err(DecodeError::Kind(1)));
        assert_eq!(Command::decode(&g.encode().unwrap()), Err(DecodeError::Kind(3)));
    }

    #[test]
    fn a_broken_message_is_refused_whole() {
        let c = Command { session: 1, generation: 1, pos: [0, 0, 0], seq: 1, fields: sample() };
        let bytes = c.encode().unwrap();
        // Every cut is short, never a part.
        for n in 0..bytes.len() {
            assert!(Command::decode(&bytes[..n]).is_err(), "cut at {n}");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(Command::decode(&longer), Err(DecodeError::Trailing));
        let mut v2 = bytes.clone();
        v2[0] = 2;
        assert_eq!(Command::decode(&v2), Err(DecodeError::Version(2)));
        // An unknown tag, a duplicate key, bad UTF-8.
        let one = |tag: u8, value: &[u8], key: &[u8], count: u16| {
            let mut w = Writer(Vec::new());
            w.head(COMMAND, 1, 1, [0, 0, 0]);
            w.u32(1);
            w.u16(count);
            for _ in 0..count {
                w.u16(key.len() as u16);
                w.0.extend_from_slice(key);
                w.u8(tag);
                w.0.extend_from_slice(value);
            }
            w.0
        };
        assert_eq!(Command::decode(&one(2, &[0, 0], b"k", 1)), Err(DecodeError::Tag(2)));
        assert_eq!(Command::decode(&one(3, &[0, 0, 0, 1], b"k", 2)), Err(DecodeError::DuplicateKey("k".into())));
        assert_eq!(Command::decode(&one(3, &[0, 0, 0, 1], &[0xff, 0xfe], 1)), Err(DecodeError::Utf8));
        assert_eq!(Command::decode(&one(3, &[0, 0, 0, 1], &[b'k'; 65], 1)), Err(DecodeError::TooLong("key")));
    }

    #[test]
    fn limits_hold_on_both_sides() {
        let mut f = Fields::default();
        f.set("s", Value::Str("x".repeat(MAX_STRING)));
        let ok = Command { session: 1, generation: 1, pos: [0; 3], seq: 1, fields: f.clone() };
        assert!(Command::decode(&ok.encode().unwrap()).is_ok());
        f.set("s", Value::Str("x".repeat(MAX_STRING + 1)));
        assert_eq!(Command { fields: f, ..ok.clone() }.encode(), Err(EncodeError("string")));
        let mut big = Fields::default();
        for i in 0..8 {
            big.set(&format!("a{i}"), Value::IntArray(vec![0; MAX_ARRAY]));
        }
        assert_eq!(Command { fields: big, ..ok.clone() }.encode(), Err(EncodeError("message")));
        let mut many = Fields::default();
        for i in 0..=MAX_FIELDS {
            many.set(&format!("k{i}"), Value::Byte(0));
        }
        assert_eq!(Command { fields: many, ..ok }.encode(), Err(EncodeError("fields")));
    }

    #[test]
    fn the_getters_are_nbt_tag_compounds() {
        let mut f = Fields::default();
        f.set("d", Value::Double(-1.5)).set("i", Value::Int(300)).set("l", Value::Long(1 << 40)).set("s", Value::Str("x".into()));
        assert_eq!((f.get_integer("d"), f.get_long("d"), f.get_byte("d")), (-2, -2, -2), "floored");
        assert_eq!((f.get_double("i"), f.get_byte("i"), f.get_boolean("i")), (300.0, 44, true));
        assert_eq!(f.get_integer("l"), 0, "a long's low 32 bits");
        assert_eq!((f.get_double("missing"), f.get_integer("missing"), f.get_string("missing")), (0.0, 0, String::new()));
        assert_eq!((f.get_double("s"), f.get_string("i"), f.get_string("l")), (0.0, "300".into(), "1099511627776L".into()));
        assert_eq!(f.get_int_array("missing"), Ok(vec![]));
        assert_eq!(f.get_int_array("i"), Err(Mismatch("i".into())));
        assert_eq!(floor_double(f64::NAN), 0);
        assert_eq!(floor_double(-0.0), 0);
        assert_eq!(floor_double(1e300), i32::MAX);
    }

    #[test]
    fn a_command_is_taken_only_from_the_current_session_in_reach_once() {
        let mut s = Sessions::default();
        let first = s.open("p", "ow", [0, 70, 0], 5);
        let cmd = |session, seq| Command { session, generation: 5, pos: [0, 70, 0], seq, fields: Fields::default() };
        let near = [0.5, 70.0, 0.5];
        assert_eq!(s.accept("q", "ow", near, &cmd(first, 1), Some(5)), Err(Refusal::NoSession));
        assert_eq!(s.accept("p", "ow", near, &cmd(first, 1), Some(5)), Ok(()));
        assert_eq!(s.accept("p", "ow", near, &cmd(first, 1), Some(5)), Err(Refusal::Repeated));
        assert_eq!(s.accept("p", "ow", near, &cmd(first, 0), Some(5)), Err(Refusal::Repeated));
        assert_eq!(s.accept("p", "ow", near, &cmd(first, 2), Some(5)), Ok(()));
        // Reach: below 20 from the corner, in the same dimension.
        assert_eq!(s.accept("p", "ow", [19.99, 70.0, 0.0], &cmd(first, 3), Some(5)), Ok(()));
        assert_eq!(s.accept("p", "ow", [20.0, 70.0, 0.0], &cmd(first, 4), Some(5)), Err(Refusal::TooFar));
        assert_eq!(s.accept("p", "nether", near, &cmd(first, 4), Some(5)), Err(Refusal::TooFar));
        // The block entity changed under the session, or is gone.
        assert_eq!(s.accept("p", "ow", near, &cmd(first, 4), Some(6)), Err(Refusal::Generation));
        assert_eq!(s.accept("p", "ow", near, &cmd(first, 4), None), Err(Refusal::Generation));
        assert_eq!(s.accept("p", "ow", near, &Command { generation: 4, ..cmd(first, 4) }, Some(5)), Err(Refusal::Generation));
        assert_eq!(s.accept("p", "ow", near, &Command { pos: [1, 70, 0], ..cmd(first, 4) }, Some(5)), Err(Refusal::OtherPlace));
        // A new screen: the old session is refused, the new one starts its own count.
        let second = s.open("p", "ow", [0, 70, 0], 5);
        assert_ne!(first, second);
        assert_eq!(s.accept("p", "ow", near, &cmd(first, 9), Some(5)), Err(Refusal::OldSession));
        assert_eq!(s.accept("p", "ow", near, &cmd(second, 1), Some(5)), Ok(()));
        s.close("p");
        assert_eq!(s.accept("p", "ow", near, &cmd(second, 2), Some(5)), Err(Refusal::NoSession));
    }
}
