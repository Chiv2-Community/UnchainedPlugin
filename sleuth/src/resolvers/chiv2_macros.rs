
/// Returns a backend URL FString given a suffix
#[macro_export]
macro_rules! backend_url {
    ($suffix:expr) => {
        $crate::ue::FString::from(format!(
            "{}{}",
            $crate::globals().cli_args.server_browser_backend
                .as_ref()
                .expect("Missing server_browser_backend"),
            $suffix
        ).as_str())
    };
}
