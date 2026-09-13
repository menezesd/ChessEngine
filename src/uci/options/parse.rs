enum SetOptionField {
    None,
    Name,
    Value,
}

#[must_use]
pub fn parse_setoption(parts: &[&str]) -> Option<(String, Option<String>)> {
    if parts.is_empty() || parts[0] != "setoption" {
        return None;
    }

    let mut name_parts: Vec<&str> = Vec::new();
    let mut value_parts: Vec<&str> = Vec::new();
    let mut field = SetOptionField::None;
    let mut saw_value = false;

    for part in parts.iter().skip(1) {
        match field {
            SetOptionField::None => {
                if *part == "name" {
                    field = SetOptionField::Name;
                }
            }
            SetOptionField::Name => {
                if *part == "value" {
                    field = SetOptionField::Value;
                    saw_value = true;
                } else {
                    name_parts.push(part);
                }
            }
            SetOptionField::Value => value_parts.push(part),
        }
    }

    if name_parts.is_empty() {
        return None;
    }

    let name = name_parts.join(" ");
    let value = if saw_value {
        Some(value_parts.join(" "))
    } else {
        None
    };

    Some((name, value))
}

#[cfg(test)]
mod tests {
    use super::parse_setoption;

    #[test]
    fn parses_name_and_value() {
        let parts = ["setoption", "name", "Hash", "value", "256"];
        assert_eq!(
            parse_setoption(&parts),
            Some(("Hash".to_string(), Some("256".to_string())))
        );
    }

    #[test]
    fn parses_multi_word_name_and_value() {
        let parts = ["setoption", "name", "Move", "Overhead", "value", "25"];
        assert_eq!(
            parse_setoption(&parts),
            Some(("Move Overhead".to_string(), Some("25".to_string())))
        );
    }

    #[test]
    fn parses_name_without_value() {
        let parts = ["setoption", "name", "Ponder"];
        assert_eq!(parse_setoption(&parts), Some(("Ponder".to_string(), None)));
    }

    #[test]
    fn parses_explicit_empty_string_value() {
        let parts = ["setoption", "name", "EvalFile", "value"];
        assert_eq!(
            parse_setoption(&parts),
            Some(("EvalFile".to_string(), Some(String::new())))
        );
    }

    #[test]
    fn preserves_keywords_inside_string_value() {
        let parts = [
            "setoption",
            "name",
            "EvalFile",
            "value",
            "/tmp/name",
            "name",
            "value",
            "network.nnue",
        ];
        assert_eq!(
            parse_setoption(&parts),
            Some((
                "EvalFile".to_string(),
                Some("/tmp/name name value network.nnue".to_string())
            ))
        );
    }

    #[test]
    fn rejects_missing_name() {
        let parts = ["setoption", "value", "256"];
        assert_eq!(parse_setoption(&parts), None);
    }
}
