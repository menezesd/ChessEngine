mod go;
mod parse;

pub use go::{parse_go_params, GoParams};
pub use parse::parse_uci_command;

#[derive(Debug, Clone)]
pub enum UciCommand {
    Uci,
    IsReady,
    UciNewGame,
    Position(String),
    Go(GoParams),
    Eval,
    EvalFeatures,
    Perft(usize),
    SetOption { name: String, value: Option<String> },
    Debug(Option<String>),
    Stop,
    PonderHit,
    Quit,
    Unknown(String),
}
