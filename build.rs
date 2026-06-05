use std::env;
use std::fs;

fn main() {
    let out_dir = String::from("C:/ArkData/misc");
    let target_dir = String::from("target");
    let project_name = env::var("CARGO_PKG_NAME").unwrap();

    let mut bin_path = std::path::Path::new(&target_dir)
        .join("release")
        .join(&project_name);

    if cfg!(windows) {
        bin_path.set_extension("exe");
    }

    let dst_dir = std::path::Path::new(&out_dir);

    if let Err(e) = fs::create_dir_all(&dst_dir) {
        println!("Failed to create destination directory: {}", e);
        return;
    }

    let mut dst_path = dst_dir.join("git");

    if cfg!(windows) {
        dst_path.set_extension("exe");
    }

    if let Err(e) = fs::copy(&bin_path, &dst_path) {
        println!("Failed to copy binary: {}", e);
        return;
    }
}
