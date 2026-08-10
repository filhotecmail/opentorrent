# Data Model: US-040 — Daemon em background e deduplicação de torrents

Persistência nova: **nenhuma no disco além da já existente** (`SessionPersistenceConfig::Json` em
`~/.config/opentorrent`, US-009). Novos tipos de **IPC em memória** (JSON sobre socket Unix).

## Novos tipos (src/ipc.rs)

### `DaemonRequest` (enum, serializado JSON — request do cliente)

| Variante | Campos | Semântica |
|----------|--------|-----------|
| `Add` | `source: String` | Adiciona origem (magnet/URL/arquivo). Dedup por infohash no daemon. |
| `Pause` | `id: usize` | Pausa o torrent. |
| `Resume` | `id: usize` | Retoma o torrent. |
| `Stop` | `id: usize` | Remove da fila mantendo arquivos (`session.delete(id, false)`). |
| `Delete` | `id: usize` | Exclui definitivo (`session.delete(id, true)`). |
| `GetState` | — | Retorna snapshot atual da fila. |
| `Shutdown` | — | Pede ao daemon para encerrar (opcionais nesta US; ver quickstart). |

### `DaemonResponse` (enum, serializado JSON — resposta do daemon)

| Variante | Campos | Uso |
|----------|--------|-----|
| `Ok` | `message: String` | Sucesso incondicional (ex.: pause/resume/stop). |
| `AlreadyManaged` | — | Dedup: anuncio `já está na lista` (estado atual: notice). |
| `State` | `snapshot: DaemonState` | Resposta ao `GetState`. |
| `Error` | `message: String` | Erro propagado sem stack trace interno. |

### `DaemonState` (struct, snapshot da fila para a TUI renderizar)

Campos (espelham o que `SessionRow`/`render` já consumem):

```rust
struct DaemonState {
    torrents: Vec<TorrentSnapshot>,
    finished: usize,          // nº de completos
    downloaded_total: u64,    // bytes totais baixados na sessão
}
struct TorrentSnapshot {
    id: usize,
    name: String,
    state: String,            // serializa TorrentStatsState (Debug)
    progress_bytes: u64,
    total_bytes: u64,
    finished: bool,
    down_speed_mbps: Option<f64>,
    up_speed_mbps: Option<f64>,
    eta_seconds: Option<u64>,
    uploaded_bytes: u64,
}
```

**Decisão**: `TorrentSnapshot` usa `String` para `state` (serialização simples pelo `Debug` do
`TorrentStatsState`) e números escalares — o cliente não precisa de `ManagedTorrent` para renderizar.
`info_hash` pode ser adicionado ao snapshot para exibir/verificar dedup, se desejado (follow-up).

## Alterações em src/session_ui.rs

- `Tui` ganha um cliente de IPC no lugar de (ou além de) `session: Arc<Session>`:
  - Campos novos: `client: IpcClient` (stream Unix), `snapshot: DaemonState` (atualizado a cada frame/na
    taxa do orçamento).
  - `session.with_torrents(...)` (linha 970) → alimentado pelo snapshot recebido. Ações
    `pause_row()/resume_row()/remove_row()/request_delete()` → enviam `DaemonRequest` e aguardam resposta.
  - `add_torrent_background` (linha 2101) → `DaemonRequest::Add`; `AddOutcome::AlreadyManaged` ganha a
    condição de "somente se não foi excluído da lista" (o daemon é quem decide, pois conhece a sessão).
- `PendingTorrent`/`PendingStatus` mantidos (origens ainda não confirmadas na fila).

## Alterações em src/main.rs

- `Opts` ganha subcomando `Daemon(DaemonOpts)`. `DaemonOpts`: `#[command(subcommand)]` → `Start`/`Stop`/
  `Status`.

## Invariantes

- Só existe **um** processo de daemon por usuário (lock do pidfile + socket ativo).
- A **sessão do librqbit vive exclusivamente no processo do daemon** quando o daemon está ativo; a TUI
  nunca instancia `Session`.
- Dedup: `Add` de um infohash já presente → `DaemonResponse::AlreadyManaged` (sem duplicação).
- Excluir (X/Delete): `Stop/Delete` remove o torrent da sessão; `Stop` mantém arquivos (re-add retoma via
  smart resume); `Delete` apaga os dados.
- TUI renderiza a partir do `DaemonState` (nunca acessa `ManagedTorrent` diretamente).

## Testabilidade

- Round-trip de `DaemonRequest`/`DaemonResponse` (serialize → deserialize) sem socket.
- `DaemonState` → linhas da tabela: função pura de render legada (unit test já existente).
- Dedup: teste com sessão real/librqbit + magnet duplicado (ver `quickstart`/tasks) ou integração manual
  documentada no quickstart.