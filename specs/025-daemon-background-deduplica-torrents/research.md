# Research: US-040 — Daemon em background e deduplicação de torrents

**Branch**: `feat/us-040-suporte-a-daemon-em-background-e-dedupli` | **Date**: 2026-08-10

## Objetivo da pesquisa

Confirmar a viabilidade técnica de desacoplar download (daemon) da interface (TUI) usando o motor
`librqbit`, definir o mecanismo de IPC (socket Unix + JSON), e validar que a deduplicação por infohash é
automática pelo motor.

## Investigações

### 1. Ciclo de vida atual da sessão (fonte: src/main.rs)

- `run_interactive()` (main.rs:377) constrói a `Session` com `SessionPersistenceConfig::Json { folder:
  Some(config_folder()) }` e a passa como `Arc<Session>` a `session_ui::run_interactive`.
- A TUI mantém a `Session` viva apenas enquanto o processo está aberto. Fechar a TUI derruba o processo →
  downloads param (a persistência salva o estado, mas não há processo baixando).
- `run_add()` (main.rs:404) cria uma sessão efêmera por execução (sem persistência configurada) para os
  modos `add`/`list`/`exit_on_finish`. Ele já trata `AddTorrentResponse::AlreadyManaged` (linhas 479-486).

**Conclusão**: o "gap" do daemon é apenas manter um processo vivo sem TUI. A persistência que permite
retomar a sessão já existe (US-009). O ponto de injeção é `run_interactive()`: hoje ele monta a sessão
dentro do processo da TUI; no modelo daemon, essa montagem vive no processo do daemon.

### 2. Deduplicação por infohash (fonte: librqbit + uso atual)

- `session.add_torrent(AddTorrent::from_cli_argument(...), opts)` retorna
  `AddTorrentResponse::AlreadyManaged(id, handle)` quando o infohash já está gerenciado pela sessão —
  independente da origem (magnet, URL `.torrent`, arquivo local). Isso cobre o requisito de dedup com
  custo zero extra.
- No modo interativo, `add_torrent_background` (session_ui.rs:2101) já mapeia esse retorno para
  `AddOutcome::AlreadyManaged`, que a TUI converte em notice (US-014).
- Excluir: `session.delete(TorrentIdOrHash::Id(id), false)` remove o torrent da sessão **sem apagar
  arquivos** (segundo argumento = apagar dados). Após a exclusão, o infohash fica livre e uma nova
  `add_torrent` re-adiciona — e o smart resume (`existing_state`, main.rs:147) retoma arquivos parciais.

**Conclusão**: dedup e liberação-por-exclusão são nativos do librqbit. Centralizar o `add` no processo do
daemon torna o dedup automático, consistente e idempotente entre TUI e CLI `add`.

### 3. IPC: socket Unix + JSON em Rust

- `tokio::net::UnixListener`/`UnixStream` fornecem server/client de domain sockets com suporte async e
  `accept()` em loop — sem deps extra (só `tokio`, já presente).
- Diretório de socket: reutilizar `config_folder()` (`~/.config/opentorrent`, US-038). Criar diretório com
  permissão `0o700` e o socket no formato `daemon.sock`. Pidfile `daemon.pid` para `status`.
- Enquadramento de mensagens: para mensagens pequenas (lista de torrents), **tamanho-prefixado** (u32 LE +
  payload) ou **newline-delimited JSON** são simples e suficientes. Recomenda-se enquadramento de
  comprimento para evitar ambiguidades com `\n` dentro de nomes.
- Serialização: `serde_json`. Verificar em `Cargo.toml` — `serde_json` já é usado em main.rs:332
  (`resp.json::<serde_json::Value>()`), portanto a dep já existe (transitiva de `reqwest`?) — confirmar e,
  se necessário, adicionar explícita.
- Processo desanexado: `std::process::Command` com `Stdio::null()` e `std::process::exit` no pai; lock do
  daemon via criação exclusiva de um arquivo de lock (flock ou O_CREAT|O_EXCL) para evitar duplo daemon.
  Detecção de "daemon já rodando": tentar conectar ao socket (recusa = não rodando); remover socket stale.

**Conclusão**: IPC viável com deps já existentes ou quase-zero custo.

### 4. Riscos

| Risco | Mitigação |
|-------|-----------|
| Dois processos disputando a pasta de persistência da sessão | O daemon é o único dono quando ativo; a TUI nunca cria sessão própria — conecta no daemon. `add` sem daemon usa sessão efêmera (sem `SessionPersistenceConfig`), como hoje. |
| Socket stale (daemon morto sem limpeza) | Antes de iniciar, tentar `connect()`: se recusar, remover o socket; fechar o socket na borda do `Drop`. |
| Duplo daemon | Lock exclusivo via `O_CREAT\|O_EXCL` no pidfile + verificação de socket ativo antes do spawn. |
| Windows sem socket Unix | `cfg(target_os = "windows")`: daemon desabilitado (fallback = comportamento atual); build cross tapando a porta do follow-up. |
| Locks across `.await` | Regex de código: guard de `Mutex` nunca atravessa `.await` no daemon (separar leitura do estado em snapshot clonado sob lock curto; drops antes de `.await`). |
| TUI lenta (rendering via IPC vs direto) | Snapshot pequeno (dezenas de torrents); taxa de poll no orçamento de frame (33ms) — custo de cópia serializada é desprezível vs I/O de terminal. |

## Referências

- Código: `src/main.rs` (run/run_interactive/run_add, existing_state), `src/session_ui.rs`
  (run_interactive:2218, add_torrent_background:2101, View/HomeAction/ContextAction, Pending*),
  `src/downloads.rs`, `src/metadata.rs`.
- Spec: `specs/025-daemon-background-deduplica-torrents/spec.md` (inclui 5 clarificações).