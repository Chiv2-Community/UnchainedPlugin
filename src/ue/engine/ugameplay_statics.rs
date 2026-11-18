use crate::ue::{TArray, UClass, UObject};
use unchained_macros::bind_resolver_func;

/// Static native helpers only; this is not a UObject instance layout.
pub struct UGameplayStatics { _private: () }

impl UGameplayStatics {
    /// # Safety
    /// Call on the engine thread with live context/class pointers and writable
    /// engine-compatible array storage. Entries are borrowed native actors.
    #[bind_resolver_func(crate::resolvers::asset_registry::GetAllActorsOfClass)]
    pub unsafe fn get_all_actors_of_class(context: *const UObject, actor_class: *mut UClass, output: *mut TArray<*mut UObject>) {}
}
