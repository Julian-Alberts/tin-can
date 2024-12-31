fn main() {
    println!("cargo:rustc-link-lib=cap");
    println!("cargo:rustc-link-search=native=/lib");
}
