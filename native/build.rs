fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("the Slint UI must compile");

    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        resource.set("ProductName", "Aurora Launcher");
        resource.set("OriginalFilename", "aurora-launcher.exe");
        resource.set("InternalName", "aurora-launcher");
        resource.set("FileDescription", "Aurora Launcher");
        #[cfg(feature = "native-update-acceptance")]
        {
            println!("cargo:rerun-if-env-changed=AURORA_NATIVE_ACCEPTANCE_VERSION");
            let version =
                std::env::var("AURORA_NATIVE_ACCEPTANCE_VERSION").expect("acceptance version");
            let parts: Vec<u64> = version.split('.').map(|v| v.parse().unwrap()).collect();
            assert!(parts.len() == 3 && parts.iter().all(|v| *v <= 65535));
            let numeric = (parts[0] << 48) | (parts[1] << 32) | (parts[2] << 16);
            resource.set_version_info(winresource::VersionInfo::PRODUCTVERSION, numeric);
            resource.set_version_info(winresource::VersionInfo::FILEVERSION, numeric);
            resource.set("ProductVersion", &version);
            resource.set("FileVersion", &version);
            resource.set("ProductName", "Aurora LOCAL UPDATE ACCEPTANCE ONLY");
        }
        resource
            .compile()
            .expect("the Windows resource must compile");
    }
}
