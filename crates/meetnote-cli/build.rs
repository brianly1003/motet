fn main() {
    // Library link arguments do not propagate to the final executable.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
