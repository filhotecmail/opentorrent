//! Resolução de origens de torrent antes de entregar ao `librqbit` (US-048).
//!
//! O `librqbit` baixa URLs http(s) e tenta decodificar o corpo como `.torrent`;
//! quando a URL é uma página de download (HTML com redirect JS — ex.:
//! fosstorrents `thankyou`), o download retorna HTML e o decode falha
//! ("error decoding torrent"). Este módulo resolve a indireção: baixa o corpo,
//! detecta o tipo (`.torrent` direto vs HTML), e se HTML extrai o link
//! `.torrent` (`href`/`window.location`/`location.href`) e o valida por
//! bencode+infohash antes de entregar os bytes prontos ao `add_torrent`.

use anyhow::{Context, bail};
use librqbit::{AddTorrent, ByteBuf};

/// Cliente HTTP compartilhado para o pipeline de resolução. `reqwest` gerencia
/// o pool de conexões; um único client evita re-handshake por origem.
pub(crate) fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("opentorrent")
            .build()
            .expect("falha ao criar o cliente HTTP")
    })
}

/// Origem resolvida, pronta para virar `AddTorrent`.
#[derive(Debug)]
pub(crate) enum ResolvedSource<'a> {
    /// Magnet / infohash / caminho local — delega a
    /// `AddTorrent::from_cli_argument` (comportamento atual preservado).
    CliArgument(&'a str),
    /// Bytes baixados e validados como `.torrent` (bencode+infohash).
    TorrentBytes(bytes::Bytes),
}

impl<'a> ResolvedSource<'a> {
    /// Converte a origem resolvida em `AddTorrent` para o `librqbit`.
    pub(crate) fn into_add_torrent(self) -> anyhow::Result<AddTorrent<'a>> {
        match self {
            ResolvedSource::CliArgument(source) => AddTorrent::from_cli_argument(source),
            ResolvedSource::TorrentBytes(bytes) => Ok(AddTorrent::TorrentFileBytes(bytes)),
        }
    }
}

/// Baixa o corpo de uma URL e o retorna em memória.
async fn fetch_bytes(client: &reqwest::Client, url: &url::Url) -> anyhow::Result<bytes::Bytes> {
    let response = client
        .get(url.clone())
        .send()
        .await
        .with_context(|| format!("falha ao baixar a URL {url}"))?
        .error_for_status()
        .with_context(|| format!("a URL {url} respondeu um status de erro"))?;
    response
        .bytes()
        .await
        .with_context(|| format!("falha ao ler o corpo da URL {url}"))
}

/// Valida se `bytes` é um `.torrent` bencode com infohash calculável.
/// Usa o parser público do `librqbit` (`torrent_from_bytes_ext`), a mesma
/// validação aplicada internamente no `add_torrent`.
fn is_valid_torrent(bytes: &[u8]) -> bool {
    librqbit::torrent_from_bytes_ext::<ByteBuf>(bytes).is_ok()
}

/// Detecta se o corpo se parece com HTML (usado para decidir se vale extrair
/// links de download). Critério lax: começa com `<!doctype`/`<html`/`</html` ou
/// contém `<body`/`<a ` — suficiente para as páginas de "thankyou".
fn looks_like_html(body: &[u8]) -> bool {
    let head = &body[..body.len().min(8192)];
    let head = String::from_utf8_lossy(head);
    let lower = head.to_lowercase();
    lower.trim_start().starts_with("<!doctype")
        || lower.trim_start().starts_with("<html")
        || head.contains("<body")
        || head.contains("<a ")
        || head.contains("window.location")
        || head.contains("location.href")
}

/// Extrai candidatos a link `.torrent` do HTML. Procura por valores de
/// atributos/atribuições (`href=`, `window.location =`, `location.href =`),
/// resolve-join relativo contra a URL base e mantém somente os que terminam em
/// `.torrent`.
fn extract_torrent_links(html: &str, base: &url::Url) -> Vec<url::Url> {
    let candidates: Vec<url::Url> = ATTRIBUTE_MARKERS
        .iter()
        .flat_map(|marker| value_after_marker(html, marker))
        .filter_map(|value| resolve_candidate(value, base))
        .collect();

    let mut links: Vec<url::Url> = Vec::new();
    for url in candidates {
        // O `.torrent` pode estar no path (`/a.torrent`) ou na query
        // (`download.php?file=a.torrent` — case fosstorrents).
        let full = url.as_str();
        if full.ends_with(".torrent") && !links.contains(&url) {
            links.push(url);
        }
    }
    links
}

