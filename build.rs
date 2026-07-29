use std::path::Path;

fn main() {
    // 确保 frontend/dist/ 存在（开发模式下可能还没 npm run build）
    let dist = Path::new("frontend/dist");
    if !dist.exists() {
        std::fs::create_dir_all(dist).ok();
        std::fs::write(dist.join("index.html"), r#"<!doctype html><html><body><h1>KeyTrace</h1><p>请先运行 npm run build 构建前端</p></body></html>"#).ok();
        println!("cargo:warning=frontend/dist 不存在，已创建占位文件。运行 npm run build 后重新编译即可嵌入完整前端。");
    }
    // 确保 rust-embed 感知到 dist 变化
    println!("cargo:rerun-if-changed=frontend/dist");

    winres::WindowsResource::new()
        .set_icon("keytrace.ico")
        .set_manifest_file("keytrace.exe.manifest")
        .compile()
        .unwrap();
}
