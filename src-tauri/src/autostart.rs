use std::io;

const APP_NAME: &str = "BlurAutoClicker";

pub fn get_autostart_enabled() -> bool {
    if crate::portable::is_portable() {
        return false;
    }
    #[cfg(target_os = "windows")]
    {
        get_registry_autostart()
    }
    #[cfg(target_os = "linux")]
    {
        autostart_file()
            .as_ref()
            .is_some_and(|path| path.exists())
    }
}

pub fn set_autostart_enabled(enabled: bool) -> io::Result<()> {
    if crate::portable::is_portable() {
        log::info!("[Autostart] Skipping registry write in portable mode");
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        set_registry_autostart(enabled)
    }
    #[cfg(target_os = "linux")]
    {
        set_xdg_autostart(enabled)
    }
}

// ---------------------------------------------------------------------------
// Windows: HKCU\...\CurrentVersion\Run
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
const RUN_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";

#[cfg(target_os = "windows")]
fn get_registry_autostart() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(run_key) = hkcu.open_subkey(RUN_KEY) else {
        return false;
    };
    run_key.get_value::<String, _>(APP_NAME).is_ok()
}

#[cfg(target_os = "windows")]
fn set_registry_autostart(enabled: bool) -> io::Result<()> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_WRITE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu.open_subkey_with_flags(RUN_KEY, KEY_WRITE)?;

    if enabled {
        let exe_path = std::env::current_exe()?;
        let value = format!("\"{}\" --autostart", exe_path.display());
        run_key.set_value(APP_NAME, &value)?;
    } else {
        let _ = run_key.delete_value(APP_NAME);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Linux: XDG autostart .desktop file in ~/.config/autostart
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn autostart_file() -> Option<std::path::PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| std::path::Path::new(&home).join(".config"))
        })?;
    Some(
        config_home
            .join("autostart")
            .join(format!("{}.desktop", APP_NAME)),
    )
}

#[cfg(target_os = "linux")]
fn set_xdg_autostart(enabled: bool) -> io::Result<()> {
    let Some(path) = autostart_file() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine autostart location",
        ));
    };

    if enabled {
        let exe_path = std::env::current_exe()?;
        let contents = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name={APP_NAME}\n\
             Exec=\"{}\" --autostart\n\
             Terminal=false\n\
             X-GNOME-Autostart-enabled=true\n",
            exe_path.display()
        );
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
    } else {
        let _ = std::fs::remove_file(&path);
    }

    Ok(())
}