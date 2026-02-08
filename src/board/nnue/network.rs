//! NNUE network structure and evaluation.
//!
//! Implements a 768 -> 256 -> 1 architecture with:
//! - Dual perspective accumulators (white/black view)
//! - Incremental updates for efficiency
//! - `SCReLU` activation

use super::simd;
use super::{QA, QB, SCALE};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Input feature size: 64 squares × 6 piece types × 2 colors
pub const INPUT_SIZE: usize = 768;

/// Hidden layer size (must match trained network)
pub const HIDDEN_SIZE: usize = 256;

/// NNUE accumulator storing hidden layer activations for both perspectives
#[derive(Clone)]
pub struct NnueAccumulator {
    /// White's perspective accumulator
    pub white: [i16; HIDDEN_SIZE],
    /// Black's perspective accumulator
    pub black: [i16; HIDDEN_SIZE],
}

impl Default for NnueAccumulator {
    fn default() -> Self {
        Self {
            white: [0; HIDDEN_SIZE],
            black: [0; HIDDEN_SIZE],
        }
    }
}

impl NnueAccumulator {
    /// Create a new accumulator initialized with biases
    #[must_use]
    pub fn new(biases: &[i16; HIDDEN_SIZE]) -> Self {
        Self {
            white: *biases,
            black: *biases,
        }
    }

    /// Refresh accumulator from scratch given active features
    pub fn refresh(
        &mut self,
        white_features: &[usize],
        black_features: &[usize],
        network: &NnueNetwork,
    ) {
        // Start with biases
        self.white = network.feature_bias;
        self.black = network.feature_bias;

        // Add active feature weights
        for &feat in white_features {
            simd::add_weights(&mut self.white, &network.feature_weights[feat]);
        }
        for &feat in black_features {
            simd::add_weights(&mut self.black, &network.feature_weights[feat]);
        }
    }

    /// Add a feature (piece placed on square)
    #[inline]
    pub fn add_feature(&mut self, white_feat: usize, black_feat: usize, network: &NnueNetwork) {
        simd::add_weights(&mut self.white, &network.feature_weights[white_feat]);
        simd::add_weights(&mut self.black, &network.feature_weights[black_feat]);
    }

    /// Remove a feature (piece removed from square)
    #[inline]
    pub fn sub_feature(&mut self, white_feat: usize, black_feat: usize, network: &NnueNetwork) {
        simd::sub_weights(&mut self.white, &network.feature_weights[white_feat]);
        simd::sub_weights(&mut self.black, &network.feature_weights[black_feat]);
    }
}

/// NNUE network weights
pub struct NnueNetwork {
    /// Feature transformer weights `[INPUT_SIZE][HIDDEN_SIZE]`
    pub feature_weights: Box<[[i16; HIDDEN_SIZE]; INPUT_SIZE]>,
    /// Feature transformer biases `[HIDDEN_SIZE]`
    pub feature_bias: [i16; HIDDEN_SIZE],
    /// Output weights for white perspective `[HIDDEN_SIZE]`
    pub output_weights_white: [i16; HIDDEN_SIZE],
    /// Output weights for black perspective `[HIDDEN_SIZE]`
    pub output_weights_black: [i16; HIDDEN_SIZE],
    /// Output bias
    pub output_bias: i16,
}

impl NnueNetwork {
    /// Load network from a .nnue file
    pub fn load<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Read feature weights
        let mut feature_weights = Box::new([[0i16; HIDDEN_SIZE]; INPUT_SIZE]);
        for i in 0..INPUT_SIZE {
            for j in 0..HIDDEN_SIZE {
                let mut buf = [0u8; 2];
                reader.read_exact(&mut buf)?;
                feature_weights[i][j] = i16::from_le_bytes(buf);
            }
        }

        // Read feature biases
        let mut feature_bias = [0i16; HIDDEN_SIZE];
        for elem in &mut feature_bias {
            let mut buf = [0u8; 2];
            reader.read_exact(&mut buf)?;
            *elem = i16::from_le_bytes(buf);
        }

        // Read output weights (white perspective)
        let mut output_weights_white = [0i16; HIDDEN_SIZE];
        for elem in &mut output_weights_white {
            let mut buf = [0u8; 2];
            reader.read_exact(&mut buf)?;
            *elem = i16::from_le_bytes(buf);
        }

