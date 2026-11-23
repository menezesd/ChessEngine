use std::collections::{hash_map::Entry, HashMap};

use super::indexing::palette_width;

/// A block offset points to a palette count, the signed distances, and
/// the packed palette indices. Identical blocks share one offset.
pub(super) fn distances(values: &[i8], block_size: usize) -> Vec<u8> {
    assert_eq!(values.len() % block_size, 0);
    assert_eq!(block_size % 8, 0);
    let mut packed = vec![0; values.len() / block_size * 4];
    let mut offsets = HashMap::new();

    for (index, block) in values.chunks_exact(block_size).enumerate() {
        let offset = match offsets.entry(block) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let offset = u32::try_from(packed.len()).unwrap();
                entry.insert(offset);
                let header = packed.len();
                packed.push(0);
                let mut indices = [u8::MAX; 256];
                for &value in block {
                    let code = value as u8;
                    let index = &mut indices[usize::from(code)];
                    if *index == u8::MAX {
                        *index = u8::try_from(packed.len() - header - 1).unwrap();
                        packed.push(code);
                    }
                }
                let count = u8::try_from(packed.len() - header - 1).unwrap();
                packed[header] = count;
                let width = palette_width(count);
                let mut pending = 0u16;
                let mut bits = 0;
                for &value in block {
                    pending |= u16::from(indices[usize::from(value as u8)]) << bits;
                    bits += width;
                    if bits >= 8 {
                        packed.push(pending as u8);
                        pending >>= 8;
                        bits -= 8;
                    }
                }
                offset
            }
        };
        packed[index * 4..][..4].copy_from_slice(&offset.to_le_bytes());
    }

    // Random access to an index may read one byte past the final block.
    packed.push(0);
    packed
}
