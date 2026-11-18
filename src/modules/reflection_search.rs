use std::collections::HashMap;

use async_trait::async_trait;
use clap::Parser;
use regex::{Regex, RegexBuilder};

use crate::commands::Command;
use crate::events::models::{CommandRequest, CommandSource, PermissionFlags};
use crate::game::engine::get_uobject_from_path;
use crate::resolvers::unchained_integration::run_on_game_thread;
use crate::tools::hook_globals::globals;
use crate::ue::exec_thunk::inner_call_of;
use crate::ue::{EFunctionFlags, FProperty, UFunction, UObjectBase, UStruct};

enum SearchKind {
    Classes,
    Functions { native_only: bool },
    Properties,
}

#[derive(Parser)]
#[command(
    name = "find-classes",
    about = "Find loaded Unreal classes by name or path (* and ? wildcards)"
)]
pub struct FindClassesArgs {
    pattern: String,
}

pub struct FindClassesCommand;

#[async_trait]
impl Command<FindClassesArgs> for FindClassesCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::ADMIN
    }
    fn required_source(&self) -> Option<CommandSource> {
        Some(CommandSource::ServerConsole)
    }

    async fn execute(&self, args: FindClassesArgs, _: &CommandRequest) {
        queue_search(SearchKind::Classes, args.pattern);
    }
}

#[derive(Parser)]
#[command(
    name = "find-functions",
    about = "Find loaded Unreal functions by name or path (* and ? wildcards)"
)]
pub struct FindFunctionsArgs {
    pattern: String,
    /// Only show native UFunctions.
    #[arg(long)]
    native: bool,
}

pub struct FindFunctionsCommand;

#[async_trait]
impl Command<FindFunctionsArgs> for FindFunctionsCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::ADMIN
    }
    fn required_source(&self) -> Option<CommandSource> {
        Some(CommandSource::ServerConsole)
    }

    async fn execute(&self, args: FindFunctionsArgs, _: &CommandRequest) {
        queue_search(
            SearchKind::Functions {
                native_only: args.native,
            },
            args.pattern,
        );
    }
}

#[derive(Parser)]
#[command(
    name = "find-properties",
    about = "Find properties on loaded Unreal classes by name or path (* and ? wildcards)"
)]
pub struct FindPropertiesArgs {
    pattern: String,
}

pub struct FindPropertiesCommand;

#[async_trait]
impl Command<FindPropertiesArgs> for FindPropertiesCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::ADMIN
    }
    fn required_source(&self) -> Option<CommandSource> {
        Some(CommandSource::ServerConsole)
    }

    async fn execute(&self, args: FindPropertiesArgs, _: &CommandRequest) {
        queue_search(SearchKind::Properties, args.pattern);
    }
}

fn wildcard_regex(pattern: &str) -> Result<Regex, regex::Error> {
    let pattern = regex::escape(pattern)
        .replace(r"\*", ".*")
        .replace(r"\?", ".");
    RegexBuilder::new(&format!(r"\A{pattern}\z"))
        .case_insensitive(true)
        .build()
}

fn queue_search(kind: SearchKind, pattern: String) {
    let matcher = match wildcard_regex(&pattern) {
        Ok(matcher) => matcher,
        Err(error) => {
            crate::swarn!("Could not search for {pattern:?} ({error})");
            return;
        }
    };
    let match_path = pattern.contains(['/', '.', ':']);
    run_on_game_thread(move || unsafe {
        let (engine_type, label) = match kind {
            SearchKind::Classes => ("Class", "classes"),
            SearchKind::Functions { .. } => ("Function", "functions"),
            SearchKind::Properties => ("Class", "properties"),
        };
        let type_path = format!("/Script/CoreUObject.{engine_type}");
        let target_type = get_uobject_from_path(&type_path, false).cast::<UStruct>();
        if target_type.is_null() {
            crate::swarn!("Cannot search loaded objects because {type_path} is unavailable");
            return;
        }

        let mut matching_types = HashMap::new();
        let mut matches = Vec::new();
        {
            let objects = globals().guobject_array_unchecked().objects();
            for object in objects.iter().flatten() {
                let class = object.class_private.cast::<UStruct>();
                let type_matches = matching_types.entry(class).or_insert_with(|| {
                    let mut current = class;
                    while let Some(class) = current.as_ref() {
                        if std::ptr::eq(class, target_type) {
                            return true;
                        }
                        current = class.super_struct;
                    }
                    false
                });
                if !*type_matches {
                    continue;
                }
                if matches!(kind, SearchKind::Properties) {
                    collect_properties(object, &matcher, match_path, &mut matches);
                    continue;
                }
                if !match_path && !matcher.is_match(&object.name_private.to_string()) {
                    continue;
                }
                let path = object.get_path_name(None);
                if match_path && !matcher.is_match(&path) {
                    continue;
                }
                if let SearchKind::Functions { native_only } = kind {
                    let function = &*(object as *const UObjectBase).cast::<UFunction>();
                    if function
                        .function_flags
                        .contains(EFunctionFlags::FUNC_Native)
                    {
                        let thunk = function.func as usize;
                        let dispatch = inner_call_of(thunk);
                        matches.push(format!(
                            "{path} (native, thunk {thunk:#x}, dispatch {dispatch:x?})"
                        ));
                    } else if !native_only {
                        matches.push(format!("{path} (non-native)"));
                    }
                } else {
                    matches.push(path);
                }
            }
        }
        matches.sort_unstable();
        for path in &matches {
            crate::sinfo!("{path}");
        }
        crate::sinfo!("{} {label} matched {pattern:?}", matches.len());
    });
}

unsafe fn collect_properties(
    object: &UObjectBase,
    matcher: &Regex,
    match_path: bool,
    matches: &mut Vec<String>,
) {
    let owner = &*(object as *const UObjectBase).cast::<UStruct>();
    let mut owner_path = None;
    let mut field = owner.child_properties;
    // Each class contributes its own declarations. Inherited fields appear under their owner.
    for _ in 0..8192 {
        let Some(property_field) = field.as_ref() else {
            break;
        };
        let property = &*field.cast::<FProperty>();
        field = property_field.next;
        let name = property_field.name_private.to_string();
        if !match_path && !matcher.is_match(&name) {
            continue;
        }
        let owner_path = owner_path.get_or_insert_with(|| object.get_path_name(None));
        let path = format!("{owner_path}:{name}");
        if match_path && !matcher.is_match(&path) {
            continue;
        }
        let type_name = property_field
            .class_private
            .as_ref()
            .map(|class| class.name.to_string())
            .unwrap_or_else(|| "unknown type".into());
        let offset = property.offset_internal;
        matches.push(format!("{path} ({type_name}, offset {offset:#x})"));
    }
}
