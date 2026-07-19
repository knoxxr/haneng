fn main() {
    // 리소스 컴파일러(rc.exe)는 Windows에서만 있다 — 크로스체크는 건너뛴다.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::consts::OS == "windows"
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/haneng.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=아이콘 리소스 컴파일 실패: {e}");
        }
    }
}
