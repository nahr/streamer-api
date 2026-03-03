use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    // target/$PROFILE is 3 levels up from OUT_DIR (out -> build/hash -> build -> $PROFILE)
    let target_profile = Path::new(&out_dir)
        .ancestors()
        .nth(3)
        .expect("OUT_DIR should have target/profile ancestor");

    let ui_dir = Path::new(&manifest_dir).join("../ui");
    let ui_dist_src = ui_dir.join("dist");
    let ui_dist_dst = target_profile.join("ui-dist");

    if !ui_dir.exists() {
        eprintln!("cargo:warning=ui directory not found at {:?}, skipping UI build", ui_dir);
        return;
    }

    println!("cargo:rerun-if-changed={}", ui_dir.display());

    // Run npm run build in ui/
    let status = Command::new("npm")
        .args(["run", "build"])
        .current_dir(&ui_dir)
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            panic!("npm run build failed with exit code: {:?}", s.code());
        }
        Err(e) => {
            panic!("failed to run npm: {}. Ensure Node.js and npm are installed.", e);
        }
    }

    if !ui_dist_src.exists() {
        panic!("UI build succeeded but dist directory not found at {:?}", ui_dist_src);
    }

    // Copy dist contents to target/$PROFILE/ui-dist
    if ui_dist_dst.exists() {
        fs::remove_dir_all(&ui_dist_dst).expect("failed to remove existing ui-dist");
    }
    copy_dir_all(&ui_dist_src, &ui_dist_dst).expect("failed to copy UI static files");
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}
