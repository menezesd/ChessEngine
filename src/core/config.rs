//! Configuration module for chess engine parameters
//!
//! This module centralizes all configurable constants and parameters used throughout
//! the chess engine, including evaluation weights, search parameters, and game rules.

/// Piece values and evaluation parameters
/// Piece values and evaluation parameters
pub mod evaluation {
    // use crate::core::constants::{PAWN_INDEX, KNIGHT_INDEX, BISHOP_INDEX, ROOK_INDEX, QUEEN_INDEX, KING_INDEX};

    /// Material values for middle game
    pub const MATERIAL_MG: [i32; crate::core::board::PieceIndex::count()] = [93, 402, 407, 589, 1250, 0];

    /// Material values for end game
    pub const MATERIAL_EG: [i32; crate::core::board::PieceIndex::count()] = [104, 345, 375, 645, 1240, 0];

    /// King value (used for mate scoring)
    pub const KING_VALUE: i32 = 20000;

    /// Mate score
    pub const MATE_SCORE: i32 = KING_VALUE * 10;

    /// Pawn evaluation constants
    pub const DOUBLED_PAWN_MG: i32 = -9;
    pub const DOUBLED_PAWN_EG: i32 = -9;
    pub const ISOLATED_PAWN_MG: i32 = -10;
    pub const ISOLATED_PAWN_EG: i32 = -18;
    pub const ISOLATED_OPEN_MG: i32 = -9;
    pub const ISOLATED_OPEN_EG: i32 = 0;
    pub const BACKWARD_PAWN_MG: i32 = -2;
    pub const BACKWARD_PAWN_EG: i32 = -1;
    pub const BACKWARD_OPEN_MG: i32 = -6;
    pub const BACKWARD_OPEN_EG: i32 = 0;

    // Pawn island penalties
    pub const PAWN_ISLAND_PENALTY_MG: i32 = -20;
    pub const PAWN_ISLAND_PENALTY_EG: i32 = -30;

    /// Piece mobility bonuses
    pub const KNIGHT_MOB: [i32; 9] = [-28, -6, -3, -2, 16, 17, 17, 20, 25];
    pub const BISHOP_MOB: [i32; 15] = [-30, -29, -23, -11, -5, 2, 8, 12, 20, 19, 23, 28, 35, 44, 40];
    pub const ROOK_MOB: [i32; 15] = [-14, -12, -13, -11, -8, -6, -8, -3, 2, 2, 7, 14, 17, 17, 10];
    pub const QUEEN_MOB: [i32; 28] = [
        -14, -13, -12, -30, -28, -17, -11, -2, -6, 0, 3, 1, 1, 1, 4, 6, 5, 0, 3, 2, 7, 3, 6, 2, 15, 21,
        12, 14,
    ];

    /// King safety penalty tables
    pub const KING_ATTACK_PEN_MG: [i32; 21] = [
        0, 4, 8, 12, 18, 26, 36, 48, 62, 78, 96, 116, 138, 162, 188, 216, 246, 278, 312, 348, 386,
    ];
    pub const KING_ATTACK_PEN_EG: [i32; 21] = [
        0, 2, 4, 6, 9, 13, 18, 24, 31, 39, 48, 58, 69, 81, 94, 108, 123, 139, 156, 174, 193,
    ];

    /// King safety evaluation parameters
    pub const KING_ATTACK_WEIGHTS: [i32; crate::core::board::PieceIndex::count()] = [0, 2, 2, 3, 3, 0]; // Pawn, Knight, Bishop, Rook, Queen, King
    pub const KING_DIST_MULT: [i32; 5] = [0, 7, 6, 4, 2]; // Distance multipliers for king attacks
    pub const KING_ATTACK_CAPS: [i32; crate::core::board::PieceIndex::count()] = [0, 9, 5, 7, 15, 0]; // Caps per piece type

    /// King shield missing penalties
    pub const SHIELD_ONE_PAWN_MISSING_MG: i32 = 15;
    pub const SHIELD_ONE_PAWN_MISSING_EG: i32 = 5;
    pub const SHIELD_TWO_PAWNS_MISSING_MG: i32 = 30;
    pub const SHIELD_TWO_PAWNS_MISSING_EG: i32 = 10;
    pub const SHIELD_THREE_PAWNS_MISSING_MG: i32 = 50;
    pub const SHIELD_THREE_PAWNS_MISSING_EG: i32 = 20;

