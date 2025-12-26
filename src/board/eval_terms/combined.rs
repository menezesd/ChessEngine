//! Combined evaluation functions.
//!
//! Functions that combine multiple evaluation terms to share attack computation.

use crate::board::state::Board;

use super::helpers::AttackContext;

impl Board {
    /// Combined evaluation of passed pawns and hanging pieces with shared attack computation.
    /// Returns `(pass_mg, pass_eg, hanging)`.
    #[must_use]
    pub fn eval_attacks_dependent(&self) -> (i32, i32, i32) {
        let ctx = self.compute_attack_context();
        self.eval_attacks_dependent_with_context(&ctx)
    }

    /// Combined evaluation using pre-computed attack context.
    #[must_use]
    pub fn eval_attacks_dependent_with_context(&self, ctx: &AttackContext) -> (i32, i32, i32) {
        let (pass_mg, pass_eg) =
            self.eval_passed_pawns_with_attacks(ctx.white_attacks, ctx.black_attacks);
        let hanging = self.eval_hanging_with_attacks(ctx.white_attacks, ctx.black_attacks);

        (pass_mg, pass_eg, hanging)
    }
}
