use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

#[derive(Debug, Parser)]
#[command(
    name = "mfa-forge-launcher",
    about = "Staging and verification helper for MFA-Forge RC MSI upgrades",
    version
)]
struct LauncherCli {
    #[arg(long, value_name = "PATH_OR_URL")]
    release_json: Option<String>,
    #[arg(long, value_name = "MSI_NAME")]
    asset_name: Option<String>,
    #[arg(long, value_name = "OWNER/REPO")]
    repo: Option<String>,
    #[arg(long, value_name = "VERSION")]
    current_version: Option<String>,
    #[arg(long, value_name = "OUTPUT_DIR")]
    output_dir: PathBuf,
    #[arg(long, value_name = "REPORT_PATH")]
    report_path: Option<PathBuf>,
    #[arg(long)]
    apply: bool,
    #[arg(long)]
    passive: bool,
    #[arg(long, value_name = "PID")]
    parent_pid: Option<u32>,
    #[arg(long, value_name = "MSIEXEC_PATH", default_value = "msiexec.exe")]
    msiexec_path: PathBuf,
    #[arg(long, hide = true)]
    helper: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct ReleaseMetadata {
    tag_name: String,
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    html_url: Option<String>,
    assets: Vec<ReleaseAsset>,
}

#[derive(Clone, Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Serialize)]
struct LauncherReport {
    release_source: String,
    current_version: Option<String>,
    update_available: bool,
    update_target_version: Option<String>,
    release_tag: Option<String>,
    prerelease: Option<bool>,
    release_url: Option<String>,
    asset_name: Option<String>,
    checksum_asset_name: Option<String>,
    staged_msi_path: Option<String>,
    staged_checksum_path: Option<String>,
    expected_sha256: Option<String>,
    actual_sha256: Option<String>,
    checksum_verified: bool,
    installer_command: Vec<String>,
    install_result: Option<InstallResult>,
    status: String,
}

#[derive(Debug, Serialize)]
struct InstallResult {
    exit_code: Option<i32>,
    success: bool,
}

#[derive(Debug)]
struct PreparedUpdate {
    current_version: Option<String>,
    update_target_version: Option<String>,
    release_source: String,
    release_tag: String,
    prerelease: bool,
    release_url: Option<String>,
    asset_name: String,
    checksum_asset_name: String,
    staged_msi_path: PathBuf,
    staged_checksum_path: PathBuf,
    expected_sha256: String,
    actual_sha256: String,
    installer_command: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ReleaseVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

enum LauncherMode {
    Explicit {
        release_source: String,
        asset_name: String,
    },
    Auto {
        repo: String,
        current_version: ReleaseVersion,
    },
}

/// Execute the launcher CLI flow for staging and optional MSI handoff.
pub fn run() -> Result<()> {
    let cli = LauncherCli::parse();
    if spawn_apply_helper_if_needed(&cli)? {
        return Ok(());
    }
    let mode = resolve_mode(&cli)?;
    let prepared = match mode {
        LauncherMode::Explicit {
            release_source,
            asset_name,
        } => Some(prepare_update(
            &release_source,
            &asset_name,
            &cli.output_dir,
            &cli.msiexec_path,
            cli.passive,
        )?),
        LauncherMode::Auto {
            repo,
            current_version,
        } => prepare_latest_prerelease_update(
            &repo,
            current_version,
            &cli.output_dir,
            &cli.msiexec_path,
            cli.passive,
        )?,
    };

    let install_result = if cli.apply {
        prepared
            .as_ref()
            .map(|prepared| {
                if let Some(parent_pid) = cli.parent_pid {
                    close_parent_process(parent_pid)?;
                }
                run_msiexec(&prepared.installer_command)
            })
            .transpose()?
    } else {
        None
    };

    let report = match prepared {
        Some(prepared) => LauncherReport {
            release_source: prepared.release_source.clone(),
            current_version: prepared.current_version.clone(),
            update_available: true,
            update_target_version: prepared.update_target_version.clone(),
            release_tag: Some(prepared.release_tag.clone()),
            prerelease: Some(prepared.prerelease),
            release_url: prepared.release_url.clone(),
            asset_name: Some(prepared.asset_name.clone()),
            checksum_asset_name: Some(prepared.checksum_asset_name.clone()),
            staged_msi_path: Some(prepared.staged_msi_path.display().to_string()),
            staged_checksum_path: Some(prepared.staged_checksum_path.display().to_string()),
            expected_sha256: Some(prepared.expected_sha256.clone()),
            actual_sha256: Some(prepared.actual_sha256.clone()),
            checksum_verified: prepared.expected_sha256 == prepared.actual_sha256,
            installer_command: prepared.installer_command.clone(),
            install_result,
            status: "update-prepared".to_owned(),
        },
        None => LauncherReport {
            release_source: match (&cli.repo, &cli.release_json) {
                (Some(repo), _) => github_releases_url(repo),
                (None, Some(release_json)) => release_json.clone(),
                (None, None) => "unknown".to_owned(),
            },
            current_version: cli.current_version.clone(),
            update_available: false,
            update_target_version: None,
            release_tag: None,
            prerelease: None,
            release_url: None,
            asset_name: None,
            checksum_asset_name: None,
            staged_msi_path: None,
            staged_checksum_path: None,
            expected_sha256: None,
            actual_sha256: None,
            checksum_verified: false,
            installer_command: Vec::new(),
            install_result: None,
            status: "no-update".to_owned(),
        },
    };

    if let Some(report_path) = cli.report_path.as_ref() {
        write_report(report_path, &report)?;
    }

    println!("Launcher status: {}", report.status);
    if report.update_available {
        if let Some(release_tag) = report.release_tag.as_deref() {
            println!("Release: {}", release_tag);
        }
        if let Some(staged_msi_path) = report.staged_msi_path.as_deref() {
            println!("MSI staged at: {}", staged_msi_path);
        }
        println!("Checksum verified: {}", report.checksum_verified);
        println!("Installer command: {}", report.installer_command.join(" "));
        if let Some(result) = &report.install_result {
            println!(
                "Installer exit code: {}",
                result
                    .exit_code
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_owned())
            );
        }
    } else if let Some(current_version) = report.current_version.as_deref() {
        println!("No newer prerelease found for {}", current_version);
    }

