# Implementation Plan: US-040 — Daemon em background e deduplicação de torrents

**Branch**: `feat/us-040-suporte-a-daemon-em-background-e-dedupli` | **Date**: 2026-08-10 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/025-daemon-background-deduplica-torrents/spec.md`

## Summary

O binário `opentorrent` passa a suportar um **modo daemon** (processo sem TUI que mantém a `Session` do
librqbit viva), desacoplando o download da presença da interface. A **TUI conecta na sessão do daemon via
socket Unix + JSON** e renderiza a partir de um snapshot serializável; quando o daemon não está rodando, a
TUI o inicia automaticamente. O daemon é instalado como **systemd user service** pelo próprio binário na
primeira execução (`~/.config/systemd/user/opentorrent-daemon.service`), e `start`/`stop`/`status` delegam
ao `systemctl --user` (fallback para spawn desanexado quando o systemd não está disponível). Um novo comando
`opentorrent update` automatiza o ciclo **para o service → baixa/substitui o binário (validando a assinatura
US-018) → religa o service**. A **deduplicação por infohash** fica centralizada no daemon (fonte da verdade),
aproveitando o `AddTorrentResponse::AlreadyManaged` do librqbit para magnet/URL/arquivo com mesmo infohash.
Excluir (X/Delete) remove da lista mantendo os arquivos no disco, liberando o infohash para re-adição (smart
resume).

Follow-ups na mesma branch (US-040): **ordenação automática do grid** (torrents em processamento no topo;
dentro de cada grupo, do mais recente para o mais antigo — id decrescente) e **cores semânticas do gauge**
(vermelho = erro, laranja = downstream < 1 MiB/s, verde escuro = 100% concluído; verde normal = Live saudável),
aplicadas ao grid da Biblioteca (o Histórico mantém o verde atual).

Abordagem técnica: um único binário com modos (daemon/headless, TUI, add, daemon admin, update); systemd user
unit para o ciclo de vida; IPC por socket Unix com protocolo JSON (request/response + snapshot de estado);
sessão persistida em `~/.config/opentorrent` (fastresume) como fonte da verdade.

## Technical Context

**Language/Version**: Rust 2021, toolchain stable

**Primary Dependencies**: `librqbit` 8.1.1 (`Session`, `AddTorrentResponse`, `SessionPersistenceConfig`, `TorrentStatsState`), `tokio` (`net::UnixListener`), `serde`/`serde_json` (JSON IPC) e `reqwest` (já usado no `--version`) p/ o `update`, `clap`, `crossterm`, `anyhow`, `size_format`

**Storage**: sistema de arquivos — sessão persistida em `~/.config/opentorrent` (JSON, já existente, US-009); pidfile e socket no mesmo diretório (`daemon.pid`, `daemon.sock`); unit systemd em `~/.config/systemd/user/opentorrent-daemon.service`

**Testing**: `cargo test` (unit + integração). Testes de IPC serializados (round-trip de `DaemonRequest`/`DaemonResponse` JSON), testes de dedup via `session.add_torrent` com magnet repetido, testes do `update` (comparação de versão, template do unit). `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test` (gate da US).

**Target Platform**: Linux (socket Unix + systemd). Build cross-platform preservado (`cfg(target_os)`); Windows usa fallback (spawn desanexado) e a porta completa é follow-up.

**Project Type**: CLI (binário único, múltiplos modos de execução)

**Performance Goals**: TUI ~30 FPS (FRAME_BUDGET existente) renderizando snapshots; snapshot da fila pequeno (lista de torrents), síncrono via socket local.

**Constraints**: manter UX/TUI atual intacta quando o daemon está ativo; sem locks held across `.await` (async-02); socket com permissões restritas ao usuário; `--version`/`add` não quebram; substituição do binário no `update` atômica (cópia temp + rename).

**Scale/Scope**: 1 daemon por usuário; dezenas de torrents; unicidade por infohash.

## Constitution Check

*GATE: Passa.* IV (Rust-Skills) atendido via registro `Rust Skills Check` abaixo e consulta ao
`rust-router` antes de codificar. VIII (Build Velocity) — loop com `cargo check` + `cargo test`; `reqwest`
já existe (sem dep nova). IX (Build & Release Pipeline) — nada altera CI; apenas código.

## Rust Skills Check

*GATE: Passa.* Skills consultadas previamente na especificação e aplicadas ao plano:

| Skill consultada | Camada | Decisão/Impacto no design |
|------------------|--------|---------------------------|
| m01-ownership | 1 | `Session` compartilhada via `Arc<Session>` (já usado na TUI); clonar `Arc` em vez de clonar dados. |
| m07-concurrency | 1 | `tokio::join!`/`select!` para aceitar conecções + manter sessão; nenhum lock guard cruzando `.await` (async-02); `UnixListener` + `tokio::net`. |
| m06-error-handling | 1 | `anyhow` no lado binário; erros do daemon propagados como `DaemonResponse::Error(String)` (sem stack traces) — err-04 context nas bordas. |
| m12-lifecycle | 2 | RAII para socket/guards (`DaemonGuard`/`RawModeGuard`); limpeza de socket/pidfile no stop; `Drop` para remover socket stale; ciclo do unit systemd (install/reload/enable). |
| m11-ecosystem | 2 | Deps: `serde`/`serde_json` para IPC JSON; `reqwest` (já no `--version`) para o `update`; `nix`/libc não necessários (tokio `UnixListener` é std-compatível). |
| domain-cli | 3 | Subcomando `daemon` + flags para start/stop/status e `update`, consistentes com o CLI `clap` existente. |
| m02-resource | 1 | Substituição do binário no `update`: escrita em arquivo temporário + `rename` atômico (nunca truncar o binário em uso em execução). |
| m13-domain-error | 2 | `update` em falha de assinatura/rede: erro de domínio amigável, estado anterior preservado (binário e service intactos). |

## Project Structure

### Documentation (this feature)

```text
specs/025-daemon-background-deduplica-torrents/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── main.rs        # Opts/SubCommand: + Daemon, + Update; despacho daemon/add/interativo/update; auto-start
├── daemon.rs      # NOVO — ciclo de vida do daemon (install do unit systemd, start/stop/status via systemctl, fallback spawn, pidfile/socket mgmt)
├── ipc.rs         # NOVO — protocolo JSON/socket Unix: DaemonRequest/DaemonResponse, snapshot, serve
├── update.rs      # NOVO — comando `update`: verifica release, para service, baixa/valida/substitui binário, religa
├── session_ui.rs  # TUI: consome snapshot via IPC; ações viram requisições; auto-start na entrada
├── downloads.rs   # inalterado
└── metadata.rs    # inalterado

