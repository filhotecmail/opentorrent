# Data Model: US-049 — Suporte a webseed (HTTP seeding) para torrents com url-list

Nenhum estado novo é persistido (os dados baixados via HTTP são o próprio conteúdo do torrent, no layout
final). Modelos em memória:

## `WebseedConfig` (module `src/webseed.rs`)

Configuração extraída do metainfo do `.torrent` (bencode bruto):

```rust
pub struct WebseedConfig {
    /// Bases GetRight (URLs terminando em `/`), na ordem do `url-list`.
    pub bases: Vec<String>,
    /// Nome do torrent (`info.name`) — usado no subfolder e no mapeamento de URL.
    pub name: String,
}
```

Validação (scope): apenas bases **GetRight** (terminam em `/`); `url-list` com strings diretas por arquivo
(sem barra final) é ignorado → segue para peers (assumption da spec).

## `WebseedFile` (module `src/webseed.rs`)

Um arquivo do torrent a baixar via HTTP:

```rust
pub struct WebseedFile {
    /// Caminho relativo ao subfolder (ex.: `20.1/linuxmint-....iso`).
    pub relative_path: PathBuf,
    pub length: u64,
    /// Padding (`attr = 'p'`) — criado como zero/espanso, sem download.
    pub is_padding: bool,
}
```

Derivado de `TorrentMetaV1Info::iter_file_details()` (via `librqbit`), filtrando por `only_files` quando
aplicável (CLI) e ignorando padding na contagem de progresso.

## `WebseedProgress` (module `src/webseed.rs`)

Progresso em tempo real de uma tarefa webseed, atualizado pela task background:

```rust
pub struct WebseedProgress {
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub total_files: usize,
    pub done_files: usize,
    /// Estado atual: baixando / concluído / erro de domínio.
    pub state: WebseedState,
}

pub enum WebseedState {
    Downloading,
    Done,
    Error(String),
}
```

## Snapshot IPC (module `src/ipc.rs`)

Exposto ao cliente via `DaemonState`, para a TUI renderizar as tarefas webseed em background:

```rust
pub struct WebseedSnapshot {
    /// Identificador da tarefa (gerado no daemon).
    pub id: usize,
    /// Origem exibida (URL/magnet/path) para casar com a linha pendente da TUI.
    pub source: String,
    pub name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub done_files: usize,
    pub total_files: usize,
    pub state: String, // "download" | "done" | "error: ..."
}
```

Adicionado como campo `webseed: Vec<WebseedSnapshot>` em `DaemonState` (com `#[serde(default)]`/`Default`
para compatibilidade evolutiva do protocolo). O `build_state` (daemon) concatena as tarefas ativas.

## Pipeline de execução

```text
dispatch(Add) { source }
  ├─ resolve_source(source)            // US-048: URL→bytes .torrent, magnet/path→CliArgument
  ├─ tem bytes .torrent?
  │     ├─ não ──► add_torrent direto (magnet/path, comportamento atual)
  │     └─ sim ── parse metainfo (librqbit) + extrai url-list (bencode bruto)
  │               ├─ sem url-list GetRight ──► add_torrent direto (FR-006)
  │               └─ com url-list ──► spawn task background:
  │                   ├─ calcula output_folder (default + subfolder do motor)
  │                   ├─ monta WebseedFile[] (only_files se CLI; skip padding)
  │                   ├─ p/ cada base (em ordem):
  │                   │    └─ p/ cada arquivo (semáforo):
  │                   │         ├─ Range GET base+name+path → 206/200, valida len
  │                   │         ├─ grava no layout final, atualiza WebseedProgress
  │                   │         └─ erro (404/timeout/len) → próxima base / marca erro parcial
  │                   ├─ padding/size-0 → cria arquivo sem download
  │                   └─ ao concluir → session.add_torrent(bytes resolvidos)
  │                       → initial_check valida pieces; peers completam o resto
  └─ responde Ok("adicionando via webseed")
```

## Tipos auxiliares

```rust
/// Extrai bases GetRight do `url-list` no bencode bruto (zero dep nova).
fn extract_url_list(bytes: &[u8]) -> Option<Vec<String>>;

/// Monta a URL de download de um arquivo: base + name + '/' + relative_path.
fn file_url(base: &str, config: &WebseedConfig, file: &WebseedFile) -> url::Url;

/// Executa o download webseed de um torrent (task background / CLI).
async fn download_webseed(
    client: &reqwest::Client,
    config: &WebseedConfig,
    files: Vec<WebseedFile>,
    output_folder: &Path,
    concurrency: usize,
    progress: impl Fn(&WebseedProgress) + Send + Sync + 'static,
) -> anyhow::Result<()>;
```

## Contratos

- `extract_url_list` devolve `None` para `url-list` ausente OU não-GetRight (sem barra final) — sinaliza
  "usar peers".
- `download_webseed` nunca apaga dados existentes: re-download sobrescreve apenas arquivos incompletos/
  divergentes (valida `len` antes de gravar); arquivos já completos são pulados.
- Ao fim (mesmo com erros parciais), a task sempre chama `session.add_torrent` com os bytes resolvidos
  (FR-005) — nunca deixa o torrent fora do motor quando há webseed.
- Os caminhos gravados replicam exatamente o layout do motor (`output_folder` + `relative_path`), para o
  `initial_check` encontrar os arquivos.
