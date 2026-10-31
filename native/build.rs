fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("the Slint UI must compile");

    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=../src-tauri/icons/icon.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../src-tauri/icons/icon.ico");
        resource.set("ProductName", "Aurora Launcher (native preview)");
        resource.set("FileDescription", "Aurora Launcher");
        resource
            .compile()
            .expect("the Windows resource must compile");
    }
}
