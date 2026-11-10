//! `cargo run -p shaderc` writes the SPIR-V files next to the renderer.

use std::path::Path;

fn main() -> std::process::ExitCode {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/sage_platform/shaders");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("{}: {e}", dir.display());
        return std::process::ExitCode::FAILURE;
    }
    for (name, bytes) in shaderc::all() {
        let path = dir.join(name);
        if let Err(e) = std::fs::write(&path, &bytes) {
            eprintln!("{}: {e}", path.display());
            return std::process::ExitCode::FAILURE;
        }
        println!("{name}: {} bytes", bytes.len());
    }
    std::process::ExitCode::SUCCESS
}
