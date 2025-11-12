fn main() {
    #[cfg(windows)]
    {
        // Compile C utilities
        cc::Build::new()
            .file("src/platform/windows_utils.c")
            .compile("windows_utils");

        println!("cargo:rerun-if-changed=src/platform/windows_utils.c");
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=gdi32");

        // Add Windows manifest to fix version detection
        // Without this, Windows 11 may report as Windows 8 due to app compatibility
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("windows_manifest.xml");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Failed to embed manifest: {}", e);
        }
    }
}
