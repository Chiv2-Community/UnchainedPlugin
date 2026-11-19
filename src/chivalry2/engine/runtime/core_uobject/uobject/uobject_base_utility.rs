//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/UObjectBaseUtility.h>.

use crate::chivalry2::engine::*;
use crate::globals;

pub type FnUObjectBaseUtilityGetPathName =
    unsafe extern "system" fn(&UObjectBase, Option<&UObject>, &mut FString);

#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct UObjectBaseUtility {
    #[inherit]
    pub uobject_base: UObjectBase,
}

impl UObjectBaseUtility {
    pub unsafe fn get_path_name(&self, stop_outer: Option<&UObject>) -> String {
        let mut string = FString::new();
        unsafe {
            globals().uobject_base_utility_get_path_name()(
                &self.uobject_base,
                stop_outer,
                &mut string,
            );
        }
        string.to_string()
    }
}
