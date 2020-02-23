use std::process::Command;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=src/elm-auth/src/Main.elm");
    println!("cargo:rerun-if-changed=src/elm-auth/src/elm.json");
    println!("cargo:rerun-if-changed=src/elm-auth/index.html");

    let input_path = Path::new("src/Main.elm");
    let dest_path = Path::new("index.html");
    let out = Command::new("elm")
        .current_dir(Path::new("src/elm-auth"))
        .arg("make")
        .arg(input_path)
        .arg("--output")
        .arg(dest_path)
        .arg("--optimize")
        .output().unwrap();

    if !out.status.success() {
        println!("{}", String::from_utf8_lossy(&out.stderr));
        std::process::exit(1);
    }
}
