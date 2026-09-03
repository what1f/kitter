#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(any(target_os = "windows", test))]
#[cfg_attr(test, allow(dead_code))]
#[path = "kitter.rs"]
mod cli;

fn main() {
    #[cfg(target_os = "windows")]
    if std::env::args_os().len() > 1 {
        cli::main_entry();
        return;
    }

    kitter::ui::run();
}