    if let Some(result) = &report.install_result
        && !result.success
    {
        bail!(
            "msiexec devolvió código {} para {}",
            result
                .exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_owned()),
            report
                .staged_msi_path
                .as_deref()
                .unwrap_or("<unknown-msi-path>")
        );
    }

    Ok(())
}

fn resolve_mode(cli: &LauncherCli) -> Result<LauncherMode> {
    match (
        cli.release_json.as_deref(),
        cli.asset_name.as_deref(),
        cli.repo.as_deref(),
        cli.current_version.as_deref(),
    ) {
        (Some(release_source), Some(asset_name), None, None) => Ok(LauncherMode::Explicit {
            release_source: release_source.to_owned(),
            asset_name: asset_name.to_owned(),
        }),
        (None, None, Some(repo), Some(current_version)) => Ok(LauncherMode::Auto {
            repo: repo.to_owned(),
            current_version: parse_plain_version(current_version)?,
        }),
        _ => bail!(
            "Use either --release-json + --asset-name for explicit mode or --repo + --current-version for automatic mode."
        ),
    }
}

fn spawn_apply_helper_if_needed(cli: &LauncherCli) -> Result<bool> {
    if !cli.apply || cli.helper {
        return Ok(false);
    }

    fs::create_dir_all(&cli.output_dir).with_context(|| {
        format!(
            "No se pudo crear el directorio {}",
            cli.output_dir.display()
        )
    })?;

    let current_exe = std::env::current_exe()
        .with_context(|| "No se pudo resolver la ruta del launcher actual".to_owned())?
        .canonicalize()
        .with_context(|| "No se pudo resolver la ruta canónica del launcher actual".to_owned())?;
    let helper_path = cli.output_dir.join("mfa-forge-launcher-helper.exe");

    let helper_canonical = if helper_path.exists() {
        Some(
            helper_path
                .canonicalize()
                .with_context(|| format!("No se pudo resolver {}", helper_path.display()))?,
        )
    } else {
        None
    };

    if helper_canonical.as_ref() == Some(&current_exe) {
        return Ok(false);
    }

    fs::copy(&current_exe, &helper_path).with_context(|| {
        format!(
            "No se pudo copiar {} a {}",
            current_exe.display(),
            helper_path.display()
        )
    })?;

    let mut command = Command::new(&helper_path);
    command.args(cli.to_args(true));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    command.spawn().with_context(|| {
        format!(
            "No se pudo iniciar el launcher helper {}",
            helper_path.display()
        )
    })?;

    Ok(true)
}

