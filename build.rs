fn main() {
    // Native Cocoa build - no special build steps needed
    println!("cargo:rerun-if-changed=src/");
}
