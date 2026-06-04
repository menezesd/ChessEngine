//! XBoard/WinBoard protocol command parsing.

/// `XBoard` protocol commands
#[derive(Debug, Clone)]
pub enum XBoardCommand {
    /// Enter `XBoard` mode
    XBoard,
    /// Protocol version negotiation
    Protover(u32),
    /// Acknowledge features accepted
    Accepted(String),
    /// Acknowledge features rejected
    Rejected(String),
    /// Start new game
    New,
    /// Set position from FEN
    SetBoard(String),
    /// Opponent's move (in SAN or coordinate notation)
    UserMove(String),
    /// Start thinking
    Go,
    /// Enter force mode (make moves without thinking)
    Force,
    /// Play the color to move
    PlayOther,
    /// Set computer's color
    White,
    Black,
    /// Time remaining for engine (centiseconds)
    Time(u64),
    /// Time remaining for opponent (centiseconds)
    OTime(u64),
    /// Set time control: level <mps> <base> <inc>
    Level {
        moves_per_session: u32,
        base_seconds: u32,
        increment_seconds: u32,
    },
    /// Set exact seconds per move
    St(u32),
    /// Set max depth
    Sd(u32),
    /// Move immediately
    MoveNow,
    /// Ping/pong for keepalive
    Ping(u32),
    /// Undo last move
    Undo,
    /// Remove last two half-moves (one for each side)
    Remove,
    /// Game result
    Result(String),
    /// Hint request
    Hint,
    /// Opponent offers a draw
    Draw,
    /// Set board (edit mode commands)
    Edit,
    /// Exit edit mode
    EditDone,
    /// Clear board in edit mode
    ClearBoard,
    /// Place piece in edit mode (e.g., "Pa2")
    EditPiece(String),
    /// Set side to move in edit mode
    EditColor(char),
    /// Computer plays itself
    Computer,
    /// Set random mode
    Random,
    /// Post thinking output
    Post,
    /// Don't post thinking output
    NoPost,
    /// Enable pondering
    Hard,
    /// Disable pondering
    Easy,
    /// Set opponent's name
    Name(String),
    /// Set memory/hash size in MB
    Memory(u32),
    /// Set number of cores
    Cores(u32),
    /// Analyze mode
    Analyze,
    /// Exit analyze mode
    ExitAnalyze,
    /// Pause thinking
    Pause,
    /// Resume thinking
    Resume,
    /// Quit the program
    Quit,
    /// Unknown command
    Unknown(String),
}

mod parse;

pub use parse::parse_xboard_command;
