#[macro_export]
macro_rules! check_main_thread {
    () => {
        if std::thread::current().id() == globals().main_thread_id {
            $crate::info!("IS IN MAIN THREAD");
        }
        else {
            $crate::error!("IS NOT IN MAIN THREAD");
        }
    }
}
