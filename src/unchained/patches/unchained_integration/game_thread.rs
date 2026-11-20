//! Run queued plugin jobs on the native game thread.

use crate::Chivalry2::Engine::Runtime::Engine::Engine::GameEngine::*;
use crate::Chivalry2::Engine::FString;
use crate::commands::NATIVE_COMMAND_QUEUE;
use ::windows::Win32::System::Memory::IsBadReadPtr;
use std::sync::{Mutex, Once};

#[unchained_macros::bind_func_detour(UGameEngine::Tick)]
pub(crate) unsafe fn tick_detour(this: *mut UGameEngine, delta: f32, state: u8) {
    unsafe {
        this.Tick(delta, state);
    }
    ENABLE_IPV6.call_once(|| {
        NATIVE_COMMAND_QUEUE
            .lock()
            .unwrap()
            .push("net.DisableIPv6 0".to_string());
    });
    let reentered = IN_TICK.with(|flag| {
        let was = flag.get();
        flag.set(true);
        was
    });
    if reentered {
        serror!("Re-entrant UGameEngineTick");
        return;
    }
    if unsafe { IsBadReadPtr(Some(this.cast()), 8) }.into() {
        serror!("IsBadReadPtr UGameEngineTick");
        return;
    }
    if !unchained_macros::detour_is_enabled("UGameEngineTick") {
        serror!("Disabled UGameEngineTick");
        return;
    }
    if let Ok(mut commands) = NATIVE_COMMAND_QUEUE.try_lock() {
        for command in commands.drain(..) {
            let command = FString::from(command.as_str());
            let _ = std::panic::catch_unwind(move || unsafe {
                crate::Chivalry2::TBL::TBLUtilityLibrary::TBLUtility::ExecuteConsoleCommand(
                    &command,
                )
            });
        }
    }
    let jobs = match JOB_QUEUE.try_lock() {
        Ok(mut queue) => std::mem::take(&mut *queue),
        Err(_) => Vec::new(),
    };
    for job in jobs {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
        if result.is_err() {
            serror!("GameThreadJob panicked! Catching to prevent engine crash.");
        }
    }
    IN_TICK.with(|flag| flag.set(false));
}

pub fn run_on_game_thread<F: FnOnce() + Send + 'static>(job: F) {
    sdebug!("Task added to job queue");
    JOB_QUEUE.lock().unwrap().push(Box::new(job));
}

pub fn in_game_thread_job() -> bool {
    IN_TICK.with(|flag| flag.get())
}

type GameThreadJob = Box<dyn FnOnce() + Send>;

static JOB_QUEUE: Mutex<Vec<GameThreadJob>> = Mutex::new(Vec::new());

static ENABLE_IPV6: Once = Once::new();

thread_local! {
    static IN_TICK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
