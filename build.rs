fn main() {
    println!("cargo:rerun-if-changed=assets/larik-text.ico");
    println!("cargo:rerun-if-changed=assets/larik-document.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/larik-text.ico");
        // Icon 2 is the document icon used for files Larik Text opens.
        resource.set_icon_with_id("assets/larik-document.ico", "2");
        resource.set("ProductName", "Larik Text");
        resource.set("FileDescription", "Larik Text");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=Could not embed the Windows icon: {error}");
        }
    }
}
