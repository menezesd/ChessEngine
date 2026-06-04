const EVAL_SCALE_DENOMINATOR: i64 = 100;

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

pub(crate) fn scaled_eval(eval: i32, scale: i32) -> i32 {
    let scaled = i64::from(eval) * i64::from(scale) / EVAL_SCALE_DENOMINATOR;
    clamp_i64_to_i32(scaled)
}

pub(crate) fn blended_eval(nnue_eval: i32, hce_eval: i32, blend: i32) -> i32 {
    let blend = i64::from(blend);
    let hce_weight = EVAL_SCALE_DENOMINATOR - blend;
    let blended =
        (blend * i64::from(nnue_eval) + hce_weight * i64::from(hce_eval)) / EVAL_SCALE_DENOMINATOR;

    clamp_i64_to_i32(blended)
}

#[cfg(test)]
mod tests {
    use super::{blended_eval, scaled_eval};

    #[test]
    fn scaled_eval_scales_by_percent() {
        assert_eq!(scaled_eval(1_000, 130), 1_300);
        assert_eq!(scaled_eval(1_000, 80), 800);
    }

    #[test]
    fn scaled_eval_clamps_extreme_values() {
        assert_eq!(scaled_eval(i32::MAX, i32::MAX), i32::MAX);
        assert_eq!(scaled_eval(i32::MIN, i32::MAX), i32::MIN);
    }

    #[test]
    fn blended_eval_weights_nnue_and_hce() {
        assert_eq!(blended_eval(200, 100, 25), 125);
        assert_eq!(blended_eval(200, 100, 100), 200);
    }

    #[test]
    fn blended_eval_clamps_extreme_values() {
        assert_eq!(blended_eval(i32::MAX, i32::MAX, 100), i32::MAX);
        assert_eq!(blended_eval(i32::MIN, i32::MIN, 100), i32::MIN);
    }
}
