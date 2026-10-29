//! The client↔server protocol over the shared storage folder
//! (docs/fundacao.md, "Protocolo").
//!
//! Each side owns one key and only ever writes that one (`c2s`, `s2c`). A key
//! holds an [`Envelope`]: the writer's session id, the session it believes
//! the reader has, the last of the reader's messages it processed (a
//! cumulative ack), and every message of its own the reader has not acked
//! yet, in order. A message is resent in each write until acked, so a lost
//! or torn write only delays it; the reader drops what it already processed,
//! so it never runs twice.
//!
//! Sessions: a writer that doesn't know its reader yet says `peer: 0`. A
//! reader ignores an envelope addressed to another session (a stale file
//! from an earlier run). A side that sees its reader's session change drops
//! its outbox: those messages were for the old reader.

use crate::{Pos, view::ViewModel};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::VecDeque;

use crate::kv::Kv;

/// What the main hand holds, as the client's `input.held` saw it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    pub item: String,
    pub count: u32,
}

/// Client to server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum C2S {
    /// First message of a client session: a new world entered (or the mod
    /// restarted). `dim` and `pos` say where the client's player is, so the
    /// server knows which of the players online is this client's.
    Hello {
        protocol: u32,
        #[serde(default)]
        dim: String,
        #[serde(default)]
        pos: [f64; 3],
    },
    /// The player opened the machine at `pos` (use button on it).
    Open { dim: String, pos: Pos },
    /// A button of the open screen.
    Action {
        dim: String,
        pos: Pos,
        action: u32,
        held: Option<Held>,
    },
    /// The player left the screen.
    Close,
    /// The detonator (`ntm:detonator`) was used: with `link` (sneaking), on
    /// the bomb at `target`; else it sets off the bomb it is linked to.
    Detonator {
        dim: String,
        target: Option<Pos>,
        link: bool,
    },
}

