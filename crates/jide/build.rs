fn main() {
    println!("cargo:rerun-if-changed=assets/jide.manifest");
    println!("cargo:rerun-if-changed=../../assets/icons/jide.ico");

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/icons/jide.ico");
        res.set_manifest_file("assets/jide.manifest");
        res.set("ProductName", "Just IDE");
        res.set("FileDescription", "Just IDE — Native Offline Code Editor");
        if let Err(err) = res.compile() {
            println!("cargo:warning=winres resource compilation skipped: {err}");
        }
    }
}
