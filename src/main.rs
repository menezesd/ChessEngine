use std::io;

/// Protocol to use for communication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtocolMode {
    Uci,
    XBoard,
    Auto,
}

fn parse_args() -> ProtocolMode {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--uci" | "-u" => return ProtocolMode::Uci,
            "--xboard" | "-x" => return ProtocolMode::XBoard,
            _ => {}
        }
    }
    ProtocolMode::Auto
}

fn main() {
    let protocol = parse_args();

    match protocol {
        ProtocolMode::Uci => chess_engine::uci::session::run_uci(),
        ProtocolMode::XBoard => chess_engine::xboard::run_xboard(),
        ProtocolMode::Auto => {
            // Auto-detect based on first command
            let stdin = io::stdin();
            let mut first_line = String::new();
            if stdin.read_line(&mut first_line).is_ok() {
                let trimmed = first_line.trim();
                match chess_engine::engine::ProtocolType::detect(trimmed) {
                    chess_engine::engine::ProtocolType::XBoard => {
                        run_xboard_with_first_line(trimmed)
                    }
                    chess_engine::engine::ProtocolType::Uci
                    | chess_engine::engine::ProtocolType::Unknown => {
                        let remaining = stdin.lock();
                        chess_engine::uci::session::run_uci_session(
                            Some(trimmed.to_string()),
                            remaining,
                        );
                    }
                }
            }
        }
    }
}

fn run_xboard_with_first_line(first_line: &str) {
    let mut handler = chess_engine::xboard::XBoardHandler::new();
    if let Some(cmd) = chess_engine::xboard::command::parse_xboard_command(first_line) {
        if let Some(response) = handler.handle_command(&cmd) {
            for line in response.lines() {
                println!("{line}");
            }
        }
    }
    handler.run();
}