    /// Additional evaluation bonuses
    pub const TEMPO: i32 = 5; // Tempo bonus for side to move
    pub const RFP_MARGIN: i32 = 100; // Reverse Futility Pruning margin
    pub const BISHOP_PAIR_MG: i32 = 40;
    pub const BISHOP_PAIR_EG: i32 = 60;
    pub const ROOK_HALF_OPEN_MG: i32 = 12;
    pub const ROOK_HALF_OPEN_EG: i32 = 12;
    pub const ROOK_OPEN_MG: i32 = 18;
    pub const ROOK_OPEN_EG: i32 = 18;
    pub const ROOK_7TH_MG: i32 = 12;
    pub const ROOK_7TH_EG: i32 = 30;
    pub const KING_PSEUDO_SHIELD: i32 = 8;
    pub const KING_OPEN_FILE_PENALTY: i32 = -8;
    pub const KING_NEAR_OPEN_PENALTY: i32 = -6;
    pub const MINOR_ON_MINOR: i32 = 15;
    pub const TRAPPED_ROOK: i32 = -25;
    pub const CONNECTED_PAWN_BONUS: i32 = 10;
    pub const PAWN_CHAIN_BONUS: i32 = 15;

    /// Piece-square tables for middle game
    pub const PST_MG: [[i32; 64]; 6] = [
        // Pawn PST MG
        [
            93, 93, 93, 93, 93, 93, 94, 93,
            83, 79, 79, 75, 80, 103, 103, 73,
            80, 79, 80, 85, 89, 92, 97, 77,
            82, 85, 88, 96, 103, 95, 85, 77,
            95, 100, 100, 104, 119, 113, 103, 86,
            106, 110, 116, 109, 117, 138, 118, 91,
            125, 127, 128, 118, 116, 122, 95, 97,
            93, 93, 93, 93, 93, 93, 93, 93,
        ],
        // Knight PST MG
        [
            -50, -40, -30, -30, -30, -30, -40, -50, -40, -20, 0, 0, 0, 0, -20, -40, -30, 0, 10, 15, 15,
            10, 0, -30, -30, 5, 15, 20, 20, 15, 5, -30, -30, 0, 15, 20, 20, 15, 0, -30, -30, 5, 10, 15,
            15, 10, 5, -30, -40, -20, 0, 5, 5, 0, -20, -40, -50, -40, -30, -30, -30, -30, -40, -50,
        ],
        // Bishop PST MG
        [
            404, 406, 393, 391, 390, 396, 379, 402,
            410, 408, 413, 398, 405, 413, 430, 413,
            403, 408, 410, 409, 411, 414, 409, 416,
            386, 409, 412, 420, 420, 411, 410, 388,
            385, 410, 409, 436, 421, 421, 410, 392,
            393, 400, 425, 418, 422, 418, 427, 400,
            397, 417, 396, 397, 400, 422, 407, 407,
            377, 378, 369, 382, 384, 373, 378, 356,
        ],
        // Rook PST MG
        [
            584, 585, 594, 599, 601, 596, 588, 582,
            559, 573, 580, 591, 588, 599, 593, 558,
            553, 567, 577, 588, 588, 591, 579, 569,
            575, 588, 597, 598, 598, 597, 588, 575,
            584, 589, 591, 596, 596, 591, 589, 584,
            594, 601, 604, 607, 607, 604, 601, 594,
            584, 589, 591, 596, 596, 591, 589, 584,
            580, 587, 591, 593, 593, 591, 587, 580,
        ],
        // Queen PST MG
        [
            1258, 1239, 1246, 1255, 1254, 1241, 1227, 1240,
            1242, 1250, 1259, 1260, 1254, 1254, 1263, 1243,
            1226, 1239, 1240, 1242, 1244, 1247, 1253, 1226,
            1240, 1243, 1265, 1261, 1261, 1265, 1243, 1268,
            1224, 1234, 1239, 1255, 1253, 1253, 1239, 1225,
            1219, 1232, 1237, 1243, 1243, 1237, 1232, 1219,
            1217, 1221, 1226, 1231, 1231, 1226, 1221, 1217,
            1209, 1218, 1223, 1225, 1222, 1223, 1218, 1209,
        ],
        // King PST MG
        [
            -65, 23, 16, -15, -56, -34, 2, 13,
            29, -1, -20, -7, -8, -4, -38, -29,
            -9, 24, 2, -16, -20, 6, 22, -22,
            -17, -20, -12, -27, -30, -25, -14, -36,
            -49, -1, -27, -39, -46, -44, -33, -51,
            -14, -14, -22, -46, -44, -30, -15, -27,
            1, 7, -8, -64, -43, -16, 9, 8,
            -15, 22, 4, -23, -39, -1, 11, 7,
        ],
    ];

