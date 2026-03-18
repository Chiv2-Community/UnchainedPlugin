use std::{
    env,
    path::PathBuf,
    process::Command,
};
use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "Project automation tasks")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build and install the plugin to the Chivalry 2 directory
    Install {
        /// Path to Chivalry 2 root directory (folder containing TBL)
        #[arg(long = "chivalry2-dir", value_name = "PATH")]
        chivalry2_dir: Option<PathBuf>,
        /// Run cargo clean before building
        #[arg(long)]
        clean: bool,
        /// Always rebuild even if DLL exists
        #[arg(long)]
        rebuild: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let project_root = get_project_root().unwrap_or_else(|_| env::current_dir().unwrap());
    let env_file = project_root.join(".env");
    
    if env_file.exists() {
        match dotenvy::from_path(&env_file) {
            Ok(_) => println!("Loaded environment variables from {:?}", env_file),
            Err(e) => println!("Error loading {:?}: {}", env_file, e),
        }
    }

    match cli.command {
        Commands::Install { chivalry2_dir, clean, rebuild } => {
            let arg_or_env_chiv2_dir =
                chivalry2_dir
                    .or_else(|| env::var("CHIVALRY2_DIR").ok().map(PathBuf::from));

            install(arg_or_env_chiv2_dir, clean, rebuild)
        }
    }
}

fn install(chivalry2_dir_flag: Option<PathBuf>, clean: bool, rebuild: bool) -> Result<()> {
    let chivalry2_dir = match chivalry2_dir_flag {
        Some(path) => path,
        None => {
            let var = env::var("CHIVALRY2_DIR")
                .map_err(|_| anyhow!("CHIVALRY2_DIR is not set. Provide --chivalry2-dir or set it in .env"))?;
            PathBuf::from(var)
        }
    };

    let project_root = get_project_root()?;
    let (plugin_name, lib_name) = get_project_info(&project_root)?;
    let dll_name = format!("{}.dll", lib_name);
    let dll_path = project_root.join("target").join("debug").join(&dll_name);

    if clean {
        clean_build(&plugin_name)?;
    } else if rebuild || !dll_path.exists() {
        if !dll_path.exists() {
            println!("Build artifact not found for {}...", plugin_name);
        }

        build(&plugin_name)?;

        if !dll_path.exists() {
            return Err(anyhow!("Build completed but DLL not found at {:?}", dll_path));
        }
    }

    let dest_dir = chivalry2_dir
        .join("TBL")
        .join("Binaries")
        .join("Win64")
        .join("plugins");

    println!("Installing {} to {:?}", dll_name, dest_dir);

    std::fs::create_dir_all(&dest_dir).context("Failed to create destination directory")?;
    std::fs::copy(&dll_path, dest_dir.join(&dll_name)).context("Failed to copy DLL")?;

    println!("Successfully installed {}", dll_name);
    Ok(())
}

fn clean_build(plugin_name: &str) -> Result<()> {
    println!("Cleaning {}...", plugin_name);
    let status = Command::new("cargo")
        .args(["clean", "-p", plugin_name])
        .status()
        .context("Failed to run cargo clean")?;

    if !status.success() {
        return Err(anyhow!("Cargo clean failed"))
    }
    
    build(plugin_name)
}

fn build(plugin_name: &str) -> Result<()> {
    println!("Building {}...", plugin_name);
    let status = Command::new("cargo")
        .args(["build"])
        .status()
        .context("Failed to run cargo build")?;

    if !status.success() {
        return Err(anyhow!("Build failed"));
    }
    Ok(())
}

fn get_project_info(project_root: &std::path::Path) -> Result<(String, String)> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(project_root.join("Cargo.toml"))
        .exec()
        .context("Failed to get cargo metadata")?;

    let root_package = metadata
        .root_package()
        .context("Failed to find root package in metadata")?;

    let plugin_name = root_package.name.clone();

    let lib_name = root_package
        .targets
        .iter()
        .find(|t| t.kind.iter().any(|k| format!("{:?}", k).to_lowercase() == "cdylib"))
        .map(|t| t.name.clone())
        .unwrap_or_else(|| plugin_name.replace('-', "_"));

    Ok((plugin_name, lib_name))
}

fn get_project_root() -> Result<PathBuf> {
    let mut path = env::current_dir()?;
    while !path.join("Cargo.toml").exists() {
        if let Some(parent) = path.parent() {
            path = parent.to_path_buf();
        } else {
            return Err(anyhow!("Could not find project root"));
        }
    }
    Ok(path)
}
