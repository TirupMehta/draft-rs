#[cfg(windows)]
fn main() {
    let mut resource = winres::WindowsResource::new();
    resource.set_icon("../../assets/draft.ico");
    resource.set("ProductName", "Draft");
    resource.set("FileDescription", "Draft - local dictation");
    resource.set("CompanyName", "Draft");
    resource.set("LegalCopyright", "MIT licensed");
    resource
        .compile()
        .expect("could not compile the Windows icon resource");
}

#[cfg(not(windows))]
fn main() {}
