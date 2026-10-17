fn main() {
    // screencapturekit links a Swift bridge; point dyld at the OS Swift runtime.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
