"""The NTM host's storage of a world (`<world>/data/ntm`), as its last save wrote it.

The mod keeps it in memory and writes it whole at the end of each save, as one file `state` (`ntm-store 1\\n`, then
`[u32 len][key][u32 len][value]`, little endian). A world of an earlier version has one file per key instead. What the
game holds now is on disk only after a save: a caller that reads during a play asks for one first (`ctl save` waits for
it, storage included)."""
import json
import struct
from pathlib import Path

MAGIC = b'ntm-store 1\n'


def keys(store):
    """Every key of the storage folder `store` and its bytes."""
    store = Path(store)
    one = store / 'state'
    if one.exists():
        data, out = one.read_bytes(), {}
        if not data.startswith(MAGIC):
            raise ValueError(f'{one}: not an NTM storage')
        at = len(MAGIC)
        while at < len(data):
            (n,) = struct.unpack_from('<I', data, at)
            key = data[at + 4:at + 4 + n].decode()
            at += 4 + n
            (n,) = struct.unpack_from('<I', data, at)
            out[key] = data[at + 4:at + 4 + n]
            at += 4 + n
        return out
    return {p.name: p.read_bytes() for p in store.iterdir() if p.is_file() and p.suffix != '.tmp'} if store.is_dir() else {}


def folders(root):
    """The storage folders of the worlds under `root` (a world's folder has its `level.dat`; the mod's own content
    also has a `data/ntm`)."""
    return sorted(p for p in Path(root).glob('**/data/ntm') if p.is_dir() and (p.parent.parent / 'level.dat').exists())


def value(root, key='world'):
    """The JSON of `key` in the storage of the one world under `root`."""
    found = folders(root)
    if len(found) != 1:
        raise RuntimeError(f'expected one NTM storage under {root}, found {[str(p) for p in found]}')
    raw = keys(found[0]).get(key)
    if raw is None:
        raise KeyError(f'{found[0]}: no key {key!r}')
    return json.loads(raw)
