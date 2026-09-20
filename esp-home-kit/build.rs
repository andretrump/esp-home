use std::{env, fs, path::Path};

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set, is this being run by cargo?");
    let out_dir = Path::new(&out_dir);
    let html = fs::read_to_string("./src/captive_portal/web/index.html")
        .expect("failed to read src/captive_portal/web/index.html");
    let css = fs::read_to_string("./src/captive_portal/web/style.css")
        .expect("failed to read src/captive_portal/web/style.css");
    let js = fs::read_to_string("./src/captive_portal/web/script.js")
        .expect("failed to read src/captive_portal/web/script.js");

    let combined = html
        .replace(
            r#"<link rel="stylesheet" href="style.css">"#,
            &format!("<style>{css}</style>"),
        )
        .replace(
            r#"<script src="script.js"></script>"#,
            &format!("<script>{js}</script>"),
        );

    let minified = minify_html::minify(
        combined.as_bytes(),
        &minify_html::Cfg {
            minify_css: true,
            minify_js: true,
            ..Default::default()
        },
    );

    assert!(
        !minified.is_empty(),
        "minify_html produced empty output | check that src/captive_portal/web/index.html is well-formed"
    );

    fs::write(out_dir.join("index.min.html"), &minified).unwrap();

    println!("cargo:rerun-if-changed=./src/captive_portal/web/index.html");
    println!("cargo:rerun-if-changed=./src/captive_portal/web/style.css");
    println!("cargo:rerun-if-changed=./src/captive_portal/web/script.js");
}
