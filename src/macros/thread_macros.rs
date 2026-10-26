#[macro_export]
macro_rules! check_main_thread {
    () => {
        if std::thread::current().id() == globals().main_thread_id {
            $crate::sinfo!("IS IN MAIN THREAD");
        }
        else {
            $crate::serror!("IS NOT IN MAIN THREAD");
        }
    }
}
