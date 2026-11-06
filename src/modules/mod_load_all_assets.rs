
use crate::commands::Command;
use crate::events::models::{CommandRequest, CommandSource, PermissionFlags};
use async_trait::async_trait;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Clone)]
#[command(name = "loadallassets", about = "Registers every asset in the mod paks into the Asset Registry (no map/marker filtering) and reports load timing")]
pub struct LoadAllAssetsArgs {
    /// Optional pak directory or single pak file; defaults to Content/Paks.
    pub path: Option<String>,
}

pub struct LoadAllAssetsCommand;

#[async_trait]
impl Command<LoadAllAssetsArgs> for LoadAllAssetsCommand {
    fn group(&self) -> String { "Mod Management".into() }
    fn required_permissions(&self) -> PermissionFlags { PermissionFlags::ADMIN }
    // Available on client and server, like the other mod-map diagnostics.
    fn required_source(&self) -> Option<CommandSource> { None }
    async fn execute(&self, args: LoadAllAssetsArgs, _command: &CommandRequest) {
        use crate::features::mod_maps::{default_paks_dir, register_all_assets_async, MapSource};

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

        // Await the async completion with a tokio oneshot (no blocking recv on
        // a tokio worker); the profiler log reports gather/AR/total timings.
        let (tx, rx) = tokio::sync::oneshot::channel();
        register_all_assets_async(source, None, Box::new(move |result| {
            let _ = tx.send(result);
        }));

        match rx.await {
            Ok(Ok(count)) => sinfo!(fn; "Registered {} asset(s) from mod paks", count),
            Ok(Err(e)) => serror!(fn; "All-asset registration failed: {}", e),
            Err(_) => serror!(fn; "All-asset registration channel dropped before a result arrived"),
        }
    }
}
