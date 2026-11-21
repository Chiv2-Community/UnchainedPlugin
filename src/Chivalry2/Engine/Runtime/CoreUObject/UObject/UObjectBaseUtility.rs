//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/UObjectBaseUtility.h>.

use crate::Chivalry2::Engine::*;

unchained_macros::define_custom_resolver!(UObjectBaseUtilityGetPathName,
    delegate patternsleuth::resolvers::unreal::UObjectBaseUtilityGetPathName);

#[unchained_macros::runtime_bindings]
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct UObjectBaseUtility {
    #[inherit]
    pub uobject_base: UObjectBase,
}

#[unchained_macros::runtime_bindings]
impl UObjectBaseUtility {
    /// # Safety
    /// Self and any stop object must be live; output must be writable native storage.
    #[unchained_macros::bind_resolved_func(
        self::UObjectBaseUtilityGetPathName
    )]
    pub unsafe fn GetPathName(&self, stop_outer: Option<&UObject>, output: &mut FString) {}

    pub unsafe fn get_path_name(&self, stop_outer: Option<&UObject>) -> String {
        let mut string = FString::new();
        unsafe {
            self.GetPathName(stop_outer, &mut string);
        }
        string.to_string()
    }
}
