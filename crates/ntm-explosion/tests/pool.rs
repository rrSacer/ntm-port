use ntm_explosion::pool::Pool;
use std::alloc::{GlobalAlloc, Layout};

#[test]
fn small_blocks_are_reused() {
    let pool = Pool::new();
    let l = Layout::from_size_align(13, 1).unwrap();
    unsafe {
        let a = pool.alloc(l);
        pool.dealloc(a, l);
        let b = pool.alloc(Layout::from_size_align(16, 8).unwrap());
        assert_eq!(a, b, "mesma classe de 16 bytes reaproveita o bloco");
        let big = Layout::from_size_align(4096, 8).unwrap();
        let z = pool.alloc_zeroed(big);
        assert!(std::slice::from_raw_parts(z, 4096).iter().all(|&v| v == 0));
        pool.dealloc(z, big);
        let zs = pool.alloc_zeroed(l);
        std::ptr::write_bytes(zs, 0xff, 13);
        pool.dealloc(zs, l);
        let zs = pool.alloc_zeroed(l);
        assert!(std::slice::from_raw_parts(zs, 13).iter().all(|&v| v == 0));
    }
}
