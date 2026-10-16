fn main() {
    println!("cargo:rerun-if-changed=assets/windows.rc");
    println!("cargo:rerun-if-changed=assets/tickreel.ico");
    embed_resource::compile("assets/windows.rc", embed_resource::NONE)
        .manifest_required()
        .expect("Windows app icon could not be embedded");
}
