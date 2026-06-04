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

    let rounded_buckets = requested_buckets
        .checked_next_power_of_two()
        .map_or(usize::MAX / 2 + 1, |count| count / 2);

    if rounded_buckets == 0 {
        default_buckets
    } else {
        rounded_buckets
    }
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
    fn matches_existing_rounding_for_normal_size() {
        let requested_buckets = UNIT_BYTES / BUCKET_SIZE;
        let expected = requested_buckets.next_power_of_two() / 2;

        assert_eq!(
            rounded_bucket_count(1, UNIT_BYTES, BUCKET_SIZE, DEFAULT_BUCKETS),
            expected
        );
    }

    #[test]
    fn saturates_extreme_size_arithmetic() {
        let count = rounded_bucket_count(usize::MAX, UNIT_BYTES, BUCKET_SIZE, DEFAULT_BUCKETS);

        assert!(count.is_power_of_two());
        assert!(count >= DEFAULT_BUCKETS);
    }
}
