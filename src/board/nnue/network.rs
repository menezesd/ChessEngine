//! NNUE network structure and evaluation.
//!
//! Implements a 768 -> 256 -> 1 architecture with:
//! - Dual perspective accumulators (white/black view)
//! - Incremental updates for efficiency
//! - `SCReLU` activation

use super::simd;
use super::{QA, QB, SCALE};
use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::path::Path;

const VERTICAL_FLIP_MASK: usize = 0b11_1000;
const BLACK_PERSPECTIVE: usize = 1;
const NETWORK_MAGIC: &[u8; 8] = b"RNQNNUE\0";
const NETWORK_VERSION: u32 = 1;

/// Input feature size: 64 squares x 6 piece types x 2 colors.
pub const INPUT_SIZE: usize = 64 * 6 * 2;

/// Hidden layer size (must match trained network)
pub const HIDDEN_SIZE: usize = 256;
const RAW_NETWORK_BYTES: usize =
    INPUT_SIZE * HIDDEN_SIZE * 2 + HIDDEN_SIZE * 2 + HIDDEN_SIZE * 2 + HIDDEN_SIZE * 2 + 2;
const HEADER_BYTES: usize = NETWORK_MAGIC.len() + 4 + 4 + 4;

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
    pub feature_weights: Vec<[i16; HIDDEN_SIZE]>,
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
        let mut file = File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Self::from_bytes(&data)
    }

    fn parse_payload(data: &[u8]) -> std::io::Result<&[u8]> {
        if data.starts_with(NETWORK_MAGIC) {
            if data.len() != HEADER_BYTES + RAW_NETWORK_BYTES {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "headered NNUE file has incorrect size",
                ));
            }

            let version = u32::from_le_bytes(data[8..12].try_into().unwrap());
            let input_size = u32::from_le_bytes(data[12..16].try_into().unwrap());
            let hidden_size = u32::from_le_bytes(data[16..20].try_into().unwrap());
            if version != NETWORK_VERSION {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "unsupported NNUE file version",
                ));
            }
            if input_size as usize != INPUT_SIZE || hidden_size as usize != HIDDEN_SIZE {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "NNUE architecture does not match engine",
                ));
            }

            return Ok(&data[HEADER_BYTES..]);
        }

        if data.len() == RAW_NETWORK_BYTES {
            Ok(data)
        } else {
            Err(Error::new(
                ErrorKind::InvalidData,
                "raw NNUE file has incorrect size",
            ))
        }
    }

    /// Load network from raw or headered bytes.
    pub fn from_bytes(data: &[u8]) -> std::io::Result<Self> {
        fn read_i16(data: &[u8], offset: &mut usize) -> i16 {
            let value = i16::from_le_bytes(data[*offset..*offset + 2].try_into().unwrap());
            *offset += 2;
            value
        }

        let payload = Self::parse_payload(data)?;
        let mut offset = 0;

        // Read feature weights
        let mut feature_weights = vec![[0i16; HIDDEN_SIZE]; INPUT_SIZE];
        for row in feature_weights.iter_mut().take(INPUT_SIZE) {
            for weight in row.iter_mut().take(HIDDEN_SIZE) {
                *weight = read_i16(payload, &mut offset);
            }
        }

        // Read feature biases
        let mut feature_bias = [0i16; HIDDEN_SIZE];
        for elem in &mut feature_bias {
            *elem = read_i16(payload, &mut offset);
        }

        // Read output weights (white perspective)
        let mut output_weights_white = [0i16; HIDDEN_SIZE];
        for elem in &mut output_weights_white {
            *elem = read_i16(payload, &mut offset);
        }

        // Read output weights (black perspective)
        let mut output_weights_black = [0i16; HIDDEN_SIZE];
        for elem in &mut output_weights_black {
            *elem = read_i16(payload, &mut offset);
        }

        // Read output bias
        let output_bias = read_i16(payload, &mut offset);

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

        // SCReLU activation and dot product (returns i64 to avoid overflow)
        let us_output = simd::screlu_dot(us_acc, us_weights);
        let them_output = simd::screlu_dot(them_acc, them_weights);

        // Combine outputs and scale
        let output = us_output + them_output + i64::from(self.output_bias) * QA as i64;

        // Scale to centipawns
        (output * SCALE as i64 / (QA as i64 * QA as i64 * QB as i64)) as i32
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
    let (oriented_sq, oriented_color) = if perspective == BLACK_PERSPECTIVE {
        // Black's perspective - flip board vertically
        (square ^ VERTICAL_FLIP_MASK, 1 - piece_color)
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
}

#[cfg(test)]
mod tests {
    use super::{feature_index, HIDDEN_SIZE, INPUT_SIZE, NETWORK_MAGIC, RAW_NETWORK_BYTES};
    use super::{NnueNetwork, NETWORK_VERSION};

    #[test]
    fn input_size_matches_feature_layout() {
        assert_eq!(INPUT_SIZE, 64 * 6 * 2);
    }

    #[test]
    fn feature_index_uses_white_perspective_layout() {
        assert_eq!(feature_index(2, 1, 10, 0), 384 + 2 * 64 + 10);
    }

    #[test]
    fn feature_index_flips_square_and_color_for_black_perspective() {
        assert_eq!(feature_index(2, 1, 10, 1), 2 * 64 + (0b00_1010 ^ 0b11_1000));
    }

    #[test]
    fn exact_raw_network_size_loads_for_legacy_files() {
        let data = vec![0; RAW_NETWORK_BYTES];
        assert!(NnueNetwork::from_bytes(&data).is_ok());
    }

    #[test]
    fn oversized_raw_network_is_rejected() {
        let data = vec![0; RAW_NETWORK_BYTES + 2];
        assert!(NnueNetwork::from_bytes(&data).is_err());
    }

    #[test]
    fn headered_network_rejects_wrong_architecture() {
        let mut data = Vec::new();
        data.extend_from_slice(NETWORK_MAGIC);
        data.extend_from_slice(&NETWORK_VERSION.to_le_bytes());
        data.extend_from_slice(&((INPUT_SIZE + 1) as u32).to_le_bytes());
        data.extend_from_slice(&(HIDDEN_SIZE as u32).to_le_bytes());
        data.resize(data.len() + RAW_NETWORK_BYTES, 0);

        assert!(NnueNetwork::from_bytes(&data).is_err());
    }
}
