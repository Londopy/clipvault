fn main() {
    // bake the version string from Cargo.toml into the binary so we can use env!("CLIPVAULT_VERSION")
    println!(
        "cargo:rustc-env=CLIPVAULT_VERSION={}",
        env!("CARGO_PKG_VERSION")
    );

    // on windows we need to embed the icon and use the windows subsystem
    // so no ugly console window pops up when users run it
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().expect("failed to compile Windows resources");
    }
}
