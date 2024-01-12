#![no_main]

use std::{
    sync::{Arc, Mutex},
    thread,
};

use hyperion_ring::RingBufMarker;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    if data[0] == 0 {
        return;
    }

    let marker = RingBufMarker::new(data[0] as usize);
    let arr: Box<[Mutex<()>]> = (0..data[0]).map(|_| Mutex::new(())).collect();
    let write_lock = Mutex::new(());
    let read_lock = Mutex::new(());

    thread::scope(|scope| {
        for v in &data[1..] {
            scope.spawn(|| {
                let v = *v;
                if v >= 128 {
                    let lock = read_lock.lock();
                    if let Some(slot) = unsafe { marker.consume(v as usize - 128) } {
                        let (a, b) = slot.slices(&arr[..]);
                        for item in a.iter().chain(b) {
                            drop(item.try_lock().unwrap());
                        }
                        unsafe { marker.release(slot) };
                    }
                    drop(lock);
                } else {
                    let lock = write_lock.lock();
                    if let Some(slot) = unsafe { marker.acquire(v as usize) } {
                        let (a, b) = slot.slices(&arr[..]);
                        for item in a.iter().chain(b) {
                            drop(item.try_lock().unwrap());
                        }
                        unsafe { marker.produce(slot) };
                    }
                    drop(lock);
                }
            });
        }
    });
});
