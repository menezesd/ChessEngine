/// Print a UCI spin option.
pub(super) fn print_spin(
    name: &str,
    default: impl std::fmt::Display,
    min: impl std::fmt::Display,
    max: impl std::fmt::Display,
) {
    println!("option name {name} type spin default {default} min {min} max {max}");
}

/// Print a UCI check option.
pub(super) fn print_check(name: &str, default: bool) {
    println!(
        "option name {name} type check default {}",
        if default { "true" } else { "false" }
    );
}

pub(super) fn print_string(name: &str, default: &str) {
    println!("option name {name} type string default {default}");
}