fn prepare_update(
    release_source: &str,
    asset_name: &str,
    output_dir: &Path,
    msiexec_path: &Path,
    passive: bool,
) -> Result<PreparedUpdate> {
    let release = load_release_metadata(release_source)?;
    prepare_update_from_release(
        release_source,
        None,
        None,
        release,
        asset_name,
        output_dir,
        msiexec_path,
        passive,
    )
}

fn prepare_latest_prerelease_update(
    repo: &str,
    current_version: ReleaseVersion,
    output_dir: &Path,
    msiexec_path: &Path,
    passive: bool,
) -> Result<Option<PreparedUpdate>> {
    let release_source = github_releases_url(repo);
    let payload = read_text_source(&release_source)?;
    let releases: Vec<ReleaseMetadata> = serde_json::from_str(&payload).with_context(|| {
        format!("No se pudo parsear metadata de releases desde {release_source}")
    })?;
    let release = select_latest_prerelease(&releases).filter(|release| {
        parse_release_tag(&release.tag_name).is_some_and(|version| version > current_version)
    });

    let Some(release) = release else {
        return Ok(None);
    };

    let target_version = parse_release_tag(&release.tag_name)
        .ok_or_else(|| anyhow!("Tag RC no soportado: {}", release.tag_name))?;
    let asset_name = format!("MFA-Forge-RC{}-x64.msi", target_version.patch);

    let prepared = prepare_update_from_release(
        &release_source,
        Some(format_release_version(current_version)),
        Some(format_release_version(target_version)),
        release.clone(),
        &asset_name,
        output_dir,
        msiexec_path,
        passive,
    )?;

    Ok(Some(prepared))
}

#[allow(clippy::too_many_arguments)]
fn prepare_update_from_release(
    release_source: &str,
    current_version: Option<String>,
    update_target_version: Option<String>,
    release: ReleaseMetadata,
    asset_name: &str,
    output_dir: &Path,
    msiexec_path: &Path,
    passive: bool,
) -> Result<PreparedUpdate> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("No se pudo crear el directorio {}", output_dir.display()))?;

    if !release.prerelease {
        bail!(
            "El release {} no está marcado como prerelease; las RC de MFA-Forge deben validarse como prerelease.",
            release.tag_name
        );
    }

    let checksum_asset_name = format!("{asset_name}.sha256.txt");
    let msi_asset = find_asset(&release.assets, asset_name)?;
    let checksum_asset = find_asset(&release.assets, &checksum_asset_name)?;

    let staged_msi_path = output_dir.join(asset_name);
    let staged_checksum_path = output_dir.join(&checksum_asset_name);

    fetch_asset(&msi_asset.browser_download_url, &staged_msi_path)?;
    fetch_asset(&checksum_asset.browser_download_url, &staged_checksum_path)?;

    let staged_msi_path = staged_msi_path
        .canonicalize()
        .with_context(|| format!("No se pudo resolver {}", staged_msi_path.display()))?;
    let staged_checksum_path = staged_checksum_path
        .canonicalize()
        .with_context(|| format!("No se pudo resolver {}", staged_checksum_path.display()))?;

    let expected_sha256 = parse_checksum_file(&staged_checksum_path)?;
    let actual_sha256 = compute_sha256(&staged_msi_path)?;
    if expected_sha256 != actual_sha256 {
        bail!(
            "SHA256 no coincide para {}. Esperado {}, obtenido {}.",
            staged_msi_path.display(),
            expected_sha256,
            actual_sha256
        );
    }

    Ok(PreparedUpdate {
        current_version,
        update_target_version,
        release_source: release_source.to_owned(),
        release_tag: release.tag_name,
        prerelease: release.prerelease,
        release_url: release.html_url,
        asset_name: asset_name.to_owned(),
        checksum_asset_name,
        staged_msi_path: staged_msi_path.clone(),
        staged_checksum_path,
        expected_sha256,
        actual_sha256,
        installer_command: build_msiexec_command(msiexec_path, &staged_msi_path, passive),
    })
}

