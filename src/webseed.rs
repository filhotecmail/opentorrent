//! Webseed (HTTP seeding, BEP 19 GetRight-style) para torrents com `url-list`
//! (US-049).
//!
//! O motor `librqbit` não suporta webseed; torrents com `url-list` e 0 seeds no
//! tracker (ex.: coleções do archive.org) nunca baixam. Este módulo implementa a
//! fonte primária: extrai as bases `url-list` do bencode, baixa cada arquivo via
//! HTTP com Range no layout final de saída (com progresso) e entrega o `.torrent`
//! ao motor — o `initial_check` do `librqbit` valida as peças baixadas por SHA1
//! e os peers completam o restante. Sem dependência nova (`librqbit-bencode` é
//! transitiva do `librqbit`; `reqwest`/`url` já existem).

#[cfg(test)]
use std::time::Instant;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, bail};
use librqbit::{ByteBufOwned, TorrentMetaV1Info};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

/// Raiz mínima do metainfo parseada só para extrair `url-list` (BEP 19). Os
/// demais campos do dicionário são ignorados pelo serde.
#[derive(Deserialize)]
struct WebseedRoot {
    #[serde(rename = "url-list")]
    url_list: Option<UrlListValue>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum UrlListValue {
    Many(Vec<String>),
    One(String),
}

/// Configuração extraída do metainfo com `url-list` presente.
#[derive(Debug, Clone)]
pub(crate) struct WebseedConfig {
    /// Bases GetRight (URLs terminando em `/`), na ordem do `url-list`.
    pub(crate) bases: Vec<String>,
    /// Nome do torrent (`info.name`) — subfolder do layout e parte da URL.
    pub(crate) name: String,
}

/// Um arquivo do torrent a baixar via HTTP (ou a criar, em caso de padding/0).
#[derive(Debug, Clone)]
pub(crate) struct WebseedFile {
    /// Caminho relativo ao subfolder (ex.: `20.1/linuxmint-....iso`).
    pub(crate) relative_path: PathBuf,
    pub(crate) length: u64,
    /// Padding (`attr = 'p'`) — criado como zero/espanso, sem download.
    pub(crate) is_padding: bool,
}

/// Estado atual de uma tarefa webseed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum WebseedState {
    #[default]
    Downloading,
    Done,
    Error(String),
}

/// Progresso em tempo real de uma tarefa webseed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct WebseedProgress {
    pub(crate) total_bytes: u64,
    pub(crate) downloaded_bytes: u64,
    pub(crate) total_files: usize,
    pub(crate) done_files: usize,
    pub(crate) state: WebseedState,
}

/// Rótulo de domínio do estado (espelha `WebseedSnapshot.state` da IPC).
impl std::fmt::Display for WebseedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebseedState::Downloading => write!(f, "Downloading"),
            WebseedState::Done => write!(f, "Done"),
            WebseedState::Error(err) => write!(f, "{err}"),
        }
    }
}

/// Entrada viva do registro webseed do daemon (US-049): identidade da tarefa +
/// progresso atual. O snapshot IPC é derivado desta entrada. O registro é
/// infraestrutura do daemon (só-unix), sem uso no modo CLI/Windows.
#[cfg(unix)]
#[derive(Debug, Clone)]
pub(crate) struct WebseedTask {
    pub(crate) id: usize,
    pub(crate) source: String,
    pub(crate) name: String,
    pub(crate) progress: WebseedProgress,
}

/// Registro compartilhado das tarefas webseed em andamento (US-049). Criado no
/// daemon, preenchido pela task background e lido pelo `build_state`.
#[cfg(unix)]
pub(crate) type WebseedRegistry = Arc<Mutex<Vec<WebseedTask>>>;

/// Cria um registro vazio de tarefas webseed.
#[cfg(unix)]
pub(crate) fn new_registry() -> WebseedRegistry {
    Arc::new(Mutex::new(Vec::new()))
}

