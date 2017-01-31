extern crate embed_resource;


fn main() {
    embed_resource::compile("cargo-install-update-manifest.rc", Some("cargo-install-update-manifest"), None);
}
