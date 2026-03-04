#[macro_export]
macro_rules! event {
    ($variant:ident($($inner:tt)*)) => {
        if let Some(handle) = $crate::discord::DISCORD_HANDLE.get() {
            handle.dispatch($crate::discord::notifications::GameEvent::$variant($($inner)*));
        }
    };
}

#[macro_export]
macro_rules! dispatch {
    ($event:expr) => {
        if let Some(handle) = $crate::discord::DISCORD_HANDLE.get() {
            handle.dispatch($event);
        }
    };
}