tests/
└── (unit em módulos conforme existente)
```

**Structure Decision**: projeto único (binário), padrão já usado. Novo módulo `daemon.rs` (ciclo de vida +
systemd), `ipc.rs` (protocolo/serviço) e `update.rs` (atualização); `session_ui.rs` adaptado. Não há workspace.

## Phase 0 — Research Findings (resumo)

- `run_interactive()` (main.rs:377) cria a `Session` com `SessionPersistenceConfig::Json` em
  `~/.config/opentorrent` (US-009) e passa `Arc<Session>` à TUI. No daemon, esse bloco passa a rodar no
  processo do daemon (vivo por si só).
- `add_torrent_background` (session_ui.rs:2101) já mapeia `AddTorrentResponse::AlreadyManaged` →
  `AddOutcome::AlreadyManaged`. No modelo daemon, esse `add` passa pelo daemon.
- Dedup por infohash é nativo do librqbit: `session.add_torrent` retorna `AlreadyManaged(id, handle)` para o
  mesmo infohash (independente de a origem ser magnet/URL/arquivo). Centralizar no daemon resolve o requisito.
- Excluir/dedup: `session.delete(id, false)` remove da sessão sem apagar arquivos → infohash liberado; o
  smart resume (lógica `existing_state` em main.rs:147) retoma arquivos parciais ao re-adicionar.
- Riscos: (a) dois processos sob a mesma pasta de persistência (`$config/opentorrent`) → o daemon é o único
  dono quando ativo; modo add sem daemon usa outra abordagem (sessão efêmera, como hoje); (b) socket stale
  (daemon morto sem limpar) → detectar conexão recusada e sobrescrever; (c) Windows sem socket Unix → gate
  `cfg`; (d) systemd user service depende de `systemctl --user` → detectar disponibilidade e cair para o
  spawn desanexado atual; (e) atualização com binário em uso → no Linux é seguro substituir o arquivo em
  execução (inode preservado); parar o service antes mantém a consistência; (f) assinatura (US-018) é
  opcional no `update` — validar quando a CA local existir, nunca bloquear por padrão.

## Phase 1 — Design (resumo)

Ver `data-model.md`, `contracts/`, `quickstart.md`. Decisões-chave:

- **Socket**: `$config/opentorrent/daemon.sock` (Unix), diretório com permissão `0o700`; pidfile
  `daemon.pid`. Liação de respostas imediata (request → response), snapshot via request `GetState`.
- **Service systemd**: unit em `~/.config/systemd/user/opentorrent-daemon.service` com
  `ExecStart=<bin> --daemon-headless`, `Restart=on-failure`, `WantedBy=default.target`. Install: escrever →
  `systemctl --user daemon-reload` → `systemctl --user enable`. `start`/`stop`/`status` →
  `systemctl --user [start|stop|is-active]`; fallback spawn desanexado quando `systemctl` ausente/falhar.
- **Update**: `opentorrent update` verifica release (reuso do fluxo US-029), compara SemVer, para o service,
  baixa `opentorrent-{tag}-linux-x86_64`, valida `.sig` se a CA local existir, substitui via temp+rename,
  religa o service.
- **Protocolo** (JSON, newline-delimited ou tamanho-prefixado): `DaemonRequest::{Add, Pause, Resume, Stop,
  Delete, GetState}`; `DaemonResponse::{Ok, AlreadyManaged, State(snapshot), Error(msg)}`. Snapshot contém
  os campos que a TUI já renderiza (id, nome, estado, bytes, finished, down/up speed, eta, ratio, upload).
- **Modos**:
  - `opentorrent daemon [start|stop|status]` — start: garante o service instalado e usa `systemctl --user
    start`; stop: `systemctl --user stop`; status: `systemctl --user is-active`.
  - Interativo: se socket ativo → conectar; senão → install do service (se ausente) + start e conectar.
  - `add` (CLI): se daemon ativo → delega `Add` via IPC (dedup aplicado no daemon); senão → comportamento
    atual (sessão efêmera).
  - `update`: fluxo de atualização automática do binário + service.
- **TUI como cliente do snapshot**: `render` constrói as linhas a partir do snapshot em vez de
  `session.with_torrents`. Ações são requisições; `add` do prompt vira `DaemonRequest::Add` com dedup no
  daemon. Preserva `PendingTorrent` (origem não confirmada) na TUI.

## Phase 2 — Task Breakdown

Ver `tasks.md` (gerado por `/speckit.tasks`).

Fases das follow-ups (ordenação + cores do gauge) documentadas no `tasks.md` (Phase 10 e 11): a ordenação é
pura de TUI (`sort_by` sobre `SessionRow`/snapshot com a mesma chave: processando primeiro, depois id desc), e
as cores são tokens novos da paleta (`progress_low` laranja, `progress_done` verde escuro) aplicados na
sobreposição da barra do grid.

## Complexity Tracking

*N/A* — nenhuma violação da constituição.

## Open Questions / Blockers

- **`serde`/`serde_json` (proto IPC)**: já confirmado presente (usado no IPC atual). Sem blocker.
- **Disponibilidade do `systemctl --user`**: varia por distribuição; detectar via `Command::new("systemctl").arg(...)` e
  usar fallback spawn quando indisponível (US-040 sarcate 5).
- **Dedup no modo add sem daemon**: manter o comportamento atual (2º `AlreadyManaged` já tratado) — sem
  mudança de contrato.
- **Shutdown do daemon**: `daemon stop` via `systemctl --user stop` (ou requisição `Shutdown` no fallback).
  O dedup/exclusão transitório da lista não persiste "lista excluída" (a sessão JSON guarda o torrent; a
  exclusão via `session.delete` remove da sessão). Se o requisito exigir "lista" distinta da sessão
  (excluído ≠ removido do fastresume), será follow-up.
- **Assinatura do `update`**: validar contra a CA local (`~/.local/share/opentorrent/signing`) apenas se
  existir; a ausência não bloqueia a atualização (o usuário pode instalar sem build local).
  O dedup/exclusão transitório da lista não persiste "lista excluída" (a sessão JSON guarda o torrent; a
  exclusão via `session.delete` remove da sessão). Se o requisito exigir "lista" distinta da sessão
  (excluído ≠ removido do fastresume), será follow-up.