/// Server to client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum S2C {
    /// Confirmed inventory exposure and persisted player dose.
    Radiation { reading: crate::radiation::Reading },
    /// The world this session is (`worldid`), once known.
    World { wid: String },
    /// The screen of the machine at `pos`: open or refresh it.
    View {
        dim: String,
        pos: Pos,
        vm: ViewModel,
    },
    /// Close the machine screen (the machine went away).
    Close,
    /// Vanilla commands for the client to run quietly as the player
    /// (`command.run-quiet`); results come back through the world
    /// (`token`), never through this channel.
    Run { tx: u32, lines: Vec<String> },
    /// A chat line for the player.
    Toast { text: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope<M> {
    pub v: u32,
    pub sid: u64,
    /// This mailbox's instance: a restart with the same `sid` (a folder too
    /// full to bump the boot counter) still reads as a new session.
    #[serde(default)]
    pub inst: u64,
    pub peer: u64,
    pub ack: u64,
    pub msgs: Vec<(u64, M)>,
}

/// Outbox bound for messages that may be dropped (screens, chat lines):
/// beyond it the oldest of them go. Messages sent with [`Mailbox::send`]
/// are never dropped.
pub const MAX_OUTBOX: usize = 256;
/// Bound on one mailbox write (bytes): well under the 1 MiB a call may write.
pub const MAX_ENVELOPE: usize = 128 << 10;

#[derive(Debug)]
pub struct Mailbox<Out, In> {
    mine: &'static str,
    theirs: &'static str,
    pub sid: u64,
    inst: u64,
    next_seq: u64,
    /// Seq, message, and the tag of a message a newer one replaces
    /// (`send_latest`): those are also the ones dropped first.
    outbox: VecDeque<(u64, Out, Option<&'static str>)>,
    /// The reader's session, once it wrote to us.
    pub peer: Option<u64>,
    peer_inst: u64,
    /// The last of our messages the peer acked.
    acked: u64,
    /// The last of the peer's messages processed.
    recv: u64,
    /// The last bytes read from `theirs`: unchanged bytes are skipped.
    last_raw: Vec<u8>,
    dirty: bool,
    _in: std::marker::PhantomData<In>,
}

/// What a poll found.
#[derive(Debug, Default)]
pub struct Polled<In> {
    pub msgs: Vec<In>,
    /// The peer's session changed (it restarted, or a new one appeared).
    pub new_peer: bool,
}

impl<Out: Serialize + Clone, In: DeserializeOwned> Mailbox<Out, In> {
    pub fn new(mine: &'static str, theirs: &'static str, sid: u64) -> Self {
        Self::with_instance(mine, theirs, sid, sid.rotate_left(29) ^ 0x5bd1_e995)
    }

    pub fn with_instance(mine: &'static str, theirs: &'static str, sid: u64, inst: u64) -> Self {
        Self {
            mine,
            theirs,
            sid,
            inst,
            next_seq: 1,
            outbox: VecDeque::new(),
            peer: None,
            peer_inst: 0,
            acked: 0,
            recv: 0,
            last_raw: Vec::new(),
            dirty: true,
            _in: std::marker::PhantomData,
        }
    }

    /// Sends `m`, kept until acked: never dropped. Its seq.
    pub fn send(&mut self, m: Out) -> u64 {
        self.push(m, None)
    }

    /// Sends `m` in place of any unacked message of the same `tag` (a
    /// screen's refresh): droppable when the outbox overflows.
    pub fn send_latest(&mut self, m: Out, tag: &'static str) -> u64 {
        self.outbox.retain(|(_, _, t)| *t != Some(tag));
        self.push(m, Some(tag))
    }

    fn push(&mut self, m: Out, tag: Option<&'static str>) -> u64 {
        let seq = self.next_seq;
        self.outbox.push_back((seq, m, tag));
        self.next_seq += 1;
        while self.outbox.len() > MAX_OUTBOX {
            let Some(i) = self.outbox.iter().position(|(_, _, t)| t.is_some()) else {
                break;
            };
            self.outbox.remove(i);
        }
        self.dirty = true;
        seq
    }

    pub fn pending(&self) -> usize {
        self.outbox.len()
    }

    /// Whether the current peer acked our message `seq`.
    pub fn acked(&self, seq: u64) -> bool {
        seq <= self.acked
    }

    /// Reads the peer's key: the messages not seen yet, in order.
    pub fn poll(&mut self, kv: &mut impl Kv) -> Polled<In> {
        let mut out = Polled {
            msgs: Vec::new(),
            new_peer: false,
        };
        let Some(raw) = kv.get(self.theirs) else {
            return out;
        };
        if raw == self.last_raw {
            return out;
        }
        // A torn or foreign write fails to parse: retried at the next change.
        let Ok(env) = serde_json::from_slice::<Envelope<In>>(&raw) else {
            return out;
        };
        self.last_raw = raw;
        if env.v != crate::PROTOCOL || (env.peer != 0 && env.peer != self.sid) {
            return out;
        }
        if self.peer != Some(env.sid) || self.peer_inst != env.inst {
            // Messages for the old reader: the owner knows (by seq) which of
            // its own it must send again.
            if self.peer.is_some() {
                self.outbox.clear();
            }
            self.peer = Some(env.sid);
            self.peer_inst = env.inst;
            self.recv = 0;
            self.acked = 0;
            out.new_peer = true;
            self.dirty = true;
        }
        if env.peer == self.sid {
            self.acked = self.acked.max(env.ack);
            let before = self.outbox.len();
            self.outbox.retain(|(seq, _, _)| *seq > env.ack);
            self.dirty |= self.outbox.len() != before;
        }
        for (seq, m) in env.msgs {
            if seq > self.recv {
                self.recv = seq;
                self.dirty = true;
                out.msgs.push(m);
            }
        }
        out
    }

    /// Writes our key when something changed since the last write.
    pub fn flush(&mut self, kv: &mut impl Kv) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        let mut env = Envelope {
            v: crate::PROTOCOL,
            sid: self.sid,
            inst: self.inst,
            peer: self.peer.unwrap_or(0),
            ack: self.recv,
            msgs: self
                .outbox
                .iter()
                .map(|(s, m, _)| (*s, m.clone()))
                .collect::<Vec<_>>(),
        };
        let mut bytes = serde_json::to_vec(&env).map_err(|e| e.to_string())?;
        while bytes.len() > MAX_ENVELOPE && !env.msgs.is_empty() {
            // The newest wait for the next write.
            let keep = env.msgs.len() / 2;
            env.msgs.truncate(keep);
            bytes = serde_json::to_vec(&env).map_err(|e| e.to_string())?;
        }
        kv.set(self.mine, &bytes)?;
        self.dirty = env.msgs.len() != self.outbox.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::MemKv;

    type ServerBox = Mailbox<S2C, C2S>;
    type ClientBox = Mailbox<C2S, S2C>;

    fn hello() -> C2S {
        C2S::Hello {
            protocol: 1,
            dim: String::new(),
            pos: [0.0; 3],
        }
    }

    #[test]
    fn screens_coalesce_runs_are_never_dropped_and_acks_are_seen() {
        let mut kv = MemKv::default();
        let mut s = ServerBox::new("s2c", "c2s", 22);
        let run = s.send(S2C::Run { tx: 1, lines: vec!["give @s a:b 1".into()] });
        for i in 0..1000 {
            s.send_latest(S2C::Toast { text: format!("{i}") }, "view");
        }
        assert_eq!(s.pending(), 2, "one run, the newest screen");
        for i in 0..(MAX_OUTBOX + 10) {
            s.send(S2C::Run { tx: i as u32, lines: vec![] });
        }
        assert_eq!(s.pending(), MAX_OUTBOX + 11, "runs are kept past the bound (the screen went)");
        s.flush(&mut kv).unwrap();
        let mut c = ClientBox::new("c2s", "s2c", 11);
        c.poll(&mut kv);
        c.flush(&mut kv).unwrap();
        assert!(!s.acked(run));
        s.poll(&mut kv);
        assert!(s.acked(run));
    }

    #[test]
    fn a_restart_with_the_same_sid_is_a_new_session() {
        let mut kv = MemKv::default();
        let mut s = ServerBox::new("s2c", "c2s", 22);
        let mut c = ClientBox::with_instance("c2s", "s2c", 11, 1);
        c.send(hello());
        c.flush(&mut kv).unwrap();
        assert!(s.poll(&mut kv).new_peer);
        let mut c = ClientBox::with_instance("c2s", "s2c", 11, 2);
        c.send(hello());
        c.flush(&mut kv).unwrap();
        let got = s.poll(&mut kv);
        assert!(got.new_peer);
        assert_eq!(got.msgs.len(), 1, "seq 1 of the new instance is read");
    }

    #[test]
    fn delivers_once_in_order_and_acks() {
        let mut kv = MemKv::default();
        let mut c = ClientBox::new("c2s", "s2c", 11);
        let mut s = ServerBox::new("s2c", "c2s", 22);
        s.flush(&mut kv).unwrap();
        c.send(hello());
        c.send(C2S::Close);
        c.flush(&mut kv).unwrap();
        let got = s.poll(&mut kv);
        assert!(got.new_peer);
        assert_eq!(got.msgs, vec![hello(), C2S::Close]);
        // Re-reading the same bytes delivers nothing.
        assert!(s.poll(&mut kv).msgs.is_empty());
        s.send(S2C::Toast { text: "oi".into() });
        s.flush(&mut kv).unwrap();
        let got = c.poll(&mut kv);
        assert_eq!(got.msgs, vec![S2C::Toast { text: "oi".into() }]);
        assert_eq!(c.pending(), 0, "the server's ack trimmed the outbox");
        // The client's ack reaches the server: its outbox empties.
        c.flush(&mut kv).unwrap();
        s.poll(&mut kv);
        assert_eq!(s.pending(), 0);
    }

    #[test]
    fn ignores_stale_sessions() {
        let mut kv = MemKv::default();
        // A file left by a client of an older server session.
        let mut old = ClientBox::new("c2s", "s2c", 5);
        old.peer = Some(999);
        old.send(C2S::Close);
        old.flush(&mut kv).unwrap();
        let mut s = ServerBox::new("s2c", "c2s", 22);
        assert!(s.poll(&mut kv).msgs.is_empty());
        assert_eq!(s.peer, None);
        // A new client session is heard, and its restart drops the old outbox.
        let mut c = ClientBox::new("c2s", "s2c", 6);
        c.send(hello());
        c.flush(&mut kv).unwrap();
        assert_eq!(s.poll(&mut kv).msgs.len(), 1);
        s.send(S2C::Close);
        let mut c2 = ClientBox::new("c2s", "s2c", 7);
        c2.send(hello());
        c2.flush(&mut kv).unwrap();
        let got = s.poll(&mut kv);
        assert!(got.new_peer);
        assert_eq!(s.pending(), 0);
    }

    #[test]
    fn torn_write_is_retried() {
        let mut kv = MemKv::default();
        let mut c = ClientBox::new("c2s", "s2c", 1);
        c.send(C2S::Close);
        c.flush(&mut kv).unwrap();
        let whole = kv.map["c2s"].clone();
        kv.map
            .insert("c2s".into(), whole[..whole.len() / 2].to_vec());
        let mut s = ServerBox::new("s2c", "c2s", 2);
        assert!(s.poll(&mut kv).msgs.is_empty());
        kv.map.insert("c2s".into(), whole);
        assert_eq!(s.poll(&mut kv).msgs, vec![C2S::Close]);
    }
}
