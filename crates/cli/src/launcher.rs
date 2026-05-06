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
    release_json: String,
    #[arg(long, value_name = "MSI_NAME")]
    asset_name: String,
    #[arg(long, value_name = "OUTPUT_DIR")]
    output_dir: PathBuf,
    #[arg(long, value_name = "REPORT_PATH")]
    report_path: Option<PathBuf>,
    #[arg(long)]
    apply: bool,
    #[arg(long)]
    passive: bool,
    #[arg(long, value_name = "MSIEXEC_PATH", default_value = "msiexec.exe")]
    msiexec_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ReleaseMetadata {
    tag_name: String,
    prerelease: bool,
    html_url: Option<String>,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Serialize)]
struct LauncherReport {
    release_source: String,
    release_tag: String,
    prerelease: bool,
    release_url: Option<String>,
    asset_name: String,
    checksum_asset_name: String,
    staged_msi_path: String,
    staged_checksum_path: String,
    expected_sha256: String,
    actual_sha256: String,
    checksum_verified: bool,
    installer_command: Vec<String>,
    install_result: Option<InstallResult>,
}

#[derive(Debug, Serialize)]
struct InstallResult {
    exit_code: Option<i32>,
    success: bool,
}

#[derive(Debug)]
struct PreparedUpdate {
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

/// Execute the launcher CLI flow for staging and optional MSI handoff.
pub fn run() -> Result<()> {
    let cli = LauncherCli::parse();
    let prepared = prepare_update(
        &cli.release_json,
        &cli.asset_name,
        &cli.output_dir,
        &cli.msiexec_path,
        cli.passive,
    )?;

    let install_result = if cli.apply {
        Some(run_msiexec(&prepared.installer_command)?)
    } else {
        None
    };

    let report = LauncherReport {
        release_source: cli.release_json.clone(),
        release_tag: prepared.release_tag.clone(),
        prerelease: prepared.prerelease,
        release_url: prepared.release_url.clone(),
        asset_name: prepared.asset_name.clone(),
        checksum_asset_name: prepared.checksum_asset_name.clone(),
        staged_msi_path: prepared.staged_msi_path.display().to_string(),
        staged_checksum_path: prepared.staged_checksum_path.display().to_string(),
        expected_sha256: prepared.expected_sha256.clone(),
        actual_sha256: prepared.actual_sha256.clone(),
        checksum_verified: prepared.expected_sha256 == prepared.actual_sha256,
        installer_command: prepared.installer_command.clone(),
        install_result,
    };

    if let Some(report_path) = cli.report_path.as_ref() {
        write_report(report_path, &report)?;
    }

    println!("Release: {}", report.release_tag);
    println!("MSI staged at: {}", report.staged_msi_path);
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

    if let Some(result) = &report.install_result
        && !result.success
    {
        bail!(
            "msiexec devolvió código {} para {}",
            result
                .exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_owned()),
            report.staged_msi_path
        );
    }

    Ok(())
}

fn prepare_update(
    release_source: &str,
    asset_name: &str,
    output_dir: &Path,
    msiexec_path: &Path,
    passive: bool,
) -> Result<PreparedUpdate> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("No se pudo crear el directorio {}", output_dir.display()))?;

    let release = load_release_metadata(release_source)?;
    if !release.prerelease {
        bail!(
            "El release {} no está marcado como prerelease; RC18 debe validarse como prerelease.",
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
}
