//! NNUE (Efficiently Updatable Neural Network) evaluation.
//!
//! Provides neural network based position evaluation with:
//! - Incremental accumulator updates for efficiency
//! - SIMD-optimized inference (AVX2/NEON)
//! - `SCReLU` activation function
//!
//! Architecture: (768 -> 256) x 2 perspectives -> 1

pub mod network;
pub mod simd;

pub use network::{NnueNetwork, NnueAccumulator, HIDDEN_SIZE};

/// Weight quantization factor for feature weights
pub const QA: i32 = 255;

/// Output weight quantization factor
pub const QB: i32 = 64;

/// Evaluation scale factor
pub const SCALE: i32 = 400;
