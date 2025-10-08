use std::io::{self, BufRead, Write};

pub mod protocol;
pub mod orchestrator;
pub mod info;

use protocol::parse_uci_command;
use orchestrator::SearchOrchestrator;

/// Run the UCI main loop reading stdin and responding to UCI commands.
///
/// This function blocks and is intended to be the program entrypoint when
/// running the engine via UCI.
pub fn run_uci_loop() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // Create UCI info channel
    let (info_tx, info_rx) = crate::uci::info::channel();

    // Spawn printer thread that serializes all info output to stdout
    let _printer_handle = std::thread::spawn(move || {
        let stdout = std::io::stdout();
        while let Ok(info) = info_rx.recv() {
            let line = info.to_uci_line();
            let mut lock = stdout.lock();
            if let Err(e) = writeln!(lock, "{}", line) {
                eprintln!("UCI printer write error: {}", e);
            }
            if let Err(e) = lock.flush() {
                eprintln!("UCI printer flush error: {}", e);
            }
        }
    });

    // Create search orchestrator
    let mut orchestrator = SearchOrchestrator::new(info_tx);

    // Main UCI loop
    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error reading stdin for UCI: {}", e);
                continue;
            }
        };

        // Parse command
        if let Some(command) = parse_uci_command(&line) {
            // Handle command and get responses
            let responses = orchestrator.handle_command(&command);

            // Output responses
            for response in responses {
                println!("{}", response.to_uci_string());
            }

            // Handle quit
            if matches!(command, protocol::UciCommand::Quit) {
                break;
            }
        }

        stdout.flush().unwrap();
    }
}

// Re-export for backward compatibility
pub use protocol::{parse_position_command, format_uci_move};
