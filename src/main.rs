use clap::{Parser};
use colored::*;
use cron::Schedule;
use chrono::{Local};
use log::{info, warn, error};
use indicatif::{ProgressBar, ProgressStyle};
use std::{thread, process::{Command as ProcessCommand, Stdio}};
use std::io::Write;
use std::str::FromStr;
use std::time::{Duration};
use env_logger::{Builder, Env};

#[derive(Parser)]
#[command(name = "Cron Job Runner")]
struct Cli {
    /// Commands to execute
    #[arg(required = true, last = true)]
    command: Vec<String>,

    /// Cron expression to schedule the job
    #[arg(short = 't', long = "cron")]
    cron: String,

    /// Suppress output
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Exit on error
    #[arg(short = 'x', long = "exit-on-error")]
    exit_on_error: bool,

    /// Ignore these exit codes when --exit-on-error is enabled (comma separated)
    #[arg(short = 'c', long = "ignored-codes", use_value_delimiter = true)]
    ignored_codes: Vec<i32>,

    /// Disable color output
    #[arg(short = 'b', long = "no-color")]
    no_color: bool,

    /// Enable stdin, disabled by default
    #[arg(short = 'i', long = "enable-stdin")]
    enable_stdin: bool,

    /// Redirect command stderr to stdout
    #[arg(short = 'r', long = "stderr-to-stdout")]
    stderr_to_stdout: bool,

    /// Disable command output
    #[arg(short = 's', long = "no-output")]
    no_output: bool,
}

fn main() {
    let cli = Cli::parse();
    let mut builder = Builder::from_env(Env::default().default_filter_or(if cli.quiet { "error" } else { "info" }));
    builder.format(move |buf, record| {
        let level = record.level();
        let message = if cli.no_color {
            format!("{}: {}", level, record.args())
        } else {
            format!("{}: {}", level.to_string().color(match level {
                log::Level::Info => "green",
                log::Level::Warn => "yellow",
                log::Level::Error => "red",
                _ => "white",
            }), record.args())
        };
        writeln!(buf, "{}", message)
    });
    builder.init();

    let schedule = Schedule::from_str(&cli.cron).expect("Failed to parse cron expression");

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner()
        .tick_strings(&["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"])
        .template("{spinner:.green} {msg}").expect("Failed to set spinner style"));

    loop {
        let next_run = schedule.upcoming(Local).next().unwrap();
        let now = Local::now();

        if next_run <= now {
            spinner.set_message("Next run is in the past, checking again...".to_string());
            warn!("Next run is in the past: {:?}", next_run);
            continue;
        }

        spinner.set_message(format!("Next run at {:?}", next_run));

        while Local::now() < next_run {
            spinner.tick();
            thread::sleep(Duration::from_millis(100)); // Update every 100 milliseconds
        }

        spinner.set_message("Running job...".to_string());
        let mut command_proc = ProcessCommand::new(&cli.command[0]);
        command_proc.args(&cli.command[1..]);

        if cli.enable_stdin {
            command_proc.stdin(Stdio::inherit());
        } else {
            command_proc.stdin(Stdio::null());
        }

        if cli.no_output {
            command_proc.stdout(Stdio::null());
        } else {
            command_proc.stdout(Stdio::inherit());
        }

        if cli.stderr_to_stdout {
            command_proc.stderr(Stdio::inherit());
        } else {
            command_proc.stderr(Stdio::piped());
        }

        match command_proc.status() {
            Ok(status) if status.success() => info!("Command exited with status {}", status),
            Ok(status) => {
                spinner.set_message(format!("Error: Command exited with status {}", status));
                if cli.exit_on_error && !cli.ignored_codes.contains(&status.code().unwrap_or_default()) {
                    std::process::exit(status.code().unwrap_or_default());
                }
            },
            Err(e) => error!("Failed to execute command: {}", e),
        }
    }
}
