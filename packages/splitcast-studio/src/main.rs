mod model;
mod network;
mod ui;
mod wire;

use std::{io, path::PathBuf, process::ExitCode};

use crate::{network::Workers, wire::Endpoint};

slint::include_modules!();

const USAGE: &str = "Usage: splitcast-studio --token-file PATH --port PORT [--agent-port PORT]";

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (mut path, mut port, mut agent_port) = (None, None, None);
    let mut trace = false;
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, USAGE);
    while let Some(flag) = args.next() {
        let number = |arg: Option<std::ffi::OsString>| {
            arg?.to_str()?.parse::<u16>().ok().filter(|p| *p != 0)
        };
        match flag.to_str() {
            Some("--help" | "-h") => {
                println!("{USAGE}");
                return Ok(());
            }
            Some("--token-file") if path.is_none() => {
                path = Some(PathBuf::from(args.next().ok_or_else(invalid)?))
            }
            Some("--port") if port.is_none() => {
                port = Some(number(args.next()).ok_or_else(invalid)?)
            }
            Some("--agent-port") if agent_port.is_none() => {
                agent_port = Some(number(args.next()).ok_or_else(invalid)?)
            }
            Some("--trace-render") if !trace => trace = true,
            _ => return Err(invalid().into()),
        }
    }
    let port = port.ok_or_else(invalid)?;
    let endpoint = Endpoint::new(port, agent_port.unwrap_or(port), &path.ok_or_else(invalid)?)?;
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Err(io::Error::other("SplitCast Studio requires a Linux display").into());
    }

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()?;
    let view = StudioWindow::new()?;
    let (workers, network, results) = Workers::start(endpoint)?;
    let result = ui::run(&view, network, results, trace);
    drop(view);
    drop(workers);
    result?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
