# Data Model: US-048 — Suporte a links de download de .torrent e drag & drop no input

Nenhum estado novo é persistido. O fluxo de resolução é efêmero (em memória). Modelos:

## `ResolvedSource<'a>` (module `src/resolve.rs`)

Estado pós-resolução de uma origem, antes de entregar ao `librqbit`:

```rust
pub enum ResolvedSource<'a> {
    /// Magnet / infohash / caminho local (delegado a `AddTorrent::from_cli_argument`).
    CliArgument(Cow<'a, str>),
    /// Bytes já baixados e validados como `.torrent` (bencode+infohash).
    TorrentBytes(bytes::Bytes),
}

impl<'a> ResolvedSource<'a> {
    pub fn to_add_torrent(self) -> Result<AddTorrent<'a>, anyhow::Error>;
}
```

Conversão:
- `CliArgument(s)` → `AddTorrent::from_cli_argument(&s)?`
- `TorrentBytes(b)` → `AddTorrent::TorrentFileBytes(b)` (perde o lifetime — `Bytes` é owned).

## Pipeline de resolução

```text
source: String
  │
  ├─ http(s) URL? ── não ──► CliArgument(source)   // magnet / infohash / path
  │
  └─ sim ── download_for_torrent(url) async
            ├─ GET(url) → resp.bytes()
            ├─ torrent_from_bytes_ext(±)?.is_ok()
            │     ├─ sim → TorrentBytes(bytes)
            │     └─ não ── corpo parece HTML? ── não ──► Err("resposta não é um torrent")
            │                    └─ sim ── extract_torrent_links(html)
            │                              ├─ [candidatos]                     // URL absoluta(.torrent)
            │                              ├─ p/ cada: GET → torrent_from_bytes_ext
            │                              │     ├─ ok → TorrentBytes(bytes)
            │                              │     └─ falhou → próximo
            │                              └─ nenhum ok → Err("nenhum link .torrent encontrado")
            └─ erro de rede/HTTP → Err("falha ao baixar a URL: …")
```

## Tipos auxiliares

```rust
/// Candidato a link `.torrent` extraído do HTML (já absoluto).
struct TorrentLinkCandidate { url: url::Url }

/// Heurística de extração: procura atributos `href`/`location`/`window.location`
/// cujo valor, resolvido contra a URL base, termina em `.torrent`.
fn extract_torrent_links(html: &str, base: &url::Url) -> Vec<url::Url>;

/// Download de um payload http(s) com User-Agent padrão do app.
async fn fetch_bytes(url: &url::Url) -> anyhow::Result<bytes::Bytes>;
```

## Estados da entrada pendente (TUI, inalterados)

`PendingStatus::Resolving → Added (removida) | Done("já gerenciada") | Error(vermelho)`.
Uma URL de download resolvida entra como `Added`; falha → `Error` com a mensagem de domínio (ex.: "nenhum link .torrent encontrado na página").

## Contratos

- `resolve_source` nunca retorna para o chamador uma origem que ainda seja uma URL http(s) não resolvida: ou vira `TorrentBytes` (validado) ou `Err`.
- `to_add_torrent` não faz I/O de rede; recebe os bytes prontos.
- Magnet/infohash/caminho local preservam exatamente o comportamento atual (delegação a `from_cli_argument`).