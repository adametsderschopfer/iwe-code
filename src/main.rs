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
    let language = infrastructure::locale::detect();
    let i18n = std::rc::Rc::new(i18n::I18n::new(language));
    infrastructure::graphics::run(&i18n, || match infrastructure::external::startup() {
        infrastructure::external::Startup::Forwarded => Ok(()),
        infrastructure::external::Startup::Primary => app::run(std::rc::Rc::clone(&i18n)),
    })
}
