//! Alocador em pool para blocos pequenos, para instalar no server mod:
//!
//! ```ignore
//! #[global_allocator]
//! static A: ntm_core::explosion::pool::Pool = ntm_core::explosion::pool::Pool::new();
//! ```
//!
//! Cada `world.get-block` devolve uma string que o host aloca no guest via
//! `cabi_realloc`, e cada `BlockChange` aloca a sua. Listas livres por classe
//! de 16 bytes (até 128) evitam o caminho completo do dlmalloc: no Horizon, o
//! fuel por `get-block` caiu de ~540 para ≤ 125 e por escrita de ~445 para
//! ~180 (rampas do spike S7). Os blocos pequenos nunca voltam ao dlmalloc
//! (ficam na lista da sua classe).
//!
//! **Só para alvos de uma thread** (o WebAssembly dos mods): as listas não têm
//! trava. Num programa nativo com threads, não instale como global.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;

const CLASSES: usize = 8; // até 128 bytes

pub struct Pool(UnsafeCell<[*mut u8; CLASSES]>);

// Uma thread só (ver acima).
unsafe impl Sync for Pool {}

impl Pool {
    pub const fn new() -> Self {
        Self(UnsafeCell::new([std::ptr::null_mut(); CLASSES]))
    }
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn class(l: &Layout) -> Option<usize> {
    (l.size() <= CLASSES * 16 && l.align() <= 16).then(|| l.size().max(1).div_ceil(16) - 1)
}

unsafe impl GlobalAlloc for Pool {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let Some(c) = class(&l) else {
            return unsafe { System.alloc(l) };
        };
        let heads = unsafe { &mut *self.0.get() };
        let p = heads[c];
        if p.is_null() {
            return unsafe { System.alloc(Layout::from_size_align_unchecked((c + 1) * 16, 16)) };
        }
        heads[c] = unsafe { *(p as *mut *mut u8) };
        p
    }

    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        // Vetores grandes: o dlmalloc sabe que memória nova já vem zerada.
        if class(&l).is_none() {
            return unsafe { System.alloc_zeroed(l) };
        }
        let p = unsafe { self.alloc(l) };
        if !p.is_null() {
            unsafe { std::ptr::write_bytes(p, 0, l.size()) };
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        let Some(c) = class(&l) else {
            return unsafe { System.dealloc(p, l) };
        };
        let heads = unsafe { &mut *self.0.get() };
        unsafe { *(p as *mut *mut u8) = heads[c] };
        heads[c] = p;
    }
}