fn load_release_metadata(release_source: &str) -> Result<ReleaseMetadata> {
    let payload = read_text_source(release_source)?;
    serde_json::from_str(&payload).with_context(|| {
        format!(
            "No se pudo parsear metadata de release desde {}",
            release_source
        )
    })
}

fn github_releases_url(repo: &str) -> String {
    format!("https://api.github.com/repos/{repo}/releases")
}

fn select_latest_prerelease(releases: &[ReleaseMetadata]) -> Option<&ReleaseMetadata> {
    releases
        .iter()
        .filter(|release| release.prerelease && !release.draft)
        .filter_map(|release| {
            parse_release_tag(&release.tag_name).map(|version| (version, release))
        })
        .max_by_key(|(version, _)| *version)
        .map(|(_, release)| release)
}

fn parse_release_tag(tag: &str) -> Option<ReleaseVersion> {
    let trimmed = tag.strip_prefix('v')?;
    let (version_part, suffix) = trimmed.split_once("-rc.")?;
    let version = parse_plain_version(version_part).ok()?;
    let rc_number = suffix.parse::<u64>().ok()?;
    (version.patch == rc_number).then_some(version)
}

fn parse_plain_version(version: &str) -> Result<ReleaseVersion> {
    let mut parts = version.split('.');
    let major = parts
        .next()
        .ok_or_else(|| anyhow!("Formato de versión no soportado: {version}"))?
        .parse::<u64>()
        .with_context(|| format!("Formato de versión no soportado: {version}"))?;
    let minor = parts
        .next()
        .ok_or_else(|| anyhow!("Formato de versión no soportado: {version}"))?
        .parse::<u64>()
        .with_context(|| format!("Formato de versión no soportado: {version}"))?;
    let patch = parts
        .next()
        .ok_or_else(|| anyhow!("Formato de versión no soportado: {version}"))?
        .parse::<u64>()
        .with_context(|| format!("Formato de versión no soportado: {version}"))?;

    if parts.next().is_some() {
        bail!("Formato de versión no soportado: {version}");
    }

    Ok(ReleaseVersion {
        major,
        minor,
        patch,
    })
}

fn format_release_version(version: ReleaseVersion) -> String {
    format!("{}.{}.{}", version.major, version.minor, version.patch)
}

fn close_parent_process(parent_pid: u32) -> Result<()> {
    let status = Command::new("taskkill")
        .args(["/PID", &parent_pid.to_string(), "/T", "/F"])
        .status()
        .with_context(|| format!("No se pudo cerrar el proceso padre {parent_pid}"))?;

    if !status.success() {
        bail!("No se pudo cerrar el proceso padre {parent_pid}");
    }

    std::thread::sleep(std::time::Duration::from_secs(2));
    Ok(())
}

fn read_text_source(source: &str) -> Result<String> {
    if let Some(path) = local_path_source(source) {
        return fs::read_to_string(&path)
            .with_context(|| format!("No se pudo leer {}", path.display()));
    }

    if let Ok(url) = Url::parse(source) {
        return match url.scheme() {
            "http" | "https" => {
                let response = ureq::get(source)
                    .set("Accept", "application/vnd.github+json")
                    .set("User-Agent", "mfa-forge-launcher")
                    .call()
                    .with_context(|| format!("No se pudo descargar {}", source))?;
                let mut reader = response.into_reader();
                let mut buffer = String::new();
                reader
                    .read_to_string(&mut buffer)
                    .with_context(|| format!("No se pudo leer {}", source))?;
                Ok(buffer)
            }
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|_| anyhow!("URL file inválida: {}", source))?;
                fs::read_to_string(&path)
                    .with_context(|| format!("No se pudo leer {}", path.display()))
            }
            _ => bail!(
                "Esquema no soportado para metadata de release: {}",
                url.scheme()
            ),
        };
    }

    fs::read_to_string(source).with_context(|| format!("No se pudo leer {}", source))
}

fn find_asset<'a>(assets: &'a [ReleaseAsset], name: &str) -> Result<&'a ReleaseAsset> {
    assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(name))
        .with_context(|| format!("No se encontró el asset requerido {name} en la metadata"))
}

