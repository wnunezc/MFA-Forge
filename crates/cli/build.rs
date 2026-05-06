fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../gui/assets/icons/app-icon.ico");
        res.compile()
            .expect("failed to compile Windows icon resource");
    }
}