        // Read output weights (black perspective)
        let mut output_weights_black = [0i16; HIDDEN_SIZE];
        for elem in &mut output_weights_black {
            let mut buf = [0u8; 2];
            reader.read_exact(&mut buf)?;
            *elem = i16::from_le_bytes(buf);
        }

        // Read output bias
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf)?;
        let output_bias = i16::from_le_bytes(buf);

        Ok(Self {
            feature_weights,
            feature_bias,
            output_weights_white,
            output_weights_black,
            output_bias,
        })
    }

    /// Evaluate position given accumulator and side to move
    /// Returns evaluation in centipawns from side-to-move perspective
    #[inline]
    #[must_use]
    pub fn evaluate(&self, acc: &NnueAccumulator, white_to_move: bool) -> i32 {
        let (us_acc, them_acc, us_weights, them_weights) = if white_to_move {
            (
                &acc.white,
                &acc.black,
                &self.output_weights_white,
                &self.output_weights_black,
            )
        } else {
            (
                &acc.black,
                &acc.white,
                &self.output_weights_black,
                &self.output_weights_white,
            )
        };

        // SCReLU activation and dot product
        let us_output = simd::screlu_dot(us_acc, us_weights);
        let them_output = simd::screlu_dot(them_acc, them_weights);

        // Combine outputs and scale
        let output = us_output + them_output + i32::from(self.output_bias) * QA;

        // Scale to centipawns
        output * SCALE / (QA * QA * QB)
    }
}

/// Compute feature index for a piece at a square from a perspective
#[inline]
#[must_use]
pub fn feature_index(
    piece_type: usize,
    piece_color: usize,
    square: usize,
    perspective: usize,
) -> usize {
    let (oriented_sq, oriented_color) = if perspective == 1 {
        // Black's perspective - flip board vertically
        (square ^ 56, 1 - piece_color)
    } else {
        // White's perspective
        (square, piece_color)
    };
    oriented_color * 384 + piece_type * 64 + oriented_sq
}

/// Embedded default network (compiled into the binary)
#[cfg(feature = "embedded_nnue")]
pub static EMBEDDED_NETWORK: &[u8] = include_bytes!("nets/default.nnue");

#[cfg(feature = "embedded_nnue")]
impl NnueNetwork {
    /// Load network from embedded bytes
    #[must_use]
    pub fn from_embedded() -> Self {
        Self::from_bytes(EMBEDDED_NETWORK).expect("Embedded NNUE is invalid")
    }

    /// Load network from byte slice
    pub fn from_bytes(data: &[u8]) -> std::io::Result<Self> {
        use std::io::Cursor;
        let mut reader = Cursor::new(data);
        Self::from_reader(&mut reader)
    }

    /// Load network from any reader
    fn from_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        // Read feature weights
        let mut feature_weights = Box::new([[0i16; HIDDEN_SIZE]; INPUT_SIZE]);
        for i in 0..INPUT_SIZE {
            for j in 0..HIDDEN_SIZE {
                let mut buf = [0u8; 2];
                reader.read_exact(&mut buf)?;
                feature_weights[i][j] = i16::from_le_bytes(buf);
            }
        }

        // Read feature biases
        let mut feature_bias = [0i16; HIDDEN_SIZE];
        for elem in &mut feature_bias {
            let mut buf = [0u8; 2];
            reader.read_exact(&mut buf)?;
            *elem = i16::from_le_bytes(buf);
        }

        // Read output weights (white perspective)
        let mut output_weights_white = [0i16; HIDDEN_SIZE];
        for elem in &mut output_weights_white {
            let mut buf = [0u8; 2];
            reader.read_exact(&mut buf)?;
            *elem = i16::from_le_bytes(buf);
        }

        // Read output weights (black perspective)
        let mut output_weights_black = [0i16; HIDDEN_SIZE];
        for elem in &mut output_weights_black {
            let mut buf = [0u8; 2];
            reader.read_exact(&mut buf)?;
            *elem = i16::from_le_bytes(buf);
        }

        // Read output bias
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf)?;
        let output_bias = i16::from_le_bytes(buf);

        Ok(Self {
            feature_weights,
            feature_bias,
            output_weights_white,
            output_weights_black,
            output_bias,
        })
    }
}
