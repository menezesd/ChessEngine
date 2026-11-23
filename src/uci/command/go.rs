#[derive(Default, Debug, Clone)]
pub struct GoParams {
    pub wtime: Option<u64>,
    pub btime: Option<u64>,
    pub winc: Option<u64>,
    pub binc: Option<u64>,
    pub movetime: Option<u64>,
    pub movestogo: Option<u64>,
    pub depth: Option<u32>,
    pub nodes: Option<u64>,
    pub mate: Option<u32>,
    pub searchmoves: Option<Vec<String>>,
    pub ponder: bool,
    pub infinite: bool,
}

fn parsed_value<T: std::str::FromStr>(value: Option<&str>) -> Option<T> {
    value.and_then(|v| v.parse::<T>().ok())
}

fn assign_if_parsed<T: std::str::FromStr>(target: &mut Option<T>, value: Option<&str>) -> bool {
    let Some(parsed) = parsed_value(value) else {
        return false;
    };

    *target = Some(parsed);
    true
}

/// Parse a clock value, clamping negatives to zero.
///
/// GUIs report negative remaining time when the engine is already over
/// budget. Keep that as an explicit zero clock instead of dropping the
/// timing information entirely.
fn assign_clamped_clock(target: &mut Option<u64>, value: Option<&str>) -> bool {
    let Some(parsed) = parsed_value::<i64>(value) else {
        return false;
    };

    *target = Some(u64::try_from(parsed.max(0)).unwrap_or_default());
    true
}

fn assign_numeric_param(params: &mut GoParams, name: &str, value: Option<&str>) -> bool {
    match name {
        "wtime" => assign_clamped_clock(&mut params.wtime, value),
        "btime" => assign_clamped_clock(&mut params.btime, value),
        "winc" => assign_clamped_clock(&mut params.winc, value),
        "binc" => assign_clamped_clock(&mut params.binc, value),
        "movetime" => assign_if_parsed(&mut params.movetime, value),
        "movestogo" => assign_if_parsed(&mut params.movestogo, value),
        "nodes" => assign_if_parsed(&mut params.nodes, value),
        "depth" => assign_if_parsed(&mut params.depth, value),
        "mate" => assign_if_parsed(&mut params.mate, value),
        _ => false,
    }
}

fn is_go_keyword(token: &str) -> bool {
    matches!(
        token,
        "wtime"
            | "btime"
            | "winc"
            | "binc"
            | "movetime"
            | "movestogo"
            | "nodes"
            | "depth"
            | "mate"
            | "ponder"
            | "infinite"
            | "searchmoves"
    )
}

#[must_use]
pub fn parse_go_params(parts: &[&str]) -> GoParams {
    parse_go_tokens(parts.iter().skip(1).copied())
}

pub(super) fn parse_go_tokens<'a>(tokens: impl Iterator<Item = &'a str>) -> GoParams {
    let mut params = GoParams::default();
    let mut tokens = tokens.peekable();

    while let Some(token) = tokens.next() {
        match token {
            "searchmoves" => {
                let mut moves = Vec::new();
                while let Some(mv) = tokens.next_if(|mv| !is_go_keyword(mv)) {
                    moves.push(mv.to_owned());
                }
                params.searchmoves = Some(moves);
            }
            "ponder" => params.ponder = true,
            "infinite" => params.infinite = true,
            _ => {
                if assign_numeric_param(&mut params, token, tokens.peek().copied()) {
                    tokens.next();
                }
            }
        }
    }
    params
}

#[cfg(test)]
mod tests;
