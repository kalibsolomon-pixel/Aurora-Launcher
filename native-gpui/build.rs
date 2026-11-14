fn main() {
    #[cfg(windows)]
    {
        // The shared multi-resolution ICO is derived from assets/icon.png.
        // GPUI loads application icon resource 1 for its native window class.
        println!("cargo:rerun-if-changed=../native/assets/icon.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../native/assets/icon.ico");
        resource.set("ProductName", "Aurora Launcher");
        resource.set("OriginalFilename", "aurora-launcher.exe");
        resource.set("InternalName", "aurora-launcher");
        resource.set("FileDescription", "Aurora Launcher");
        resource
            .compile()
            .expect("the Aurora application icon resource must compile");
    }
}
