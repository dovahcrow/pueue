use std::fs::create_dir_all;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Clap;
use simplelog::{Config, LevelFilter, SimpleLogger};

use pueue_lib::network::certificate::create_certificates;
use pueue_lib::network::message::Message;
use pueue_lib::network::protocol::socket_cleanup;
use pueue_lib::network::secret::init_shared_secret;
use pueue_lib::settings::Settings;
use pueue_lib::state::State;

use crate::cli::CliArguments;
use crate::network::socket::accept_incoming;
use crate::task_handler::TaskHandler;

mod cli;
mod network;
mod platform;
mod task_handler;

#[async_std::main]
async fn main() -> Result<()> {
    // Parse commandline options.
    let opt = CliArguments::parse();

    if opt.daemonize {
        return fork_daemon(&opt);
    }

    // Set the verbosity level of the logger.
    let level = match opt.verbose {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };
    SimpleLogger::init(level, Config::default()).unwrap();

    // Try to read settings from the configuration file.
    let settings = match Settings::read(false, &opt.config) {
        Ok(settings) => settings,
        Err(_) => {
            // There's something wrong with the config file or something's missing.
            // Try to read the config and fill missing values with defaults.
            // This might be possible on version upgrade or first run.
            let settings = Settings::new(false, &opt.config)?;

            // Since we needed to add values to the configuration, we have to save it.
            // This also creates the save file in case it didn't exist yet.
            if let Err(error) = settings.save(&opt.config) {
                println!("Failed saving config file.");
                println!("{:?}", error);
            }
            settings
        }
    };

    init_directories(&settings.shared.pueue_directory);
    if !settings.shared.daemon_key.exists() && !settings.shared.daemon_cert.exists() {
        create_certificates(&settings)?;
    }
    init_shared_secret(&settings.shared.shared_secret_path)?;

    let mut state = State::new(&settings, opt.config.clone());
    // Restore the previous state and save any changes that might have happened during this process
    state.restore();
    state.save();
    let state = Arc::new(Mutex::new(state));

    let (sender, receiver) = channel();
    let mut task_handler = TaskHandler::new(state.clone(), receiver);

    // This section handles Shutdown via SigTerm/SigInt process signals
    // 1. Remove the unix socket (if it exists).
    // 2. Notify the TaskHandler, so it can shutdown gracefully.
    //
    // The actual program exit will be done via the TaskHandler.
    let sender_clone = sender.clone();
    let settings_clone = settings.clone();
    ctrlc::set_handler(move || {
        socket_cleanup(&settings_clone.shared);

        // Notify the task handler
        sender_clone
            .send(Message::DaemonShutdown)
            .expect("Failed to send Message to TaskHandler on Shutdown");
    })?;

    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        std::process::exit(1);
    }));

    std::thread::spawn(move || {
        task_handler.run();
    });

    accept_incoming(sender, state.clone()).await?;

    Ok(())
}

/// Initialize all directories needed for normal operation.
fn init_directories(pueue_dir: &Path) {
    // Pueue base path
    if !pueue_dir.exists() {
        if let Err(error) = create_dir_all(&pueue_dir) {
            panic!(
                "Failed to create main directory at {:?} error: {:?}",
                pueue_dir, error
            );
        }
    }

    // Task log dir
    let log_dir = pueue_dir.join("log");
    if !log_dir.exists() {
        if let Err(error) = create_dir_all(&log_dir) {
            panic!(
                "Failed to create log directory at {:?} error: {:?}",
                log_dir, error
            );
        }
    }

    // Task certs dir
    let certs_dir = pueue_dir.join("certs");
    if !certs_dir.exists() {
        if let Err(error) = create_dir_all(&certs_dir) {
            panic!(
                "Failed to create certificate directory at {:?} error: {:?}",
                certs_dir, error
            );
        }
    }

    // Task log dir
    let logs_dir = pueue_dir.join("task_logs");
    if !logs_dir.exists() {
        if let Err(error) = create_dir_all(&logs_dir) {
            panic!(
                "Failed to create task logs directory at {:?} error: {:?}",
                logs_dir, error
            );
        }
    }
}

/// This is a simple and cheap custom fork method.
/// Simply spawn a new child with identical arguments and exit right away.
fn fork_daemon(opt: &CliArguments) -> Result<()> {
    let mut arguments = Vec::<String>::new();

    if let Some(config) = &opt.config {
        arguments.push("--config".to_string());
        arguments.push(config.to_string_lossy().into_owned());
    }

    if opt.verbose > 0 {
        arguments.push("-".to_string() + &" ".repeat(opt.verbose as usize));
    }

    Command::new("pueued").args(&arguments).spawn()?;

    println!("Pueued is now running in the background");
    Ok(())
}
