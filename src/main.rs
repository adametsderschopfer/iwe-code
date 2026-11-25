#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cli;
mod domain;
mod i18n;
mod infrastructure;
mod modules;
mod ui;

fn main() -> std::process::ExitCode {
    if infrastructure::mcp::requested() {
        return app::mcp::serve();
    }
    if let Some(code) = infrastructure::updates::startup() {
        return code;
    }
    if let Some(code) = cli::run() {
        return code;
    }
    let i18n = std::rc::Rc::new(i18n::I18n::new(i18n::Language::English));
    infrastructure::graphics::run(&i18n, || app::run(std::rc::Rc::clone(&i18n)))
}
