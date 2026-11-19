# Engine source map

`src/chivalry2/engine` follows UE **4.25-plus**, not current UE documentation.
All 76 stock header paths below were checked against Epic's branch.
Each Rust file links its native source at the top.

- `Runtime` is implicit. Omit `Public`, `Private`, and `Classes`.
- Keep module/header namespaces; use snake-case header filenames.
  Thus `Engine/Classes/Engine/World.h` becomes `engine/engine/world.rs`.
- Plugin headers use `plugins/<plugin path>/<header>.rs`; omit their repeated
  `Source/<module>/Classes` scaffolding.
- `online/icmp.rs` omits the redundant `ICMP` module/header repetition.
- `mod.rs` files are indexes, not native owners.
- Keep layouts, resolvers, bindings, native state, and patches with their owners.
  Consolidate compatible inherent impls directly below the owning struct.
- Plugin replacements may live outside the model and target its methods.
  See [native bindings and plugin detours](native-bindings.md). `UDPEcho` uses
  this split; the other detours have not yet migrated.
- Static native calls stay static. Genuine UE globals remain free functions.
  Rust helper namespaces do not imply a native instance layout.

## Stock headers

Rust paths are relative to `src/chivalry2/engine`; native paths are relative to
`Engine/Source/Runtime`, except the two plugin entries.

| Rust file | UE 4.25-plus header |
| --- | --- |
| `asset_registry/asset_data.rs` | `AssetRegistry/Public/AssetData.h` |
| `asset_registry/asset_registry.rs` | `AssetRegistry/Private/AssetRegistry.h` |
| `asset_registry/asset_registry_helpers.rs` | `AssetRegistry/Public/AssetRegistryHelpers.h` |
| `asset_registry/asset_registry_state.rs` | `AssetRegistry/Public/AssetRegistryState.h` |
| `asset_registry/i_asset_registry.rs` | `AssetRegistry/Public/IAssetRegistry.h` |
| `core/containers/array.rs` | `Core/Public/Containers/Array.h` |
| `core/containers/bit_array.rs` | `Core/Public/Containers/BitArray.h` |
| `core/containers/container_allocation_policies.rs` | `Core/Public/Containers/ContainerAllocationPolicies.h` |
| `core/containers/map.rs` | `Core/Public/Containers/Map.h` |
| `core/containers/set.rs` | `Core/Public/Containers/Set.h` |
| `core/containers/sparse_array.rs` | `Core/Public/Containers/SparseArray.h` |
| `core/containers/unreal_string.rs` | `Core/Public/Containers/UnrealString.h` |
| `core/hal/memory_base.rs` | `Core/Public/HAL/MemoryBase.h` |
| `core/hal/memory_misc.rs` | `Core/Public/HAL/MemoryMisc.h` |
| `core/hal/platform.rs` | `Core/Public/HAL/Platform.h` |
| `core/hal/unreal_memory.rs` | `Core/Public/HAL/UnrealMemory.h` |
| `core/internationalization/text.rs` | `Core/Public/Internationalization/Text.h` |
| `core/math/color.rs` | `Core/Public/Math/Color.h` |
| `core/math/rotator.rs` | `Core/Public/Math/Rotator.h` |
| `core/math/vector.rs` | `Core/Public/Math/Vector.h` |
| `core/misc/guid.rs` | `Core/Public/Misc/Guid.h` |
| `core/misc/output_device.rs` | `Core/Public/Misc/OutputDevice.h` |
| `core/serialization/archive.rs` | `Core/Public/Serialization/Archive.h` |
| `core/serialization/large_memory_reader.rs` | `Core/Public/Serialization/LargeMemoryReader.h` |
| `core/templates/shared_pointer.rs` | `Core/Public/Templates/SharedPointer.h` |
| `core/templates/tuple.rs` | `Core/Public/Templates/Tuple.h` |
| `core/templates/type_hash.rs` | `Core/Public/Templates/TypeHash.h` |
| `core/uobject/name_types.rs` | `Core/Public/UObject/NameTypes.h` |
| `core/windows/windows_critical_section.rs` | `Core/Public/Windows/WindowsCriticalSection.h` |
| `core_uobject/templates/subclass_of.rs` | `CoreUObject/Public/Templates/SubclassOf.h` |
| `core_uobject/uobject/class.rs` | `CoreUObject/Public/UObject/Class.h` |
| `core_uobject/uobject/core_net.rs` | `CoreUObject/Public/UObject/CoreNet.h` |
| `core_uobject/uobject/core_online.rs` | `CoreUObject/Public/UObject/CoreOnline.h` |
| `core_uobject/uobject/field.rs` | `CoreUObject/Public/UObject/Field.h` |
| `core_uobject/uobject/object.rs` | `CoreUObject/Public/UObject/Object.h` |
| `core_uobject/uobject/object_macros.rs` | `CoreUObject/Public/UObject/ObjectMacros.h` |
| `core_uobject/uobject/primary_asset_id.rs` | `CoreUObject/Public/UObject/PrimaryAssetId.h` |
| `core_uobject/uobject/script_interface.rs` | `CoreUObject/Public/UObject/ScriptInterface.h` |
| `core_uobject/uobject/soft_object_path.rs` | `CoreUObject/Public/UObject/SoftObjectPath.h` |
| `core_uobject/uobject/soft_object_ptr.rs` | `CoreUObject/Public/UObject/SoftObjectPtr.h` |
| `core_uobject/uobject/stack.rs` | `CoreUObject/Public/UObject/Stack.h` |
| `core_uobject/uobject/unreal_type.rs` | `CoreUObject/Public/UObject/UnrealType.h` |
| `core_uobject/uobject/uobject_array.rs` | `CoreUObject/Public/UObject/UObjectArray.h` |
| `core_uobject/uobject/uobject_base.rs` | `CoreUObject/Public/UObject/UObjectBase.h` |
| `core_uobject/uobject/uobject_base_utility.rs` | `CoreUObject/Public/UObject/UObjectBaseUtility.h` |
| `core_uobject/uobject/uobject_globals.rs` | `CoreUObject/Public/UObject/UObjectGlobals.h` |
| `core_uobject/uobject/uobject_thread_context.rs` | `CoreUObject/Public/UObject/UObjectThreadContext.h` |
| `core_uobject/uobject/weak_object_ptr.rs` | `CoreUObject/Public/UObject/WeakObjectPtr.h` |
| `engine/ai/navigation/nav_agent_interface.rs` | `Engine/Classes/AI/Navigation/NavAgentInterface.h` |
| `engine/engine/asset_manager.rs` | `Engine/Classes/Engine/AssetManager.h` |
| `engine/engine/blueprint.rs` | `Engine/Classes/Engine/Blueprint.h` |
| `engine/engine/blueprint_core.rs` | `Engine/Classes/Engine/BlueprintCore.h` |
| `engine/engine/engine_base_types.rs` | `Engine/Classes/Engine/EngineBaseTypes.h` |
| `engine/engine/game_engine.rs` | `Engine/Classes/Engine/GameEngine.h` |
| `engine/engine/local_player.rs` | `Engine/Classes/Engine/LocalPlayer.h` |
| `engine/engine/net_driver.rs` | `Engine/Classes/Engine/NetDriver.h` |
| `engine/engine/player.rs` | `Engine/Classes/Engine/Player.h` |
| `engine/engine/world.rs` | `Engine/Classes/Engine/World.h` |
| `engine/engine_utils.rs` | `Engine/Public/EngineUtils.h` |
| `engine/game_framework/actor.rs` | `Engine/Classes/GameFramework/Actor.h` |
| `engine/game_framework/character.rs` | `Engine/Classes/GameFramework/Character.h` |
| `engine/game_framework/character_movement_component.rs` | `Engine/Classes/GameFramework/CharacterMovementComponent.h` |
| `engine/game_framework/controller.rs` | `Engine/Classes/GameFramework/Controller.h` |
| `engine/game_framework/game_mode.rs` | `Engine/Classes/GameFramework/GameMode.h` |
| `engine/game_framework/info.rs` | `Engine/Classes/GameFramework/Info.h` |
| `engine/game_framework/online_repl_structs.rs` | `Engine/Classes/GameFramework/OnlineReplStructs.h` |
| `engine/game_framework/pawn.rs` | `Engine/Classes/GameFramework/Pawn.h` |
| `engine/game_framework/player_controller.rs` | `Engine/Classes/GameFramework/PlayerController.h` |
| `engine/game_framework/player_state.rs` | `Engine/Classes/GameFramework/PlayerState.h` |
| `engine/game_framework/save_game.rs` | `Engine/Classes/GameFramework/SaveGame.h` |
| `engine/kismet/gameplay_statics.rs` | `Engine/Classes/Kismet/GameplayStatics.h` |
| `online/icmp.rs` | `Online/ICMP/Public/Icmp.h` |
| `pak_file/i_platform_file_pak.rs` | `PakFile/Public/IPlatformFilePak.h` |
| `sockets/ip_address.rs` | `Sockets/Public/IPAddress.h` |
| `plugins/online/online_subsystem_utils/ip_connection.rs` | `Engine/Plugins/Online/OnlineSubsystemUtils/Source/OnlineSubsystemUtils/Classes/IpConnection.h` |
| `plugins/online/online_subsystem_utils/ip_net_driver.rs` | `Engine/Plugins/Online/OnlineSubsystemUtils/Source/OnlineSubsystemUtils/Classes/IpNetDriver.h` |

