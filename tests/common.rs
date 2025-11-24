use nix::fcntl::OFlag;
use nix::sys::mman::shm_open;
use nix::sys::stat::Mode;
use std::ffi::CString;

fn generate_sem_key(name: &str) -> i32 {
    // FIXME is the same function of SemaphoreSet
    let mut hash: i32 = 0x1234; // Seed iniziale

    for byte in name.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as i32);
    }

    let key = hash.abs();
    if key == 0 { 0x1234 } else { key }
}

pub fn shm_exists(name: &str) -> bool {
    let shm_name = CString::new(name).unwrap();
    shm_open(shm_name.as_c_str(), OFlag::O_RDONLY, Mode::empty()).is_ok()
}

pub fn sem_exists(name: &str) -> bool {
    unsafe {
        let key = generate_sem_key(name);
        let sem_id = libc::semget(key, 1, 0o666);
        sem_id > 0
    }
}
