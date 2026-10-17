fn main() -> Result<(), slint::PlatformError> {
    use slint::ComponentHandle;
    bulletnotes::AppWindow::new()?.run()
}
