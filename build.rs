#[path = "src/board/search/endgame/generate.rs"]
mod endgame;

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/board/search/endgame");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    for (name, data) in endgame::generate() {
        std::fs::write(output.join(name), data)?;
    }
    Ok(())
}
