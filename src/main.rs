mod config;
mod exec;
mod models;
mod session;
mod ssh_docker;
mod ui;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    app.set_status_text("Disconnected. (bootstrap)".into());
    app.run()
}
