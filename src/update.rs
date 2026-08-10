//! Comando `update` (US-040): atualização automática do binário + service.
//!
//! Fluxo: verifica a release mais recente no GitHub (mesmo fluxo do `--version`,
//! US-029), compara SemVer e, quando há atualização, para o daemon, baixa o novo
//! binário, valida a assinatura digital (US-018) quando a CA local existir,
//! substitui o binário e religa o service. Qualquer falha mantém o binário e o
//! service atuais intactos.

use std::{fs, path::Path, process::Command};

use anyhow::{Context, bail};

use crate::{daemon, fetch_latest_release};

/// Comparação SemVer: `true` quando a release remota (tag, ex.: `v0.1.16`) é
/// estritamente superior à versão instalada. Versões malformadas não contam
/// como atualização (falha graciosa).
pub(crate) fn has_update(latest_tag: &str, current: &str) -> bool {
    let latest = latest_tag.trim_start_matches('v');
    match (
        semver::Version::parse(latest),
        semver::Version::parse(current),
    ) {
        (Ok(a), Ok(b)) => a > b,
        _ => false,
    }
}

/// Nome do asset binário de uma release no GitHub (Linux x86_64).
fn asset_name(tag: &str) -> String {
    format!("opentorrent-{tag}-linux-x86_64")
}

/// URL de download do asset binário de uma release.
fn asset_url(tag: &str) -> String {
    format!(
        "https://github.com/filhotecmail/opentorrent/releases/download/{tag}/{}",
        asset_name(tag)
    )
}

/// Diretório das chaves/certificados de assinatura (US-018): a CA local
/// (`ca.crt` + `code-signing.pub`) mantida fora do git pelo `sign-release.sh`.
fn signing_dir() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.local/share/opentorrent"))
        .join("opentorrent")
        .join("signing")
}

/// Indica se a assinatura pode ser validada localmente (CA da US-018 presente).
fn can_verify_signature() -> bool {
    let dir = signing_dir();
    dir.join("ca.crt").exists() && dir.join("code-signing.pub").exists()
}

/// Baixa um asset e o grava em `dest`. Falha de rede/JSON aborta a atualização
/// sem tocar no binário instalado.
async fn download_to(url: &str, dest: &Path) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("User-Agent", "opentorrent")
        .send()
        .await
        .context("falha ao baixar a release (rede)")?
        .error_for_status()
        .context("release não encontrada no GitHub")?;
    let bytes = resp
        .bytes()
        .await
        .context("falha ao ler o binário da release")?;
    fs::write(dest, &bytes)
        .with_context(|| format!("falha ao gravar o binário baixado em {}", dest.display()))?;
    Ok(())
}

/// Valida a assinatura digital do binário baixado contra a CA local (US-018)
/// caso ela exista. Sem CA local, a verificação é pulada (o GitHub já garante a
/// origem via HTTPS). Falha de verificação aborta a atualização.
async fn verify_signature(bin: &Path, tag: &str) -> anyhow::Result<()> {
    if !can_verify_signature() {
        return Ok(());
    }
    let dir = signing_dir();
    let sig = format!("{}.sig", bin.display());
    let sig_url = format!("{}.sig", asset_url(tag));
    download_to(&sig_url, Path::new(&sig))
        .await
        .context("falha ao baixar a assinatura (.sig) para verificação")?;

    let output = Command::new("openssl")
        .args([
            "dgst",
            "-sha256",
            "-verify",
            dir.join("code-signing.pub").to_str().unwrap(),
            "-signature",
            &sig,
            bin.to_str().unwrap(),
        ])
        .output()
        .context("falha ao executar openssl para verificar a assinatura")?;
    if !output.status.success() {
        bail!(
            "assinatura inválida do binário baixado: {} (abortando sem substituir)",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Comando sugerido para atualizar o binário (agora um subcomando nativo).
pub(crate) fn update_command() -> String {
    "opentorrent update".to_string()
}

/// Executa `opentorrent update`: para o daemon, substitui o binário pela
/// release mais recente e religa o service.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn run_update() -> anyhow::Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    println!("OpenTorrent v{current}");

    let Some(tag) = fetch_latest_release().await else {
        bail!("não foi possível consultar a última release (offline?)");
    };
    if !has_update(&tag, current) {
        println!("você está na versão mais recente (v{current})");
        return Ok(());
    }
    println!("Nova versão disponível: {tag} — atualizando...");

    // Diretório temporário para o binário baixado (mesmo filesystem em geral).
    let temp_dir = std::env::temp_dir();
    let dest = temp_dir.join(asset_name(&tag));
    download_to(&asset_url(&tag), &dest).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))
            .context("falha ao tornar o binário baixado executável")?;
    }

    // Valida a assinatura antes de qualquer substituição.
    verify_signature(&dest, &tag).await?;

    // Para o daemon (service systemd ou fallback spawn) antes de substituir.
    if daemon::is_running()? {
        daemon::stop().await?;
    }

    // Substituição atômica do binário (temp + rename).
    let exe = std::env::current_exe().context("não foi possível localizar o binário atual")?;
    fs::rename(&dest, &exe).with_context(|| {
        format!(
            "falha ao substituir o binário em {} (possível falta de permissão)",
            exe.display()
        )
    })?;

    // Reinstala o service (unit pode referenciar o novo caminho) e religa.
    daemon::install_service()?;
    daemon::start()?;

    println!("atualizado para {tag} — daemon religado.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_update_detects_newer_release() {
        assert!(has_update("v0.1.12", "0.1.10"));
        assert!(has_update("v0.2.0", "0.1.99"));
    }

    #[test]
    fn has_update_false_when_equal_or_older() {
        assert!(!has_update("v0.1.11", "0.1.11"));
        assert!(!has_update("v0.1.10", "0.1.11"));
    }

    #[test]
    fn has_update_ignores_malformed_versions() {
        assert!(!has_update("not-a-version", "0.1.15"));
        assert!(!has_update("v0.1.16", "junk"));
        assert!(!has_update("", "0.1.15"));
    }

    #[test]
    fn asset_name_targets_linux_x86_64() {
        let name = asset_name("v0.1.16");
        assert_eq!(name, "opentorrent-v0.1.16-linux-x86_64");
        let url = asset_url("v0.1.16");
        assert!(url.contains("/download/v0.1.16/opentorrent-v0.1.16-linux-x86_64"));
    }

    #[test]
    fn update_command_native_subcommand() {
        assert_eq!(update_command(), "opentorrent update");
    }
}
