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
        let message = format!(
            "Kitter 启动失败。\n\n错误详情已写入：\n{}\n\n{}",
            log_path.display(),
            panic
        );
        show_windows_error(&message);
    }));
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
