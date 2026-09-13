pub(crate) fn rounded_bucket_count(
    requested_size: usize,
    unit_bytes: usize,
    bucket_size: usize,
    default_buckets: usize,
) -> usize {
    let requested_bytes = requested_size.saturating_mul(unit_bytes);
    let requested_buckets = requested_bytes / bucket_size;

    if requested_buckets == 0 {
        return default_buckets;
    }

    // Round down, retaining exact powers of two. Halving next_power_of_two
    // also halves every ordinary power-of-two Hash setting.
    1usize << (usize::BITS - 1 - requested_buckets.leading_zeros())
}

#[cfg(test)]
mod tests {
    use super::rounded_bucket_count;

    const UNIT_BYTES: usize = 1024;
    const BUCKET_SIZE: usize = 64;
    const DEFAULT_BUCKETS: usize = 1024;

    #[test]
    fn uses_default_for_zero_size() {
        assert_eq!(
            rounded_bucket_count(0, UNIT_BYTES, BUCKET_SIZE, DEFAULT_BUCKETS),
            DEFAULT_BUCKETS
        );
    }

    #[test]
    fn uses_default_when_request_rounds_to_zero_buckets() {
        assert_eq!(
            rounded_bucket_count(1, 1, BUCKET_SIZE, DEFAULT_BUCKETS),
            DEFAULT_BUCKETS
        );
    }

    #[test]
    fn exact_power_of_two_uses_the_requested_capacity() {
        let requested_buckets = UNIT_BYTES / BUCKET_SIZE;

        assert_eq!(
            rounded_bucket_count(1, UNIT_BYTES, BUCKET_SIZE, DEFAULT_BUCKETS),
            requested_buckets
        );
    }

    #[test]
    fn rounds_down_only_when_the_bucket_count_is_not_a_power_of_two() {
        for requested in 1..=1024 {
            let count = rounded_bucket_count(requested, 1, 1, DEFAULT_BUCKETS);
            assert!(count.is_power_of_two());
            assert!(
                count <= requested,
                "{count} buckets exceed requested {requested}"
            );
            assert!(
                count * 2 > requested,
                "{count} wastes half of requested {requested}"
            );
        }
    }

    #[test]
    fn saturates_extreme_size_arithmetic() {
        let count = rounded_bucket_count(usize::MAX, UNIT_BYTES, BUCKET_SIZE, DEFAULT_BUCKETS);

        assert!(count.is_power_of_two());
        assert!(count >= DEFAULT_BUCKETS);
    }
}
