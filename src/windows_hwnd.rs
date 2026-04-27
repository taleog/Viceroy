use std::sync::atomic::{AtomicIsize, Ordering};

static HWND_VALUE: AtomicIsize = AtomicIsize::new(0);

pub fn set(hwnd: isize) {
    HWND_VALUE.store(hwnd, Ordering::Relaxed);
}

pub fn get() -> Option<isize> {
    let value = HWND_VALUE.load(Ordering::Relaxed);
    if value == 0 {
        None
    } else {
        Some(value)
    }
}