    /// Piece-square tables for end game
    pub const PST_EG: [[i32; 64]; 6] = [
        // Pawn PST EG
        [
            104, 104, 104, 104, 104, 104, 104, 104,
            101, 100, 114, 116, 116, 114, 100, 101,
            101, 104, 103, 104, 104, 103, 104, 101,
            106, 106, 99, 90, 90, 99, 106, 106,
            123, 115, 106, 91, 91, 106, 115, 123,
            150, 144, 126, 109, 109, 126, 144, 150,
            156, 167, 147, 136, 136, 147, 167, 156,
            104, 104, 104, 104, 104, 104, 104, 104,
        ],
        // Knight PST EG
        [
            303, 306, 335, 335, 335, 335, 306, 303,
            332, 348, 349, 352, 352, 349, 348, 332,
            327, 356, 385, 385, 393, 385, 356, 327,
            334, 369, 385, 388, 393, 385, 369, 334,
            343, 379, 384, 397, 397, 384, 379, 343,
            405, 429, 433, 459, 459, 433, 429, 405,
            352, 384, 406, 393, 393, 406, 384, 352,
            261, 319, 326, 318, 318, 326, 319, 261,
        ],
        // Bishop PST EG
        [
            360, 374, 361, 368, 368, 361, 374, 360,
            356, 360, 368, 373, 373, 368, 360, 356,
            373, 382, 381, 389, 389, 381, 382, 373,
            374, 385, 393, 389, 389, 393, 385, 374,
            385, 402, 393, 391, 391, 393, 402, 385,
            390, 395, 384, 380, 380, 384, 395, 390,
            375, 385, 387, 387, 387, 387, 385, 375,
            373, 381, 371, 381, 381, 371, 381, 373,
        ],
        // Rook PST EG
        [
            618, 631, 627, 624, 624, 627, 631, 618,
            623, 630, 630, 624, 624, 630, 630, 623,
            632, 635, 630, 629, 629, 630, 635, 632,
            644, 638, 644, 644, 644, 644, 638, 644,
            651, 652, 656, 652, 652, 656, 652, 651,
            650, 667, 649, 653, 653, 649, 667, 650,
            640, 649, 650, 650, 650, 650, 649, 640,
            630, 663, 666, 664, 664, 666, 663, 630,
        ],
        // Queen PST EG
        [
            1171, 1241, 1249, 1246, 1247, 1243, 1246, 1246,
            1232, 1246, 1268, 1260, 1258, 1240, 1252, 1252,
            1230, 1260, 1275, 1269, 1269, 1245, 1249, 1241,
            1244, 1256, 1246, 1248, 1248, 1260, 1280, 1257,
            1257, 1273, 1262, 1272, 1264, 1270, 1264, 1267,
            1240, 1240, 1253, 1251, 1237, 1239, 1242, 1251,
            1236, 1234, 1238, 1238, 1227, 1228, 1237, 1231,
            1190, 1239, 1213, 1210, 1215, 1226, 1253, 1238,
        ],
        // King PST EG
        [
            -53, -34, -21, -11, -28, -14, -24, -43, -27, -11, 4, 13, 14, 4, 4, -5, -3, -5, -4, -7, -7, -8, -3,
            -1, 9, -5, -11, -8, -6, -6, -3, 0, 21, -15, 3, 22, 22, 5, 11, 8, 25, 30, 4, 39, 29, 43, 37, 27,
            36, 58, 39, 50, 37, 55, 50, 11, 63, 56, 65, 67, 55, 62, 34, 60,
        ],
    ];

    /// Pawn support pattern bonuses
    pub const P_SUPPORT: [i32; 64] = [
        0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 1, -1, 2, 5, 6, 4, 3, 0, 2, 3, 4, 2, 4, 6, 1, 1, 6, 17, 10, 5, 1,
        1, 3, 4, 8, 14, 15, 9, 4, 3, 7, 9, 10, 12, 12, 10, 9, 7, 9, 10, 12, 14, 14, 12, 10, 9, 0, 0, 0,
        0, 0, 0, 0, 0,
    ];