#[cfg(unix)]
impl WebseedTask {
    /// Derivada serializável para a IPC (`DaemonState.webseed`). A IPC é
    /// só-unix (socket; `crate::ipc` não existe no Windows).
    pub(crate) fn snapshot(&self) -> crate::ipc::WebseedSnapshot {
        crate::ipc::WebseedSnapshot {
            id: self.id,
            source: self.source.clone(),
            name: self.name.clone(),
            downloaded_bytes: self.progress.downloaded_bytes,
            total_bytes: self.progress.total_bytes,
            done_files: self.progress.done_files,
            total_files: self.progress.total_files,
            state: self.progress.state.to_string(),
        }
    }
}

/// Extrai as bases GetRight (URLs terminando em `/`) do campo raiz `url-list`
/// no bencode bruto. `None` ⇒ sem `url-list` ou nenhuma base GetRight → seguir
/// por peers (FR-006).
pub(crate) fn extract_url_list(bytes: &[u8]) -> Option<Vec<String>> {
    let mut de = librqbit_bencode::BencodeDeserializer::new_from_buf(bytes);
    let root: WebseedRoot = match serde::Deserialize::deserialize(&mut de) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("BENCODE_DESER_ERR: {e:?}");
            return None;
        }
    };
    let urls = match root.url_list? {
        UrlListValue::Many(urls) => urls,
        UrlListValue::One(url) => vec![url],
    };
    let mut bases: Vec<String> = Vec::with_capacity(urls.len());
    for url in urls {
        if url.ends_with('/') {
            bases.push(url);
        }
    }
    if bases.is_empty() { None } else { Some(bases) }
}

/// Monta a lista de arquivos a partir do metainfo, filtrando por `only_files`
/// quando aplicável. Padding é marcado (e não conta no progresso).
pub(crate) fn build_webseed_files(
    info: &TorrentMetaV1Info<ByteBufOwned>,
    only_files: Option<&[usize]>,
) -> anyhow::Result<Vec<WebseedFile>> {
    let mut files = Vec::new();
    for (idx, file) in info.iter_file_details()?.enumerate() {
        if only_files.is_some_and(|files| !files.contains(&idx)) {
            continue;
        }
        files.push(WebseedFile {
            relative_path: file
                .filename
                .to_pathbuf()
                .context("filename inválido no torrent")?,
            length: file.len,
            is_padding: file.attrs().padding,
        });
    }
    Ok(files)
}

/// URL de download de um arquivo (BEP 19 GetRight):
/// - multi-file: `base + name + '/' + caminho_relativo` (o `name` é o subfolder);
/// - single-file: `base + name` (`name` é o próprio arquivo, sem subfolder).
pub(crate) fn file_url(
    base: &str,
    config: &WebseedConfig,
    file: &WebseedFile,
) -> anyhow::Result<url::Url> {
    let base = base.trim_end_matches('/');
    let mut url = url::Url::parse(base).context("base webseed inválida")?;
    // single-file: `relative_path == name` (o subfolder não existe).
    let is_single = is_single_file(&config.name, &file.relative_path);
    let path_text = if is_single {
        config.name.to_string()
    } else {
        format!(
            "{}/{path}",
            config.name,
            path = relative_path_text(&file.relative_path)
        )
    };
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("base webseed sem path válido"))?
        .extend(path_text.split('/'));
    Ok(url)
}

/// true quando o torrent é single-file (sem subfolder): o caminho do arquivo é
/// exatamente o `name` (o próprio arquivo).
fn is_single_file(name: &str, relative_path: &Path) -> bool {
    relative_path == Path::new(name)
        || relative_path
            .components()
            .filter(|c| matches!(c, std::path::Component::Normal(_)))
            .count()
            == 1
            && relative_path.file_name().and_then(|n| n.to_str()) == Some(name)
}

/// Texto do caminho relativo com separadores `/`.
fn relative_path_text(path: &Path) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for part in path.components() {
        let s = part.as_os_str().to_string_lossy();
        let _ = write!(out, "{s}/");
    }
    out.pop();
    out
}

