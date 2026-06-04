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

fn assign_numeric_param(params: &mut GoParams, name: &str, value: Option<&str>) -> bool {
    match name {
        "wtime" => assign_if_parsed(&mut params.wtime, value),
        "btime" => assign_if_parsed(&mut params.btime, value),
        "winc" => assign_if_parsed(&mut params.winc, value),
        "binc" => assign_if_parsed(&mut params.binc, value),
        "movetime" => assign_if_parsed(&mut params.movetime, value),
        "movestogo" => assign_if_parsed(&mut params.movestogo, value),
        "nodes" => assign_if_parsed(&mut params.nodes, value),
        "depth" => assign_if_parsed(&mut params.depth, value),
        "mate" => assign_if_parsed(&mut params.mate, value),
        _ => false,
    }
}

#[must_use]
pub fn parse_go_params(parts: &[&str]) -> GoParams {
    let mut params = GoParams::default();
    let mut i = 1;

    while i < parts.len() {
        let value = parts.get(i + 1).copied();
        let consumed = if assign_numeric_param(&mut params, parts[i], value) {
            2
        } else {
            match parts[i] {
                "ponder" => {
                    params.ponder = true;
                    1
                }
                "infinite" => {
                    params.infinite = true;
                    1
                }
                _ => 1,
            }
        };
        i += consumed;
    }
    params
}

#[cfg(test)]
mod tests;