    /// Pawn piece-square tables (separate from main PST for evaluation)
    pub const PUB_PAWN_PST_MG: [i32; 64] = [
        0, 0, 0, 0, 0, 0, 0, 0, -35, -1, -20, -23, -15, 24, 38, -22, -26, -4, -4, -10, 3, 3, 33,
        -12, -27, -2, -5, 12, 17, 6, 10, -25, -14, 13, 6, 21, 23, 12, 17, -23, -6, 7, 26, 31, 65,
        56, 25, -20, 98, 134, 61, 95, 68, 126, 34, -11, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    pub const PUB_PAWN_PST_EG: [i32; 64] = [
        0, 0, 0, 0, 0, 0, 0, 0, -3, -4, 10, 12, 12, 10, -4, -3, -3, 0, -1, 0, 0, -1, 0, -3, 2, 2, -5,
        -14, -14, -5, 2, 2, 19, 11, 2, -13, -13, 2, 11, -7, 46, 40, 22, 5, 5, 22, 40, -2, 52, 63, 43,
        32, 32, 43, 63, 52, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    /// Passed pawn bonuses by rank (mg, eg)
    pub const PASSED_PAWN_BONUS: [(i32, i32); 8] = [
        (0, 0), (5, 10), (10, 20), (20, 40), (40, 80), (80, 160), (160, 320), (0, 0)
    ];

    /// Passed pawn push bonuses by rank (mg, eg)
    pub const PASSED_PAWN_PUSH_BONUS: [(i32, i32); 8] = [
        (0, 0), (0, 0), (5, 10), (10, 20), (20, 40), (40, 80), (80, 160), (0, 0)
    ];
}

/// Game rules and board configuration
pub mod game {
    /// Standard chess board size (8x8)
    pub const BOARD_SIZE: usize = 8;

    /// Number of colors (White, Black)
    pub const NUM_COLORS: usize = crate::core::board::ColorIndex::count();

    /// Number of piece types (Pawn, Knight, Bishop, Rook, Queen, King)
    pub const NUM_PIECES: usize = crate::core::board::PieceIndex::count();

    /// Starting ranks for pieces
    pub const WHITE_START_RANK: usize = 0;
    pub const BLACK_START_RANK: usize = 7;

    /// Starting ranks for pawns
    pub const WHITE_PAWN_RANK: usize = 1;
    pub const BLACK_PAWN_RANK: usize = 6;

    /// Castling file positions
    pub const KINGSIDE_ROOK_FILE: usize = 7;
    pub const QUEENSIDE_ROOK_FILE: usize = 0;
    pub const KINGSIDE_KING_FILE: usize = 6;
    pub const QUEENSIDE_KING_FILE: usize = 2;
    pub const KINGSIDE_ROOK_DEST_FILE: usize = 5;
    pub const QUEENSIDE_ROOK_DEST_FILE: usize = 3;
}

/// Search algorithm parameters
pub mod search {
    use std::time::Duration;

    /// Safety margin for time management (milliseconds)
    pub const SAFETY_MARGIN_MS: u64 = 5;

    /// Time growth factor between depths
    pub const TIME_GROWTH_FACTOR: f32 = 2.0;

    /// Safety margin as Duration
    pub const SAFETY_MARGIN: Duration = Duration::from_millis(SAFETY_MARGIN_MS);

    // Late Move Pruning thresholds
    pub const LMP_DEPTH_THRESHOLD: u32 = 6;
    pub const LMP_MOVE_INDEX_THRESHOLD: usize = 4;
    // Singular Extension parameters
    pub const SINGULAR_EXTENSION_MIN_DEPTH: u32 = 8;
    pub const SINGULAR_EXTENSION_MARGIN: i32 = 50; // Centipawns
    pub const SINGULAR_EXTENSION_VERIFICATION_REDUCTION: u32 = 3;

    // Quiescence Search parameters
    pub const QS_FUTILITY_MARGIN: i32 = 100; // Centipawns (e.g., 1 Pawn)
    pub const QS_SEE_PRUNING_MARGIN: i32 = -50; // Centipawns (e.g., lose more than 0.5 pawn)

    // History Pruning parameters
    pub const HISTORY_PRUNING_THRESHOLD: i32 = 500;
}