/// Executa o download webseed de um torrent no layout final de saída (US-049).
///
/// - tenta as bases em ordem para cada arquivo (FR-003);
/// - seguir redirects, aceita `206`/`200` e valida o tamanho (FR-004);
/// - padding/size-0 criados sem HTTP (FR-008);
/// - arquivo já completo no disco é pulado (idempotência por peça);
/// - erro parcial não aborta os demais (FR-009);
/// - progresso por callback chamada em intervalo fixo (sem locks held; o
///   chamador decide o destino — `MultiProgress` no CLI, registro no daemon).
///
/// # Errors
///
/// `Err` apenas em falha estrutural (sem bases; diretório de saída inválido).
/// Arquivos individuais com falha não abortam os demais (registrados no estado
/// `Error` do progresso).
pub(crate) async fn download_webseed<F>(
    config: &WebseedConfig,
    files: Vec<WebseedFile>,
    output_folder: &Path,
    concurrency: usize,
    progress: F,
) -> anyhow::Result<WebseedProgress>
where
    F: Fn(&WebseedProgress) + Send + Sync + 'static,
{
    if config.bases.is_empty() {
        bail!("sem bases webseed para download");
    }
    let total_bytes: u64 = files
        .iter()
        .filter(|f| !f.is_padding)
        .map(|f| f.length)
        .sum();
    let total_files = files.iter().filter(|f| !f.is_padding).count();

    let shared = Arc::new(SharedProgress::new(total_bytes, total_files));
    let semaphore = Arc::new(Semaphore::new(concurrency.max(1)));

    let mut tasks = Vec::with_capacity(files.len());
    for file in files {
        let semaphore = semaphore.clone();
        let bases = config.bases.clone();
        let state = shared.clone();
        let output_folder = output_folder.to_path_buf();
        let name = config.name.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = semaphore.acquire().await.expect("semáforo não fechado");
            match download_file(&name, &file, &bases, &output_folder).await {
                Ok(()) => state.mark_done(&file, None),
                Err(err) => state.mark_done(&file, Some(format!("{err:#}"))),
            }
        }));
    }

    // Publisher de progresso: a cada ~200ms captura o estado compartilhado e
    // chama o callback do chamador (CLI barra / daemon registro).
    let tick = tokio::task::spawn_blocking({
        let state = shared.clone();
        let progress = Arc::new(progress);
        move || {
            loop {
                let done = state.all_done();
                progress(&state.snapshot());
                if done {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    });

    let joined = futures::future::join_all(tasks).await;
    let _ = joined.into_iter().collect::<Result<Vec<_>, _>>();
    let _ = tick.await;
    // O download nunca aborta por falha parcial: o progresso final carrega o
    // estado (`Done` com tudo ok, ou `Error` com a mensagem do primeiro
    // arquivo que falhou) para o chamador decidir.
    Ok(shared.snapshot())
}

/// Estado compartilhado do progresso de uma tarefa webseed, atualizado pelas
/// tasks de download e lido pelo publisher de progresso.
struct SharedProgress {
    /// bytes baixados acumulados (arquivos não-padding).
    downloaded: Arc<std::sync::atomic::AtomicU64>,
    /// arquivos não-padding concluídos ou com erro.
    done_files: Arc<std::sync::atomic::AtomicUsize>,
    /// first error de arquivo registrado (para o estado final).
    error: Mutex<Option<String>>,
    total_bytes: u64,
    total_files: usize,
}

impl SharedProgress {
    fn new(total_bytes: u64, total_files: usize) -> Self {
        Self {
            downloaded: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            done_files: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            error: Mutex::new(None),
            total_bytes,
            total_files,
        }
    }

    fn mark_done(&self, file: &WebseedFile, err: Option<String>) {
        if let Some(err) = err {
            let mut e = self.error.lock().expect("error lock");
            if e.is_none() {
                *e = Some(err);
            }
        }
        if !file.is_padding {
            let _ = self
                .done_files
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn all_done(&self) -> bool {
        self.done_files.load(std::sync::atomic::Ordering::Relaxed) >= self.total_files
    }

    fn snapshot(&self) -> WebseedProgress {
        let downloaded = self.downloaded.load(std::sync::atomic::Ordering::Relaxed);
        let done = self.done_files.load(std::sync::atomic::Ordering::Relaxed);
        let state = match &*self.error.lock().expect("error lock") {
            Some(e) => WebseedState::Error(e.clone()),
            None if done >= self.total_files && self.total_files > 0 => WebseedState::Done,
            None => WebseedState::Downloading,
        };
        WebseedProgress {
            total_bytes: self.total_bytes,
            downloaded_bytes: downloaded,
            total_files: self.total_files,
            done_files: done,
            state,
        }
    }
}

/// Baixa um único arquivo via HTTP com Range, tentando as bases em ordem.
async fn download_file(
    name: &str,
    file: &WebseedFile,
    bases: &[String],
    output_folder: &Path,
) -> anyhow::Result<()> {
    let dest = output_folder.join(&file.relative_path);

    // Arquivo já completo no disco → pulado (idempotência por peça).
    if !file.is_padding
        && let Ok(meta) = tokio::fs::metadata(&dest).await
        && meta.len() == file.length
    {
        return Ok(());
    }

    // Padding e arquivos de tamanho 0: criar sem HTTP (FR-008).
    if file.is_padding || file.length == 0 {
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("falha ao criar diretório {}", parent.display()))?;
        }
        let f = tokio::fs::File::create(&dest)
            .await
            .with_context(|| format!("falha ao criar {}", dest.display()))?;
        if file.length > 0 {
            f.set_len(file.length)
                .await
                .with_context(|| format!("falha ao definir tamanho de {}", dest.display()))?;
        }
        return Ok(());
    }

    let config = WebseedConfig {
        bases: bases.to_vec(),
        name: name.to_string(),
    };
    let mut last_err: Option<anyhow::Error> = None;
    for base in bases {
        match download_file_from_base(base, &config, file, &dest).await {
            Ok(()) => return Ok(()),
            Err(err) => last_err = Some(err),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("nenhuma base webseed disponível")))
}

/// Baixa um arquivo de uma única base, gravando em streaming (`part` + rename)
/// para não segurar bytes grandes em memória, e valida o tamanho total.
async fn download_file_from_base(
    base: &str,
    config: &WebseedConfig,
    file: &WebseedFile,
    dest: &Path,
) -> anyhow::Result<()> {
    use futures::stream::StreamExt as _;
    use tokio::io::AsyncWriteExt as _;

    let url = file_url(base, config, file)?;
    let range_end = file.length.saturating_sub(1);
    let range = format!("bytes=0-{range_end}");
    let response = crate::resolve::http_client()
        .get(url.clone())
        .header(reqwest::header::RANGE, range)
        .send()
        .await
        .with_context(|| format!("falha ao baixar {url}"))?
        .error_for_status()
        .with_context(|| format!("{url} respondeu erro HTTP"))?;

    let status = response.status();
    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        let full = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_content_range_total)
            .unwrap_or(file.length);
        if full != file.length {
            bail!(
                "tamanho divergente para {} (content-range {full} != {} do torrent)",
                file.relative_path.display(),
                file.length
            );
        }
    } else if let Some(len) = response.content_length()
        && len != file.length
    {
        bail!(
            "tamanho divergente para {} ({} != {} do torrent)",
            file.relative_path.display(),
            len,
            file.length
        );
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("falha ao criar diretório {}", parent.display()))?;
    }
    let tmp = dest.with_file_name(format!(
        ".{}.part{}",
        dest.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
        std::process::id()
    ));
    let _ = tokio::fs::remove_file(&tmp).await;
    let mut out = tokio::fs::File::create(&tmp)
        .await
        .with_context(|| format!("falha ao criar {}", tmp.display()))?;
    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("falha ao ler corpo de {url}"))?;
        out.write_all(&chunk)
            .await
            .with_context(|| format!("falha ao gravar {}", tmp.display()))?;
        written += chunk.len() as u64;
    }
    if written != file.length {
        bail!(
            "tamanho divergente para {} ({} baixado != {} do torrent)",
            file.relative_path.display(),
            written,
            file.length
        );
    }
    tokio::fs::rename(&tmp, dest)
        .await
        .with_context(|| format!("falha ao finalizar {}", dest.display()))?;
    Ok(())
}