const ATTRIBUTE_MARKERS: &[&str] = &[
    "window.location = ",
    "window.location=",
    "location.href = ",
    "location.href=",
    "href=\"",
    "href='",
    "href=",
];

/// Pega o valor textual logo após um marcador de atributo de URL, até um
/// delimitador de HTML/string (`"`, `'`, `>`, espaço ou ponto-e-vírgula).
fn value_after_marker<'a>(html: &'a str, marker: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut search_from = 0;
    while let Some(idx) = html[search_from..].find(marker) {
        let start = search_from + idx + marker.len();
        let rest = &html[start..];
        // Pula a aspa de abertura quando o marcador não a inclui
        // (ex.: `window.location = "/files/a.torrent"`).
        let rest = rest.trim_start_matches(['"', '\'']);
        let end = rest
            .find(['"', '\'', '>', '<', '\n', '\r', ' ', ';'])
            .unwrap_or(rest.len());
        let value = rest[..end].trim();
        if !value.is_empty() {
            out.push(value);
        }
        search_from = start + 1;
    }
    out
}

/// Resolve um candidato contra a URL base, aceitando caminhos relativos.
fn resolve_candidate(value: &str, base: &url::Url) -> Option<url::Url> {
    if value.contains("://") {
        url::Url::parse(value).ok()
    } else {
        base.join(value).ok()
    }
}

/// Baixa uma URL e tenta resolvê-la em bytes `.torrent` válidos.
///
/// - corpo `.torrent` → `Ok(Some(bytes))`;
/// - corpo HTML → extrai links `.torrent`, baixa e valida cada um (primeiro
///   ok vence); nenhum ok → `Err` de domínio;
/// - outro corpo → `Err("resposta não é um torrent")`.
async fn download_for_torrent(
    client: &reqwest::Client,
    url: &url::Url,
) -> anyhow::Result<Option<bytes::Bytes>> {
    let body = fetch_bytes(client, url).await?;

    if is_valid_torrent(&body) {
        return Ok(Some(body));
    }

    if !looks_like_html(&body) {
        bail!("resposta não é um torrent");
    }

    let html = String::from_utf8_lossy(&body);
    let links = extract_torrent_links(&html, url);
    let mut last_err: Option<anyhow::Error> = None;
    for link in links {
        match fetch_bytes(client, &link).await {
            Ok(bytes) if is_valid_torrent(&bytes) => return Ok(Some(bytes)),
            Ok(_) => {
                last_err = Some(anyhow::anyhow!(
                    "o link {link} não contém um .torrent válido"
                ));
            }
            Err(err) => last_err = Some(err),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("nenhum link .torrent encontrado na página")))
}

