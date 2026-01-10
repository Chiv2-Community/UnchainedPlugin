#[macro_export]
macro_rules! event {
    ($struct:ident { $($field:ident : $val:expr),* $(,)? }) => {
        if let Some(handle) = $crate::discord::DISCORD_HANDLE.get() {
            handle.dispatch($struct {
                $( $field: $val.into() ),*
            });
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