fn fetch_asset(source: &str, destination: &Path) -> Result<()> {
    if let Some(path) = local_path_source(source) {
        fs::copy(&path, destination).with_context(|| {
            format!(
                "No se pudo copiar {} a {}",
                path.display(),
                destination.display()
            )
        })?;
        return Ok(());
    }

    if let Ok(url) = Url::parse(source) {
        match url.scheme() {
            "http" | "https" => {
                let response = ureq::get(source)
                    .set("User-Agent", "mfa-forge-launcher")
                    .call()
                    .with_context(|| format!("No se pudo descargar asset {}", source))?;
                let mut reader = response.into_reader();
                let mut file = File::create(destination)
                    .with_context(|| format!("No se pudo crear {}", destination.display()))?;
                std::io::copy(&mut reader, &mut file)
                    .with_context(|| format!("No se pudo escribir {}", destination.display()))?;
                return Ok(());
            }
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|_| anyhow!("URL file inválida: {}", source))?;
                fs::copy(&path, destination).with_context(|| {
                    format!(
                        "No se pudo copiar {} a {}",
                        path.display(),
                        destination.display()
                    )
                })?;
                return Ok(());
            }
            _ => bail!("Esquema no soportado para asset: {}", url.scheme()),
        }
    }

    fs::copy(source, destination)
        .with_context(|| format!("No se pudo copiar {} a {}", source, destination.display()))?;
    Ok(())
}

fn local_path_source(source: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(source);
    if candidate.is_absolute() || candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

fn parse_checksum_file(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("No se pudo abrir {}", path.display()))?;
    let mut lines = BufReader::new(file).lines();
    let line = lines
        .next()
        .transpose()
        .with_context(|| format!("No se pudo leer {}", path.display()))?
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Archivo checksum vacío: {}", path.display()))?;

    let hash = line
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow!("Checksum inválido en {}", path.display()))?;

    let normalized = hash.trim().to_uppercase();
    if normalized.len() != 64 || !normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!("Checksum SHA256 inválido en {}", path.display());
    }

    Ok(normalized)
}

fn compute_sha256(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("No se pudo abrir {}", path.display()))?;
    let mut buffer = [0_u8; 8192];
    let mut hasher = Sha256::new();

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .with_context(|| format!("No se pudo leer {}", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:X}", hasher.finalize()))
}

fn build_msiexec_command(msiexec_path: &Path, msi_path: &Path, passive: bool) -> Vec<String> {
    let mut command = vec![
        msiexec_path.display().to_string(),
        "/i".to_owned(),
        normalize_windows_path(msi_path),
    ];
    if passive {
        command.push("/passive".to_owned());
    }
    command
}

fn normalize_windows_path(path: &Path) -> String {
    let raw = path.display().to_string();
    raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_owned()
}

fn run_msiexec(command_parts: &[String]) -> Result<InstallResult> {
    let (program, arguments) = command_parts
        .split_first()
        .ok_or_else(|| anyhow!("Comando msiexec vacío"))?;
    let status = Command::new(program)
        .args(arguments)
        .status()
        .with_context(|| format!("No se pudo ejecutar {}", program))?;
    Ok(InstallResult {
        exit_code: status.code(),
        success: status.success(),
    })
}

fn write_report(path: &Path, report: &LauncherReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("No se pudo crear {}", parent.display()))?;
    }

    let mut file =
        File::create(path).with_context(|| format!("No se pudo crear {}", path.display()))?;
    let payload = serde_json::to_string_pretty(report)?;
    file.write_all(payload.as_bytes())
        .with_context(|| format!("No se pudo escribir {}", path.display()))
}