## Custom/private exceptions

These files have no verified one-to-one stock UE header. Their top comments link
the nearest SDK/module evidence without claiming a stock declaration.

| File under `extensions/` | Scope |
| --- | --- |
| `assembly_blueprint.rs` | Game-specific Assembly blueprint helpers. |
| `fonline_auth_playfab.rs` | Private PlayFab authentication ABI. |
| `fonline_session_playfab.rs` | Private PlayFab session ABI. |
| `open_api_client_api.rs` | Game-specific OpenAPI request bindings and hooks. |
| `playfab_request_handler.rs` | Private request ABI; `AllowAlternateBackend`'s exact native owner is unverified. |
| `redpoint_steam_auth.rs` | Private Redpoint Steam authentication integration. |
| `unchained.rs` | Plugin marker reflection and blueprint extensions. |

Additional Rust adapters stay beside the native types they adapt:
`AssetRegistryScriptInterface`, opaque registry/reader storage, concrete shared
pointer/subclass wrappers, `FMemory::try_read`, and object-pointer upcasts.
These are not claims of identical native C++ declarations.

Plugin profile policy lives in `src/features/config_overrides.rs`, not stock
engine sources. `StaticFindObject` and `StaticLoadObject` remain genuine free
functions in `core_uobject/uobject/uobject_globals.rs`.
