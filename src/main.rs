fn main() -> Result<(), Box<dyn std::error::Error>> {
    use slint::ComponentHandle;
    let window = bulletnotes::AppWindow::new()?;
    bulletnotes::app::ui::present_settings(
        &window,
        &bulletnotes::store::settings::AppSettings::default(),
        bulletnotes::theme::SystemAppearance::Light,
    )?;
    window.run()?;
    Ok(())
}
