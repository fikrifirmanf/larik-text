//! Makes the app show up in the system's "Open with" list without a separate installer.
//! macOS declares its file types in Info.plist, so this only does work on Windows and Linux.

/// Registers Larik Text for the current user. Runs once per executable path, off the UI thread.
pub fn register() {
    std::thread::spawn(|| {
        if let Err(error) = platform::register() {
            eprintln!("Couldn’t add Larik Text to Open with: {error:#}");
        }
    });
}

#[cfg(target_os = "linux")]
mod platform {
    use anyhow::{Context as _, Result};
    use std::path::PathBuf;

    const DESKTOP: &str = include_str!("../packaging/linux/larik-text.desktop");
    const ICON: &str = include_str!("../assets/app-icon.svg");

    pub fn register() -> Result<()> {
        let exe = std::env::current_exe()?;
        let exe = exe
            .to_str()
            .context("The executable path isn’t valid UTF-8.")?;
        if exe.contains(['\n', '"']) || exe.contains("/target/") && exe.contains("/debug/") {
            return Ok(());
        }
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::home_dir().map(|home| home.join(".local/share")))
            .context("No data folder.")?;
        let escaped = exe.replace('\\', "\\\\").replace(' ', "\\s");
        let desktop = DESKTOP.replace("Exec=larik-text", &format!("Exec={escaped}"));
        let applications = data.join("applications");
        let entry = applications.join("larik-text.desktop");
        let icon = data.join("icons/hicolor/scalable/apps/larik-text.svg");
        let mut changed = false;
        for (path, content) in [(&entry, desktop.as_str()), (&icon, ICON)] {
            if std::fs::read_to_string(path).is_ok_and(|current| current == content) {
                continue;
            }
            std::fs::create_dir_all(path.parent().context("No parent folder.")?)?;
            std::fs::write(path, content)?;
            changed = true;
        }
        if changed {
            let _ = std::process::Command::new("update-desktop-database")
                .arg(&applications)
                .status();
        }
        Ok(())
    }
}

#[cfg(windows)]
mod platform {
    use anyhow::{Context as _, Result};
    use std::{io::Write as _, os::windows::process::CommandExt as _};

    const EXTENSIONS: &[&str] = &[
        "txt", "md", "markdown", "mdx", "log", "json", "jsonc", "yaml", "yml", "toml", "ini",
        "cfg", "conf", "env", "xml", "html", "css", "scss", "csv", "tsv", "sql", "py", "js", "mjs",
        "ts", "tsx", "jsx", "rs", "go", "java", "kt", "c", "h", "cpp", "hpp", "cs", "rb", "php",
        "lua", "sh", "bash", "zsh", "ps1", "bat", "mk", "diff", "patch",
    ];
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn register() -> Result<()> {
        let exe = std::env::current_exe()?;
        let exe_text = exe
            .to_str()
            .context("The executable path isn’t valid UTF-8.")?
            .to_owned();
        if exe_text.contains('"') {
            return Ok(());
        }
        let marker = dirs::config_dir()
            .context("No config folder.")?
            .join("Larik Text/open-with-registered.txt");
        if std::fs::read_to_string(&marker).is_ok_and(|done| done == exe_text) {
            return Ok(());
        }
        let quoted = exe_text.replace('\\', "\\\\");
        let mut reg = String::from("Windows Registry Editor Version 5.00\r\n\r\n");
        let prog = "HKEY_CURRENT_USER\\Software\\Classes\\LarikText.Document";
        reg += &format!("[{prog}]\r\n@=\"Text document\"\r\n\r\n");
        // The document icon is the second icon resource embedded in the executable.
        reg += &format!("[{prog}\\DefaultIcon]\r\n@=\"\\\"{quoted}\\\",-2\"\r\n\r\n");
        reg +=
            &format!("[{prog}\\shell\\open\\command]\r\n@=\"\\\"{quoted}\\\" \\\"%1\\\"\"\r\n\r\n");
        let app = "HKEY_CURRENT_USER\\Software\\Classes\\Applications\\larik-text.exe";
        reg += &format!("[{app}]\r\n\"FriendlyAppName\"=\"Larik Text\"\r\n\r\n");
        reg +=
            &format!("[{app}\\shell\\open\\command]\r\n@=\"\\\"{quoted}\\\" \\\"%1\\\"\"\r\n\r\n");
        reg += &format!("[{app}\\SupportedTypes]\r\n");
        for ext in EXTENSIONS {
            reg += &format!("\".{ext}\"=\"\"\r\n");
        }
        reg += "\r\n";
        for ext in EXTENSIONS {
            reg += &format!(
                "[HKEY_CURRENT_USER\\Software\\Classes\\.{ext}\\OpenWithProgids]\r\n\"LarikText.Document\"=\"\"\r\n\r\n"
            );
        }
        let file = std::env::temp_dir().join("larik-text-open-with.reg");
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend(reg.encode_utf16().flat_map(u16::to_le_bytes));
        std::fs::File::create(&file)?.write_all(&bytes)?;
        let status = std::process::Command::new("reg")
            .arg("import")
            .arg(&file)
            .creation_flags(CREATE_NO_WINDOW)
            .status()?;
        let _ = std::fs::remove_file(&file);
        anyhow::ensure!(status.success(), "reg import failed.");
        std::fs::create_dir_all(marker.parent().context("No parent folder.")?)?;
        std::fs::write(marker, exe_text)?;
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    pub fn register() -> anyhow::Result<()> {
        Ok(())
    }
}
