#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "windows")]
    install_windows_panic_reporter();
    kitter::ui::run();
}

#[cfg(target_os = "windows")]
fn install_windows_panic_reporter() {
    std::panic::set_hook(Box::new(|panic| {
        use std::{fs, fs::OpenOptions, io::Write as _};

        let data_dir = kitter::config::app_data_dir();
        let log_path = data_dir.join("crash.log");
        let _ = fs::create_dir_all(&data_dir);
        if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(&log_path) {
            let _ = writeln!(log, "{panic}");
        }
        let language = kitter::config::AppConfig::load()
            .map(|config| config.language)
            .unwrap_or_default();
        show_windows_error(crash_dialog_message(language));
    }));
}

#[cfg(any(target_os = "windows", test))]
fn crash_dialog_message(language: kitter::config::Language) -> &'static str {
    use kitter::config::Language;

    let uses_chinese = language == Language::ZhCn
        || (language == Language::System && Language::system() == Language::ZhCn);
    if uses_chinese {
        "Kitter 遇到问题。请重新打开后再试。"
    } else {
        "Kitter encountered a problem. Please reopen it and try again."
    }
}

#[cfg(target_os = "windows")]
fn show_windows_error(message: &str) {
    use std::{ffi::c_void, ptr};

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            window: *mut c_void,
            text: *const u16,
            caption: *const u16,
            kind: u32,
        ) -> i32;
    }

    let text = message
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let caption = "Kitter"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    const MB_OK: u32 = 0;
    const MB_ICONERROR: u32 = 0x10;
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(test)]
mod tests {
    use kitter::config::Language;

    #[test]
    fn crash_dialog_is_localized_without_exposing_diagnostics() {
        let chinese = super::crash_dialog_message(Language::ZhCn);
        let english = super::crash_dialog_message(Language::En);

        assert_eq!(chinese, "Kitter 遇到问题。请重新打开后再试。");
        assert_eq!(
            english,
            "Kitter encountered a problem. Please reopen it and try again."
        );
        for message in [chinese, english] {
            assert!(!message.contains("crash.log"));
            assert!(!message.contains("panic"));
            assert!(!message.contains(':'));
        }
    }
}
