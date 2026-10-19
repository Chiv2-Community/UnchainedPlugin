use clap::Parser;
use std::io::{self, BufRead};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;
use UnchainedPlugin::features::rcon::transport::{load_psk, Session};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:9001")]
    address: SocketAddr,
    #[arg(long)]
    psk_file: PathBuf,
    command: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let key = load_psk(&args.psk_file)?;
    let mut stream = TcpStream::connect_timeout(&args.address, Duration::from_secs(10))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut session = Session::connect(&mut stream, &key)?;
    if let Some(command) = args.command {
        session.write_command(&mut stream, &command)?;
    } else {
        for command in io::stdin().lock().lines() {
            session.write_command(&mut stream, &command?)?;
        }
    }
    Ok(())
}
