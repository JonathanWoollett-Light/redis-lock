//! Build script.
/// Re-runs compilation is the lua script changes.
fn main() {
    println!("cargo::rerun-if-changed=src/functions.lua");
}
