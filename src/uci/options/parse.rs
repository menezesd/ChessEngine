fn next_word<'a>(line: &mut &'a str) -> Option<&'a str> {
    let rest = line.trim_start();
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let (word, remaining) = rest.split_at(end);
    *line = remaining;
    (!word.is_empty()).then_some(word)
}

#[must_use]
pub fn parse_setoption(parts: &[&str]) -> Option<(String, Option<String>)> {
    parse_setoption_line(&parts.join(" "))
}

pub(in crate::uci) fn parse_setoption_line(mut line: &str) -> Option<(String, Option<String>)> {
    if next_word(&mut line)? != "setoption" {
        return None;
    }
    while next_word(&mut line)? != "name" {}

    let mut name = String::new();
    while let Some(word) = next_word(&mut line) {
        if word == "value" {
            // Filenames can include multiple consecutive spaces or tabs.
            return (!name.is_empty()).then(|| (name, Some(line.trim().to_owned())));
        }
        if !name.is_empty() {
            name.push(' ');
        }
        name.push_str(word);
    }
    (!name.is_empty()).then_some((name, None))
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
