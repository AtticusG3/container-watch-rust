mod config;
mod exec;
mod models;
mod session;
mod ssh_docker;
mod ssh_util;
mod ui;

fn main() -> Result<(), slint::PlatformError> {
    // Force light native widgets (LineEdit, Button, CheckBox) on dark Windows theme.
    std::env::set_var("SLINT_STYLE", "fluent-light");
    ui::run_app()
}