impl LauncherCli {
    fn to_args(&self, include_helper: bool) -> Vec<String> {
        let mut args = Vec::new();

        if let Some(release_json) = &self.release_json {
            args.push("--release-json".to_owned());
            args.push(release_json.clone());
        }
        if let Some(asset_name) = &self.asset_name {
            args.push("--asset-name".to_owned());
            args.push(asset_name.clone());
        }
        if let Some(repo) = &self.repo {
            args.push("--repo".to_owned());
            args.push(repo.clone());
        }
        if let Some(current_version) = &self.current_version {
            args.push("--current-version".to_owned());
            args.push(current_version.clone());
        }

        args.push("--output-dir".to_owned());
        args.push(self.output_dir.display().to_string());

        if let Some(report_path) = &self.report_path {
            args.push("--report-path".to_owned());
            args.push(report_path.display().to_string());
        }
        if self.apply {
            args.push("--apply".to_owned());
        }
        if self.passive {
            args.push("--passive".to_owned());
        }
        if let Some(parent_pid) = self.parent_pid {
            args.push("--parent-pid".to_owned());
            args.push(parent_pid.to_string());
        }

        args.push("--msiexec-path".to_owned());
        args.push(self.msiexec_path.display().to_string());

        if include_helper {
            args.push("--helper".to_owned());
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn checksum_parser_accepts_manifest_format() {
        let temp = tempdir().expect("tempdir should be created");
        let checksum_path = temp.path().join("asset.sha256.txt");
        fs::write(
            &checksum_path,
            "AEDD696039508AEA0069C2413E9E00F0982B0604E1AA4CA525E73FC909475066 *MFA-Forge-RC17-x64.msi",
        )
        .expect("checksum file should be written");

        let parsed = parse_checksum_file(&checksum_path).expect("checksum should parse");
        assert_eq!(
            parsed,
            "AEDD696039508AEA0069C2413E9E00F0982B0604E1AA4CA525E73FC909475066"
        );
    }

    #[test]
    fn prepare_update_copies_local_assets_and_verifies_checksum() {
        let temp = tempdir().expect("tempdir should be created");
        let release_dir = temp.path().join("release");
        let output_dir = temp.path().join("staged");
        fs::create_dir_all(&release_dir).expect("release dir should exist");

        let msi_name = "MFA-Forge-RC18-x64.msi";
        let msi_path = release_dir.join(msi_name);
        fs::write(&msi_path, b"fake msi bytes").expect("msi should be written");

        let hash = compute_sha256(&msi_path).expect("hash should compute");
        let checksum_name = format!("{msi_name}.sha256.txt");
        let checksum_path = release_dir.join(&checksum_name);
        fs::write(&checksum_path, format!("{hash} *{msi_name}\n"))
            .expect("checksum file should be written");

        let release_json_path = release_dir.join("release.json");
        let release_payload = json!({
            "tag_name": "v0.1.18-rc.18",
            "prerelease": true,
            "html_url": "https://example.invalid/releases/tag/v0.1.18-rc.18",
            "assets": [
                {
                    "name": msi_name,
                    "browser_download_url": msi_path.display().to_string(),
                },
                {
                    "name": checksum_name,
                    "browser_download_url": checksum_path.display().to_string(),
                }
            ]
        });
        fs::write(
            &release_json_path,
            serde_json::to_vec_pretty(&release_payload).expect("release json should serialize"),
        )
        .expect("release json should be written");

        let prepared = prepare_update(
            &release_json_path.display().to_string(),
            msi_name,
            &output_dir,
            Path::new("msiexec.exe"),
            true,
        )
        .expect("update should prepare");

        assert_eq!(prepared.release_tag, "v0.1.18-rc.18");
        assert_eq!(prepared.expected_sha256, prepared.actual_sha256);
        assert!(prepared.staged_msi_path.exists());
        assert!(prepared.staged_checksum_path.exists());
        assert_eq!(
            prepared.installer_command,
            vec![
                "msiexec.exe".to_owned(),
                "/i".to_owned(),
                normalize_windows_path(&prepared.staged_msi_path),
                "/passive".to_owned()
            ]
        );
    }

    #[test]
    fn parse_release_tag_requires_matching_patch_and_rc_number() {
        let version = parse_release_tag("v0.1.21-rc.21").expect("tag should parse");
        assert_eq!(format_release_version(version), "0.1.21");
        assert!(parse_release_tag("v0.1.21-rc.22").is_none());
    }

    #[test]
    fn select_latest_prerelease_ignores_drafts_and_non_matching_tags() {
        let releases = vec![
            ReleaseMetadata {
                tag_name: "v0.1.20-rc.20".to_owned(),
                prerelease: true,
                draft: false,
                html_url: None,
                assets: Vec::new(),
            },
            ReleaseMetadata {
                tag_name: "v0.1.21-rc.21".to_owned(),
                prerelease: true,
                draft: true,
                html_url: None,
                assets: Vec::new(),
            },
            ReleaseMetadata {
                tag_name: "v0.1.22-rc.22".to_owned(),
                prerelease: true,
                draft: false,
                html_url: None,
                assets: Vec::new(),
            },
        ];

        let latest = select_latest_prerelease(&releases).expect("latest prerelease should exist");
        assert_eq!(latest.tag_name, "v0.1.22-rc.22");
    }
}
