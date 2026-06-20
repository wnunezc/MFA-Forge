use std::{env, path::PathBuf, process::Command};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::{
    app::ForgeApp,
    i18n::{tr, trf},
    state::BannerTone,
};

impl ForgeApp {
    pub fn current_release_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    pub fn update_stage_directory(&self) -> Result<PathBuf, String> {
        release_update_stage_directory("manual")
    }

    pub fn open_update_dialog(&mut self) {
        self.state.update_dialog.open();
    }

    pub fn start_latest_rc_update(&mut self, automatic: bool) {
        if !automatic {
            self.state.update_dialog.error = None;
        }

        let launcher_path = match launcher_path_from_current_install() {
            Ok(path) => path,
            Err(error) => {
                if automatic {
                    self.set_banner(BannerTone::Warning, error);
                } else {
                    self.state.update_dialog.error = Some(error);
                }
                return;
            }
        };

        let stage_dir =
            match release_update_stage_directory(if automatic { "startup" } else { "manual" }) {
                Ok(path) => path,
                Err(error) => {
                    if automatic {
                        self.set_banner(BannerTone::Warning, error);
                    } else {
                        self.state.update_dialog.error = Some(error);
                    }
                    return;
                }
            };

        if let Err(error) = std::fs::create_dir_all(&stage_dir) {
            let message =
                format!("The local update staging directory could not be created: {error}");
            if automatic {
                self.set_banner(BannerTone::Warning, message);
            } else {
                self.state.update_dialog.error = Some(message);
            }
            return;
        }

        let report_path = stage_dir.join(if automatic {
            "launcher-auto-report.json"
        } else {
            "launcher-manual-report.json"
        });
        let mut command = Command::new(&launcher_path);
        command
            .arg("--repo")
            .arg("wnunezc/MFA-Forge")
            .arg("--current-version")
            .arg(self.current_release_version())
            .arg("--output-dir")
            .arg(&stage_dir)
            .arg("--report-path")
            .arg(&report_path)
            .arg("--apply");

        if automatic {
            command
                .arg("--passive")
                .arg("--parent-pid")
                .arg(std::process::id().to_string());
        }

        #[cfg(windows)]
        command.creation_flags(0x08000000);

        match command.spawn() {
            Ok(_) => {
                if automatic {
                    self.set_banner(
                        BannerTone::Info,
                        tr(
                            "Automatic RC update check started. If GitHub has a newer prerelease, the launcher will verify it and hand control to Windows Installer.",
                        ),
                    );
                } else {
                    self.state.update_dialog.close();
                    self.set_banner(
                        BannerTone::Info,
                        tr(
                            "Launcher started. It will check GitHub for a newer prerelease RC, verify the MSI checksum, and then open Windows Installer if an update exists.",
                        ),
                    );
                }
            }
            Err(error) => {
                let message = trf(
                    "The update launcher could not be started: {error}",
                    &[("error", &error.to_string())],
                );
                if automatic {
                    self.set_banner(BannerTone::Warning, message);
                } else {
                    self.state.update_dialog.error = Some(message);
                }
            }
        }
    }
}

fn launcher_path_from_current_install() -> Result<PathBuf, String> {
    let launcher_path = env::current_exe()
        .map_err(|error| format!("The current GUI executable path could not be resolved: {error}"))?
        .with_file_name("mfa-forge-launcher.exe");

    if launcher_path.is_file() {
        Ok(launcher_path)
    } else {
        Err(trf(
            "The installed launcher could not be found at {path}. This build cannot start the launcher-driven path.",
            &[("path", &launcher_path.display().to_string())],
        ))
    }
}

fn release_update_stage_directory(channel: &str) -> Result<PathBuf, String> {
    Ok(mfa_forge_storage::app_data::data_local_file("updates")?.join(channel))
}