/// Extrai o tamanho total de um header `content-range: bytes 0-1023/4096`.
fn parse_content_range_total(value: &str) -> Option<u64> {
    value.rsplit('/').next()?.parse().ok()
}

/// Caminho de output para um teste/regressão (diretório temporário exclusivo).
#[cfg(test)]
fn test_output_folder(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "opentorrent-webseed-{label}-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Write as _;

    use super::*;

    const SAMPLE: &[u8] = b"sample data for webseed";

    fn config(base: String) -> WebseedConfig {
        WebseedConfig {
            bases: vec![base],
            name: "linuxmint-collection".to_string(),
        }
    }

    fn file(relative: &str, len: u64) -> WebseedFile {
        WebseedFile {
            relative_path: PathBuf::from(relative),
            length: len,
            is_padding: false,
        }
    }

    /// Servidor HTTP local mínimo que serve respostas fixas por rota, com
    /// suporte a Range (`206`), redirect e resposta sem Range (`200`).
    struct TestServer {
        addr: std::net::SocketAddr,
    }

    struct Route {
        body: Vec<u8>,
        // true → sempre `200` ignorando Range.
        ignore_range: bool,
    }

    impl TestServer {
        fn start<const N: usize>(routes: [(&'static str, Vec<u8>); N]) -> Self {
            let routes: HashMap<String, Route> = routes
                .into_iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        Route {
                            body: v,
                            ignore_range: false,
                        },
                    )
                })
                .collect();
            Self::start_with_routes(routes)
        }

        fn start_with_routes(routes: HashMap<String, Route>) -> Self {
            let routes = Arc::new(routes);
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("rt");
                rt.block_on(async move {
                    let listener =
                        tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
                    let _ = tx.send(listener.local_addr().expect("addr"));
                    loop {
                        let (mut socket, _) = listener.accept().await.expect("accept");
                        let routes = routes.clone();
                        tokio::spawn(async move {
                            use tokio::io::{AsyncReadExt, AsyncWriteExt};
                            let mut buf = Vec::new();
                            let mut tmp = [0u8; 1024];
                            loop {
                                match socket.read(&mut tmp).await {
                                    Ok(0) | Err(_) => break,
                                    Ok(n) => {
                                        buf.extend_from_slice(&tmp[..n]);
                                        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                                            break;
                                        }
                                    }
                                }
                            }
                            let request = String::from_utf8_lossy(&buf);
                            let path = request
                                .lines()
                                .next()
                                .and_then(|l| l.split_whitespace().nth(1))
                                .unwrap_or("/")
                                .to_string();
                            let range = request
                                .lines()
                                .find(|l| l.to_ascii_lowercase().starts_with("range:"))
                                .and_then(|l| l.split(':').nth(1))
                                .map(|s| s.trim().to_string());
                            let route = routes.get(&path);
                            let body = route.map(|r| r.body.clone()).unwrap_or_default();
                            if body.is_empty() {
                                let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                                let _ = socket.write_all(resp.as_bytes()).await;
                                return;
                            }
                            if route.map(|r| r.ignore_range).unwrap_or(false) {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    body.len()
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                                let _ = socket.write_all(&body).await;
                                return;
                            }
                            if let Some(range) = range {
                                // `bytes=start-end`
                                if let Some(r) = range.strip_prefix("bytes=") {
                                    let parts: Vec<&str> = r.split('-').collect();
                                    if let (Some(start), end) = (
                                        parts.first().and_then(|s| s.parse::<usize>().ok()),
                                        parts.get(1).and_then(|s| s.parse::<usize>().ok()),
                                    ) {
                                        let end = end.unwrap_or(body.len() - 1);
                                        let slice = body.get(start..=end).unwrap_or(&body[start..]);
                                        let resp = format!(
                                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                                            slice.len(),
                                            start,
                                            start + slice.len() - 1,
                                            body.len()
                                        );
                                        let _ = socket.write_all(resp.as_bytes()).await;
                                        let _ = socket.write_all(slice).await;
                                        return;
                                    }
                                }
                            }
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            let _ = socket.write_all(resp.as_bytes()).await;
                            let _ = socket.write_all(&body).await;
                        });
                    }
                });
            });
            Self {
                addr: rx.recv().expect("server addr"),
            }
        }
    }

    // --- extract_url_list ---

    fn bencode_for_urls(urls: &[&str]) -> Vec<u8> {
        let mut d = vec![];
        d.push(b'd');
        if !urls.is_empty() {
            d.extend_from_slice(b"8:url-list");
            d.push(b'l');
            for u in urls {
                let bt = u.as_bytes();
                d.extend_from_slice(format!("{}:", bt.len()).as_bytes());
                d.extend_from_slice(bt);
            }
            d.push(b'e');
        }
        d.push(b'e');
        d
    }

    #[test]
    fn extract_url_list_parses_list_of_bases() {
        let bytes = bencode_for_urls(&["https://archive.org/download/", "http://x/items/"]);
        assert_eq!(
            extract_url_list(&bytes),
            Some(vec![
                "https://archive.org/download/".to_string(),
                "http://x/items/".to_string()
            ])
        );
    }

    #[test]
    fn extract_url_list_accepts_single_string() {
        let bytes = bencode_for_urls(&["https://a.com/download/"]);
        assert_eq!(
            extract_url_list(&bytes),
            Some(vec!["https://a.com/download/".to_string()])
        );
    }

    #[test]
    fn extract_url_list_none_without_field() {
        let bytes = bencode_for_urls(&[]);
        assert_eq!(extract_url_list(&bytes), None);
    }

    #[test]
    fn extract_url_list_filters_non_getright_bases() {
        // URL sem barra final → não é base GetRight → None (peers).
        let bytes = bencode_for_urls(&["https://a.com/file.iso"]);
        assert_eq!(extract_url_list(&bytes), None);
    }

    #[test]
    fn extract_url_list_ignores_unknown_fields() {
        // Dicionário com um campo extra (`announce`) antes do `url-list`: o
        // parser deve pular o campo desconhecido e achar `url-list`.
        let announce = b"udp://tracker.example/announce";
        let mut d = vec![];
        d.push(b'd');
        d.extend_from_slice(b"8:announce");
        d.extend_from_slice(format!("{}:", announce.len()).as_bytes());
        d.extend_from_slice(announce);
        d.extend_from_slice(b"8:url-list");
        d.push(b'l');
        d.extend_from_slice(b"23:https://a.com/download/");
        d.push(b'e');
        d.push(b'e');
        assert_eq!(
            extract_url_list(&d),
            Some(vec!["https://a.com/download/".to_string()])
        );
    }

    #[test]
    fn extract_url_list_parses_real_mint_torrent_when_present() {
        // Fixture real (archive.org): validação end-to-end da extração; quando
        // ausente, o teste é pulado (ambientes sem o fixture).
        let path = std::path::Path::new("/tmp/ottest/mint.torrent");
        let Ok(bytes) = std::fs::read(path) else {
            eprintln!("fixture /tmp/ottest/mint.torrent ausente — teste pulado");
            return;
        };
        let bases = extract_url_list(&bytes).expect("url-list do mint");
        assert_eq!(bases.len(), 3);
        assert!(bases[0].starts_with("https://archive.org/download/"));
    }

    // --- build_webseed_files ---

    #[test]
    fn build_webseed_files_derives_from_real_info() {
        // Cria um .torrent real (single-file) via librqbit.create_torrent e
        // valida que build_webseed_files produz 1 arquivo não-padding.
        let dir = test_output_folder("buildfiles");
        let data = dir.join("data.bin");
        let mut f = std::fs::File::create(&data).unwrap();
        f.write_all(SAMPLE).unwrap();
        drop(f);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let info: TorrentMetaV1Info<ByteBufOwned> = rt
            .block_on(librqbit::create_torrent(
                &data,
                librqbit::CreateTorrentOptions::default(),
            ))
            .unwrap()
            .as_info()
            .info
            .clone();
        let files = build_webseed_files(&info, None).unwrap();
        assert_eq!(files.len(), 1);
        assert!(!files[0].is_padding);
        assert_eq!(files[0].length, SAMPLE.len() as u64);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- file_url ---

    #[test]
    fn file_url_multi_file_uses_name_and_encoded_path() {
        let cfg = config("https://archive.org/download/".to_string());
        let f = file("20.1/x iso.bin", 10);
        let url = file_url("https://archive.org/download/", &cfg, &f).unwrap();
        assert_eq!(
            url.as_str(),
            "https://archive.org/download/linuxmint-collection/20.1/x%20iso.bin"
        );
    }

    #[test]
    fn file_url_single_file_uses_name_without_subfolder() {
        // Torrent single-file: `info.name` == nome do arquivo (sem subfolder).
        let cfg = WebseedConfig {
            bases: vec!["https://archive.org/download/".to_string()],
            name: "single.iso".to_string(),
        };
        let f = file("single.iso", 10);
        let url = file_url("https://archive.org/download/", &cfg, &f).unwrap();
        assert_eq!(url.as_str(), "https://archive.org/download/single.iso");
    }

    // --- download_webseed ---

    #[test]
    fn download_webseed_writes_files_and_progress() {
        let server = TestServer::start([("/linuxmint-collection/a.txt", SAMPLE.to_vec())]);
        let sized = SAMPLE.to_vec();
        let base = format!("http://{}/", server.addr);
        let cfg = config(base);
        let dir = test_output_folder("writes");
        let progress = Arc::new(Mutex::new(WebseedProgress::default()));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let p = progress.clone();
        let out = dir.clone();
        let cfg2 = cfg.clone();
        rt.block_on(async move {
            crate::webseed::download_webseed(
                &cfg2,
                vec![file("a.txt", sized.len() as u64)],
                &out,
                4,
                move |s| {
                    let mut g = p.lock().unwrap();
                    *g = s.clone();
                },
            )
            .await
            .expect("download webseed");
        });
        let written = std::fs::read(dir.join("a.txt")).expect("arquivo gravado");
        assert_eq!(written, SAMPLE);
        let p = progress.lock().unwrap();
        assert_eq!(p.done_files, 1);
        assert_eq!(p.state, WebseedState::Done);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_webseed_falls_back_to_second_base() {
        let server = TestServer::start([("/linuxmint-collection/b.txt", SAMPLE.to_vec())]);
        let good_base = format!("http://{}/", server.addr);
        let cfg = WebseedConfig {
            bases: vec!["http://127.0.0.1:1/".to_string(), good_base],
            name: "linuxmint-collection".to_string(),
        };
        let dir = test_output_folder("fallback");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let out = dir.clone();
        rt.block_on(async move {
            crate::webseed::download_webseed(
                &cfg,
                vec![file("b.txt", SAMPLE.len() as u64)],
                &out,
                4,
                |_| {},
            )
            .await
            .expect("download webseed");
        });
        let written = std::fs::read(dir.join("b.txt")).expect("arquivo gravado");
        assert_eq!(written, SAMPLE);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_webseed_skips_existing_complete_file() {
        let server = TestServer::start([("/linuxmint-collection/c.txt", SAMPLE.to_vec())]);
        let cfg = config(format!("http://{}/", server.addr));
        let dir = test_output_folder("skip");
        std::fs::write(dir.join("c.txt"), SAMPLE).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let out = dir.clone();
        rt.block_on(async move {
            crate::webseed::download_webseed(
                &cfg,
                vec![file("c.txt", SAMPLE.len() as u64)],
                &out,
                4,
                |_| {},
            )
            .await
            .expect("download webseed");
        });
        let written = std::fs::read(dir.join("c.txt")).expect("arquivo preservado");
        assert_eq!(written, SAMPLE);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_webseed_creates_padding_without_http() {
        let server = TestServer::start([
            ("/linuxmint-collection/pad.bin", SAMPLE.to_vec()),
            ("/linuxmint-collection/data.bin", SAMPLE.to_vec()),
        ]);
        let cfg = config(format!("http://{}/", server.addr));
        let dir = test_output_folder("padding");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let out = dir.clone();
        let progress = Arc::new(Mutex::new(WebseedProgress::default()));
        let p = progress.clone();
        rt.block_on(async move {
            let files = vec![
                WebseedFile {
                    relative_path: PathBuf::from("data.bin"),
                    length: SAMPLE.len() as u64,
                    is_padding: false,
                },
                WebseedFile {
                    relative_path: PathBuf::from(".____padding_file/1"),
                    length: 4096,
                    is_padding: true,
                },
            ];
            crate::webseed::download_webseed(&cfg, files, &out, 4, move |s| {
                *p.lock().unwrap() = s.clone();
            })
            .await
            .expect("download webseed");
        });
        let data = std::fs::read(dir.join("data.bin")).unwrap();
        assert_eq!(data, SAMPLE);
        let pad = std::fs::metadata(dir.join(".____padding_file/1")).unwrap();
        assert_eq!(pad.len(), 4096);
        let p = progress.lock().unwrap();
        // Padding não conta no progresso de arquivos.
        assert_eq!(p.done_files, 1);
        assert_eq!(p.total_files, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_webseed_accepts_200_when_range_ignored() {
        let mut routes = HashMap::new();
        routes.insert(
            "/linuxmint-collection/d.bin".to_string(),
            Route {
                body: SAMPLE.to_vec(),
                ignore_range: true,
            },
        );
        let server = TestServer::start_with_routes(routes);
        let cfg = config(format!("http://{}/", server.addr));
        let dir = test_output_folder("range200");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let out = dir.clone();
        rt.block_on(async move {
            crate::webseed::download_webseed(
                &cfg,
                vec![file("d.bin", SAMPLE.len() as u64)],
                &out,
                4,
                |_| {},
            )
            .await
            .expect("download webseed");
        });
        let written = std::fs::read(dir.join("d.bin")).unwrap();
        assert_eq!(written, SAMPLE);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_webseed_marks_partial_error() {
        // Apenas o arquivo inexistente → erro parcial; o download não aborta.
        let server = TestServer::start([("/linuxmint-collection/exists.bin", SAMPLE.to_vec())]);
        let cfg = config(format!("http://{}/", server.addr));
        let dir = test_output_folder("partial");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let out = dir.clone();
        let progress = Arc::new(Mutex::new(WebseedProgress::default()));
        let p = progress.clone();
        rt.block_on(async move {
            let files = vec![
                file("exists.bin", SAMPLE.len() as u64),
                file("missing.bin", 1234),
            ];
            crate::webseed::download_webseed(&cfg, files, &out, 4, move |s| {
                *p.lock().unwrap() = s.clone();
            })
            .await
            .expect("download webseed não aborta");
        });
        let exists = std::fs::read(dir.join("exists.bin")).unwrap();
        assert_eq!(exists, SAMPLE);
        assert!(!dir.join("missing.bin").exists());
        let p = progress.lock().unwrap();
        assert_eq!(p.done_files, 2);
        assert!(matches!(p.state, WebseedState::Error(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_webseed_rejects_divergent_content_length() {
        let server = TestServer::start([("/linuxmint-collection/e.bin", SAMPLE.to_vec())]);
        let cfg = config(format!("http://{}/", server.addr));
        let dir = test_output_folder("divergent");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let out = dir.clone();
        let progress = Arc::new(Mutex::new(WebseedProgress::default()));
        let p = progress.clone();
        rt.block_on(async move {
            crate::webseed::download_webseed(
                &cfg,
                vec![file("e.bin", SAMPLE.len() as u64 + 100)],
                &out,
                4,
                move |s| *p.lock().unwrap() = s.clone(),
            )
            .await
            .expect("download webseed não aborta");
        });
        assert!(!dir.join("e.bin").exists());
        let p = progress.lock().unwrap();
        assert!(matches!(p.state, WebseedState::Error(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_content_range_total_extracts_total() {
        assert_eq!(parse_content_range_total("bytes 0-1023/4096"), Some(4096));
        assert_eq!(parse_content_range_total("bytes */4096"), Some(4096));
        assert_eq!(parse_content_range_total("bytes 0-1"), None);
    }

    #[test]
    fn download_webseed_empty_files_list_completes() {
        let cfg = config("http://127.0.0.1:9/".to_string());
        let dir = test_output_folder("empty");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let out = dir.clone();
        rt.block_on(async move {
            crate::webseed::download_webseed(&cfg, vec![], &out, 4, |_| {})
                .await
                .unwrap();
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_webseed_creates_zero_length_file_without_http() {
        let cfg = config("http://127.0.0.1:9/".to_string());
        let dir = test_output_folder("zero");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let out = dir.clone();
        rt.block_on(async move {
            crate::webseed::download_webseed(&cfg, vec![file("empty.txt", 0)], &out, 4, |_| {})
                .await
                .expect("download webseed");
        });
        let meta = std::fs::metadata(dir.join("empty.txt")).unwrap();
        assert_eq!(meta.len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