/// Resolve uma origem de torrent para o `librqbit` (US-048).
///
/// - Magnet (`magnet:`), infohash de 40 hex ou caminho local → delega (sem
///   I/O de rede);
/// - URL http(s) → baixa e valida; HTML com link `.torrent` é seguido.
///
/// # Errors
///
/// Falha de rede/HTTP, resposta que não é torrent nem HTML com link `.torrent`
/// (mensagens de domínio para exibição na TUI).
pub(crate) async fn resolve_source<'a>(source: &'a str) -> anyhow::Result<ResolvedSource<'a>> {
    let is_http = source.starts_with("http://") || source.starts_with("https://");
    if is_http {
        let url = url::Url::parse(source).with_context(|| format!("URL inválida: {source}"))?;
        let resolved = download_for_torrent(http_client(), &url).await?;
        return Ok(ResolvedSource::TorrentBytes(
            resolved.expect("download_for_torrent sempre retorna Some ou Err"),
        ));
    }
    Ok(ResolvedSource::CliArgument(source))
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    /// Servidor HTTP local mínimo que serve respostas fixas por rota.
    struct TestServer {
        addr: std::net::SocketAddr,
    }

    impl TestServer {
        fn start(routes: &[(&str, Vec<u8>)]) -> Self {
            let routes: std::collections::HashMap<String, Vec<u8>> = routes
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();
            let routes = std::sync::Arc::new(routes);
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn({
                let routes = routes.clone();
                move || {
                    let rt = tokio::runtime::Runtime::new().expect("rt");
                    rt.block_on(async move {
                        let listener =
                            tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
                        let _ = tx.send(listener.local_addr().expect("addr"));
                        loop {
                            let (mut socket, _) = listener.accept().await.expect("accept");
                            let routes = routes.clone();
                            tokio::spawn(async move {
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
                                let path = String::from_utf8_lossy(&buf)
                                    .lines()
                                    .next()
                                    .and_then(|l| l.split_whitespace().nth(1))
                                    .unwrap_or("/")
                                    .to_string();
                                let body = routes.get(&path).cloned().unwrap_or_default();
                                let status = if body.is_empty() { "404 Not Found" } else { "200 OK" };
                                let response = format!(
                                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    body.len()
                                );
                                let _ = socket
                                    .write_all(response.as_bytes())
                                    .await;
                                let _ = socket.write_all(&body).await;
                            });
                        }
                    });
                }
            });
            let addr = rx.recv().expect("server addr");
            Self { addr }
        }
    }

    fn sample_html() -> &'static str {
        r#"<html><head><title>Thank You</title>
        <script type="text/javascript">function startDownload() {window.location = "/files/download.php?file=arcoplasma-v25.05.01-x86_64.iso.torrent";}</script>
        </head><body onload="startDownload();">
        <a href="https://fosstorrents.com/files/download.php?file=alt.torrent">retry</a>
        </body></html>"#
    }

    fn base_url() -> url::Url {
        url::Url::parse("https://fosstorrents.com/thankyou/?name=arcoplasma&id=1").unwrap()
    }

    fn smallest_torrent_bytes() -> bytes::Bytes {
        static BYTES: std::sync::OnceLock<bytes::Bytes> = std::sync::OnceLock::new();
        BYTES
            .get_or_init(|| {
                static COUNTER: AtomicUsize = AtomicUsize::new(0);
                let dir = std::env::temp_dir().join(format!(
                    "opentorrent-resolve-test-{}-{}",
                    std::process::id(),
                    COUNTER.fetch_add(1, Ordering::SeqCst)
                ));
                std::fs::create_dir_all(&dir).expect("create temp dir");
                let file = dir.join("data.bin");
                std::fs::File::create(&file)
                    .expect("create fixture file")
                    .write_all(&[0u8; 1024])
                    .expect("write fixture");
                // `create_torrent` usa `spawn_block_in_place`: precisa de um
                // runtime current-thread dedicado (e não dentro de outro runtime).
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("current-thread runtime");
                rt.block_on(librqbit::create_torrent(
                    &file,
                    librqbit::CreateTorrentOptions::default(),
                ))
                .expect("create torrent")
                .as_bytes()
                .expect("serialize torrent")
            })
            .clone()
    }

    #[test]
    fn extract_torrent_links_picks_window_location_and_href() {
        let links = extract_torrent_links(sample_html(), &base_url());
        assert_eq!(links.len(), 2);
        assert!(links.iter().any(|l| l.as_str().ends_with(".iso.torrent")));
        assert!(links.iter().any(|l| l.as_str().ends_with("alt.torrent")));
        assert!(links.iter().all(|l| l.as_str().ends_with(".torrent")));
    }

    #[test]
    fn extract_torrent_links_resolves_relative_paths() {
        let html = r#"<a href="/files/download.php?file=x.torrent">dl</a>"#;
        let base = base_url();
        let links = extract_torrent_links(html, &base);
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0].as_str(),
            "https://fosstorrents.com/files/download.php?file=x.torrent"
        );
    }

    #[test]
    fn extract_torrent_links_dedupes_and_filters_non_torrent() {
        let html = r#"<a href="/files/a.torrent">a</a><a href="/files/a.torrent">dup</a><a href="/files/image.png">img</a>"#;
        let base = base_url();
        assert_eq!(extract_torrent_links(html, &base).len(), 1);
    }

    #[test]
    fn extract_torrent_links_empty_without_torrent_href() {
        let html = r#"<html><body>nope</body></html>"#;
        assert!(extract_torrent_links(html, &base_url()).is_empty());
    }

    #[test]
    fn looks_like_html_detects_doctype_and_body() {
        assert!(looks_like_html(b"<!DOCTYPE html><html>"));
        assert!(looks_like_html(b"<html><body>x</body></html>"));
        assert!(looks_like_html(b"<a href=\"/x\">"));
        assert!(!looks_like_html(b"d8:announce36:udp://tracker/announce"));
    }

    #[test]
    fn is_valid_torrent_accepts_bencode_and_rejects_garbage() {
        let good = smallest_torrent_bytes();
        assert!(is_valid_torrent(&good));
        assert!(!is_valid_torrent(b"<html>not a torrent</html>"));
        assert!(!is_valid_torrent(b"387a0nb"));
    }

    #[test]
    fn resolve_source_delegates_local_path_and_magnet() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let src = rt
            .block_on(resolve_source("/tmp/nonexistent.torrent"))
            .unwrap();
        assert!(matches!(src, ResolvedSource::CliArgument(_)));
        let magnet = rt
            .block_on(resolve_source(
                "magnet:?xt=urn:btih:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ))
            .unwrap();
        assert!(matches!(magnet, ResolvedSource::CliArgument(_)));
    }

    #[test]
    fn resolve_source_rejects_invalid_percent_encoding_url() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(resolve_source("https://example.com/%zz"));
        assert!(result.is_err(), "esperava erro, veio Ok");
    }

    #[test]
    fn into_add_torrent_roundtrip_cli_argument() {
        let src = ResolvedSource::CliArgument(
            "magnet:?xt=urn:btih:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        );
        let add = src.into_add_torrent().expect("to AddTorrent");
        assert!(matches!(add, AddTorrent::Url(_)));
    }

    #[test]
    fn into_add_torrent_roundtrip_bytes() {
        let bytes = smallest_torrent_bytes();
        let src = ResolvedSource::TorrentBytes(bytes.clone());
        let add = src.into_add_torrent().expect("to AddTorrent");
        match add {
            AddTorrent::TorrentFileBytes(b) => assert_eq!(b, bytes),
            other => {
                let _ = other;
                panic!("esperava TorrentFileBytes");
            }
        }
    }

    #[test]
    fn value_after_marker_handles_quotes_and_plain() {
        assert_eq!(
            value_after_marker(
                "window.location = '/files/a.torrent';",
                "window.location = "
            ),
            vec!["/files/a.torrent"]
        );
        assert_eq!(
            value_after_marker(&format!(r#"href="{}""#, "/x.torrent"), "href=\""),
            vec!["/x.torrent"]
        );
    }

    #[test]
    fn resolve_candidate_joins_relative() {
        let base = base_url();
        assert_eq!(
            resolve_candidate("/files/a.torrent", &base)
                .unwrap()
                .as_str(),
            "https://fosstorrents.com/files/a.torrent"
        );
    }

    #[tokio::test]
    async fn resolve_source_downloads_direct_torrent_from_url() {
        let torrent = smallest_torrent_bytes();
        let server = TestServer::start(&[("/t", torrent.to_vec())]);
        let url = format!("http://{}/t", server.addr);
        match resolve_source(&url).await.expect("resolve") {
            ResolvedSource::TorrentBytes(bytes) => assert_eq!(bytes, torrent),
            other => panic!("esperava TorrentBytes, veio {other:?}"),
        }
    }

    #[tokio::test]
    async fn resolve_source_follows_html_link_to_torrent() {
        let torrent = smallest_torrent_bytes();
        let html = r#"<html><body onload="window.location = '/files/download.php?file=x.torrent'">
            <a href="/files/download.php?file=x.torrent">download</a></body></html>"#;
        let server = TestServer::start(&[
            ("/page", html.as_bytes().to_vec()),
            ("/files/download.php?file=x.torrent", torrent.to_vec()),
        ]);
        let url = format!("http://{}/page", server.addr);
        match resolve_source(&url).await.expect("resolve") {
            ResolvedSource::TorrentBytes(bytes) => assert_eq!(bytes, torrent),
            other => panic!("esperava TorrentBytes, veio {other:?}"),
        }
    }

    #[tokio::test]
    async fn resolve_source_errors_on_html_without_torrent_link() {
        let html = r#"<html><body><a href="/nope">nothing here</a></body></html>"#;
        let server = TestServer::start(&[("/page", html.as_bytes().to_vec())]);
        let url = format!("http://{}/page", server.addr);
        let err = resolve_source(&url).await.expect_err("deve falhar");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("nenhum link .torrent"),
            "mensagem inesperada: {msg}"
        );
    }

    #[tokio::test]
    async fn resolve_source_errors_on_non_torrent_non_html_body() {
        let server = TestServer::start(&[("/img", b"\x89PNG\r\n\x1a\nbinary".to_vec())]);
        let url = format!("http://{}/img", server.addr);
        let err = resolve_source(&url).await.expect_err("deve falhar");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("não é um torrent"),
            "mensagem inesperada: {msg}"
        );
    }
}
