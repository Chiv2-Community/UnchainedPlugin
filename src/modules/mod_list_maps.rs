
use crate::commands::Command;
use crate::events::models::{CommandRequest, CommandSource, PermissionFlags};
use async_trait::async_trait;
use clap::Parser;
use std::sync::mpsc;

#[derive(Parser, Clone)]
#[command(name = "listmodmaps", about = "Lists mod assets currently known to the Asset Registry under /Game/Mods")]
pub struct ListModMapsArgs {}

pub struct ListModMapsCommand;

#[async_trait]
impl Command<ListModMapsArgs> for ListModMapsCommand {
    fn group(&self) -> String { "Mod Management".into() }
    fn required_permissions(&self) -> PermissionFlags { PermissionFlags::ADMIN }
    fn required_source(&self) -> Option<CommandSource> { Some(CommandSource::ServerConsole) }
    async fn execute(&self, _args: ListModMapsArgs, _command: &CommandRequest) {
        use crate::resolvers::unchained_integration::run_on_game_thread;
        let (tx, rx) = mpsc::channel();
        sinfo!(fn; "Querying the Asset Registry for /Game/Mods on the game thread");

        run_on_game_thread(move || {
            let outcome = match unsafe { crate::game::engine::get_assets_by_path("/Game/Mods", true) } {
                Ok(assets) => {
                    let mut lines: Vec<String> = Vec::new();
                    let mut map_count = 0usize;
                    for asset in assets.as_slice() {
                        if asset.ObjectPath.comparison_index.value == 0 {
                            continue;
                        }
                        let is_map = asset.AssetClass.to_string() == "World";
                        if is_map {
                            map_count += 1;
                        }
                        lines.push(format!(
                            "[{}] {}",
                            if is_map { "map" } else { "asset" },
                            asset.ObjectPath
                        ));
                    }
                    (
                        Ok(format!(
                            "{} registered asset(s) under /Game/Mods, {} of them map(s)",
                            lines.len(),
                            map_count
                        )),
                        lines,
                    )
                }
                Err(e) => (Err(e), Vec::new()),
            };
            let _ = tx.send(outcome);
        });

        match rx.recv() {
            Ok((Ok(summary), lines)) => {
                sinfo!(fn; "{}", summary);
                for line in lines.iter().filter(|l| l.starts_with("[map]")) {
                    sinfo!(fn; "{}", line);
                }
                const SAMPLE: usize = 20;
                let other_count = lines.iter().filter(|l| l.starts_with("[asset]")).count();
                for line in lines.iter().filter(|l| l.starts_with("[asset]")).take(SAMPLE) {
                    sinfo!(fn; "{}", line);
                }
                if other_count > SAMPLE {
                    sinfo!(fn; "... and {} more non-map asset(s)", other_count - SAMPLE);
                }
            }
            Ok((Err(e), _)) => {
                serror!(fn; "Asset Registry query failed: {}", e);
                serror!(fn; "Hint: the GetAssetsByPath signature is still a placeholder — populate the real pattern in src/resolvers/asset_registry.rs and remove the name from RESOLVERS_TO_IGNORE in src/lib.rs.");
            }
            Err(_) => serror!(fn; "Game thread job channel dropped before a result arrived"),
        }
    }
}
