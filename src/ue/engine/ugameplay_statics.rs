use crate::ue::{TArray, UClass, UObject};
use unchained_macros::bind_resolver_func;

define_pattern_resolver!(GetAllActorsOfClass,["40 55 53 56 57 48 8D 6C 24 C1 48 81 EC A8 00 00 00 48 8B 05 70 B1 70 02 48 33 C4 48 89 45 1F 41"]);

/// Static native helpers only; this is not a UObject instance layout.
pub struct UGameplayStatics { _private: () }

impl UGameplayStatics {
    /// # Safety
    /// Call on the engine thread with live context/class pointers and writable
    /// engine-compatible array storage. Entries are borrowed native actors.
    #[bind_resolver_func(self::GetAllActorsOfClass)]
    pub unsafe fn get_all_actors_of_class(context: *const UObject, actor_class: *mut UClass, output: *mut TArray<*mut UObject>) {}
}
