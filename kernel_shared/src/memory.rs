use crate::constants::ARCH_WORD_SIZE;

/// Memcpy
///
/// Copy N bytes of memory from one location to another.
///
/// This faster implementation works by copying bytes not one-by-one, but in
/// groups of 8 bytes (or 4 bytes in the case of 32-bit architectures).
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let n_usize: usize = n / ARCH_WORD_SIZE; // Number of word sized groups
    let mut i: usize = 0;

    // Copy `WORD_SIZE` bytes at a time
    let n_fast = n_usize * ARCH_WORD_SIZE;
    while i < n_fast {
        *((dest as usize + i) as *mut usize) = *((src as usize + i) as *const usize);
        i += ARCH_WORD_SIZE;
    }

    // Copy 1 byte at a time
    while i < n {
        *((dest as usize + i) as *mut u8) = *((src as usize + i) as *const u8);
        i += 1;
    }

    dest
}