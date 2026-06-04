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

    for part in parts.iter().skip(1) {
        match *part {
            "name" => field = SetOptionField::Name,
            "value" => field = SetOptionField::Value,
            _ => match field {
                SetOptionField::Name => name_parts.push(part),
                SetOptionField::Value => value_parts.push(part),
                SetOptionField::None => {}
            },
        }
    }

    if name_parts.is_empty() {
        return None;
    }

    let name = name_parts.join(" ");
    let value = if value_parts.is_empty() {
        None
    } else {
        Some(value_parts.join(" "))
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
    fn rejects_missing_name() {
        let parts = ["setoption", "value", "256"];
        assert_eq!(parse_setoption(&parts), None);
    }
}
