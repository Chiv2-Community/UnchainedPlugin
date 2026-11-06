
use crate::commands::Command;
use crate::events::models::{CommandRequest, CommandSource, PermissionFlags};
use async_trait::async_trait;
use clap::Parser;
use std::path::PathBuf;
use std::sync::mpsc;

#[derive(Parser, Clone)]
#[command(name = "registermodmaps", about = "Discovers and registers mod maps from the game's Content/Paks dir into the Asset Registry")]
pub struct RegisterModMapsArgs {
    /// Optional pak directory or single pak file; defaults to Content/Paks.
    pub path: Option<String>,
}

pub struct RegisterModMapsCommand;

#[async_trait]
impl Command<RegisterModMapsArgs> for RegisterModMapsCommand {
    fn group(&self) -> String { "Mod Management".into() }
    fn required_permissions(&self) -> PermissionFlags { PermissionFlags::ADMIN }
    fn required_source(&self) -> Option<CommandSource> { Some(CommandSource::ServerConsole) }
    async fn execute(&self, args: RegisterModMapsArgs, _command: &CommandRequest) {
        use crate::features::mod_maps::{default_paks_dir, MapSource, register_mod_maps_async};

        let source = match args.path.map(PathBuf::from) {
            Some(path) if path.is_dir() => MapSource::PakDir(path),
            Some(path) => MapSource::PakFiles(vec![path]),
            None => match default_paks_dir() {
                Some(dir) => MapSource::PakDir(dir),
                None => {
                    serror!(fn; "Could not derive Content/Paks from the executable; pass a path");
                    return;
                }
            },
        };

        let (tx, rx) = mpsc::channel();
        register_mod_maps_async(source, None, Box::new(move |result| {
            let _ = tx.send(result);
        }));

        match rx.recv() {
            Ok(Ok(count)) => sinfo!(fn; "Registered {} mod package(s); run listmodmaps to verify", count),
            Ok(Err(e)) => {
                serror!(fn; "Mod map registration failed: {}", e);
                serror!(fn; "Hint: the GetAssetsByPath / ScanPathsAndFilesSynchronous signatures are still placeholders — populate the real patterns in src/resolvers/asset_registry.rs and remove both names from RESOLVERS_TO_IGNORE in src/lib.rs.");
            }
            Err(_) => serror!(fn; "Registration channel dropped before a result arrived"),
        }
    }
}
