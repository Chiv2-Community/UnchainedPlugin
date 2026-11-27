use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use unchained_macros_core::ExitObservation;

#[repr(align(128))]
struct Snapshot {
    drops: Arc<AtomicUsize>,
    panic_on_drop: bool,
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);

        if self.panic_on_drop {
            panic!("snapshot destructor");
        }
    }
}

unsafe fn finish(state: *mut (), result: *const ()) {
    assert_eq!(state as usize % 128, 0);

    let snapshot = unsafe { &*state.cast::<Snapshot>() };

    assert_eq!(snapshot.drops.load(Ordering::SeqCst), 0);

    // No 'static bound on the borrowed result.
    let result = unsafe { &*result.cast::<&u32>() };

    assert_eq!(**result, 17);
}

unsafe fn panics(_: *mut (), _: *const ()) {
    panic!("observer");
}

#[test]

fn snapshot_owner_drops_once_after_finish_discard_panic_and_unwind() {
    assert_eq!(std::mem::size_of::<ExitObservation>(), 3 * std::mem::size_of::<usize>());

    let value = 17_u32;

    let borrowed = &value;

    for mode in 0..5 {
        let drops = Arc::new(AtomicUsize::new(0));

        let observation = unsafe {
            ExitObservation::from_snapshot(
                Snapshot {
                    drops: drops.clone(),
                    panic_on_drop: mode >= 3,
                },
                if mode == 2 { panics } else { finish },
            )
        };

        match mode {
            0 | 3 => unsafe {
                observation.finish((&borrowed as *const &u32).cast());
            },

            1 => drop(observation),
            2 => unsafe {
                observation.finish(std::ptr::null());
            },

            _ => {
                assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _observation = observation;

                    panic!("outer unwind");
                }))
                .is_err());
            }
        }

        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    struct Zst;

    unsafe fn finish_zst(state: *mut (), _: *const ()) {
        let _ = unsafe { &*state.cast::<Zst>() };
    }

    unsafe {
        ExitObservation::from_snapshot(Zst, finish_zst).finish(std::ptr::null());
    }
}
