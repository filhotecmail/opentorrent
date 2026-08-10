# Tasks: US-040 — Daemon em background e deduplicação de torrents

**Input**: Design documents from `/specs/025-daemon-background-deduplica-torrents/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/README.md

**Tests**: Testes são parte da US (round-trip JSON IPC, snapshot→render, dedup). Eles são OBRIGATÓRIOS conforme a spec.

**Organization**: Tasks grouped by user story.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: dependências e tipos base do IPC

- [ ] T001 Adicionar `net` e `io-util` ao `tokio` (Cargo.toml:30) para socket Unix; adicionar `serde` direta com `derive`. Conferir `serde_json` já presente (linha 28).
- [ ] T002 Criar `src/ipc.rs` com os tipos serde: `DaemonRequest`, `DaemonResponse`, `DaemonState`, `TorrentSnapshot` (data-model.md) + módulo `pub mod ipc;` em main.rs.
- [ ] T003 [P] Funções de enquadramento `encode_frame`/`decode_frame` (u32 LE + payload JSON) com testes unitários (round-trip).

**Checkpoint**: infraestrutura de IPC serializável pronta.

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: ciclo de vida do daemon — necessário ANTES de qualquer user story

- [ ] T004 Criar `src/daemon.rs`: `DaemonConfig` (socket_path/pid_path a partir de `config_folder()`), `is_running()`, `ensure_started()` (spawn desanexado `opentorrent --daemon-headless`), `stop()`, `status()`. `pub mod daemon;` em main.rs.
- [ ] T005 Criar `src/ipc.rs::serve()`: `UnixListener` no socket (remover stale), aceitar conexões, despachar `DaemonRequest` para a `Session` (add/pause/resume/stop/delete/get_state/shutdown), responder `DaemonResponse`. Ações sem manter guard cruzando `.await` (async-02); snapshot clonado sob lock curto.
- [ ] T006 Adicionar flag interna `--daemon-headless` em `Opts` (`#[arg(long, hide = true)]`) que executa `serve()` sem TUI, com logs em arquivo/stderr e sem morrer ao fechar TTY; `DaemonGuard`/RAII na limpeza do socket/pidfile.
- [ ] T007 Numeric subcommand `opentorrent daemon [start|stop|status]` em `SubCommand` (main.rs:196) com tratamento no `run()`.

**Checkpoint**: daemon sobe, serve IPC e encerra por `Shutdown`; `cargo check` limpo.

## Phase 3: User Story 1 - Downloads continuam com a TUI fechada (P1) 🎯 MVP

**Goal**: daemon mantém o download sem TUI; reconectar mostra progresso atualizado

- [ ] T008 `run_interactive()` (main.rs:377): substituir criação direta de `Session` por `daemon::ensure_started()` + conexão ao socket; TUI desconectada não derruba o daemon.
- [ ] T009 `run_interactive` (session_ui.rs:2218): receber cliente e usar `GetState` a cada frame/período do orçamento; remover/abstrair acesso direto a `session.with_torrents` no render (a TUI consome snapshot).
- [ ] T010 Verificação manual do US-1 (quickstart.md) — fechar TUI, progresso continua, reabrir mostra maior.

## Phase 4: User Story 2 - Deduplicação por infohash (P1)

**Goal**: adicionar torrent já na lista (processando ou processado) → nada, notice; excluído (arquivos mantidos) → readiciona

- [ ] T011 `ipc.rs`: `Add` delega a `session.add_torrent`; mapear `AlreadyManaged` → `DaemonResponse::AlreadyManaged`; `Added` → `Ok`. Este é o único ponto de decisão de dedup (fonte da verdade = sessão).
- [ ] T012 `add_torrent_background` (session_ui.rs:2101) → enviar `DaemonRequest::Add` via cliente; `AddOutcome::AlreadyManaged` exibe notice "já está na lista".
- [ ] T013 Teste de dedup: usar `session.add_torrent(magnet)` 2x com o mesmo infohash → 2ª retorna `AlreadyManaged` (teste de integração opcional `RUN_SLOW_TESTS`); validar 3 origens (magnet/URL/arquivo → mesmo infohash).

## Phase 5: User Story 3 - Daemon inicia automaticamente (P1)

**Goal**: abrir sem subcomando inicia o daemon se não rodando; segunda TUI conecta no existente

- [ ] T014 `daemon::ensure_started()`: lock exclusivo do pidfile + verificação do socket antes do spawn (evita duplo daemon); socket stale removido e recriado. `run_interactive` chama `ensure_started` na entrada (T008 dependência).
- [ ] T015 Modo `add` (run_add, main.rs:404): se daemon ativo → instruir/encaminhar `Add` via IPC e reportar `AlreadyManaged`; senão manter sessão efêmera atual. Teste manual de não-conflito (2ª TUI / CLI add).

## Phase 6: User Story 4 - Excluir da lista mantém arquivos (P2)

**Goal**: X/Delete remove da lista mantendo arquivos; excluído deixa de bloquear re-adição

- [ ] T016 `ipc.rs`: `Stop { id }` → `session.delete(id, false)` (mantém arquivos, libera infohash); `Delete { id }` → `session.delete(id, true)`. Responses `Ok`.
- [ ] T017 TUI: `remove_row()`/`confirm_delete()` (X/Delete) enviam `Stop`/`Delete` via IPC em vez de chamar a sessão diretamente; notice "removido da fila"/confirmação Y/N intactos.
- [ ] T018 Teste de integração do smart resume: excluir (Stop), readicionar mesma origem → retoma arquivos parciais (manual conforme quickstart; `existing_state` cobre parcial).

## Phase 7: User Story 5 - Daemon como systemd user service (P1)

**Goal**: binário cria o unit systemd na instalação; start/stop/status delegam ao systemctl (fallback spawn)

- [x] T019 Criar unit template (constante) em `daemon.rs`: `ExecStart=<bin> --daemon-headless`,
      `Restart=on-failure`, `WantedBy=default.target`; função `service_unit_path()` →
      `~/.config/systemd/user/opentorrent-daemon.service`; `install_service()` (escrever se ausente/diff,
      `systemctl --user daemon-reload` + `enable`).
- [x] T020 `is_systemd_user_available()`: detecta `systemctl` no PATH e tenta `systemctl --user is-system-running`.
- [x] T021 `start`/`stop`/`status` delegam a `systemctl --user` quando disponível; fallback para o spawn
      desanexado requisição `Shutdown` quando não.
- [x] T022 Auto-install do service na primeira execução: `run_interactive()`/`run_add()`/`daemon start`
      chamam `install_service()` idempotente (não reescreve se binário não mudou).
- [x] T023 Teste unitário do unit template (conteúdo com caminho do binário atual) e do caminho do service;
      teste manual `systemctl --user is-active opentorrent-daemon`.

## Phase 8: User Story 6 - Atualização automática (P1)

**Goal**: `opentorrent update` para o service, substitui o binário (assinatura opcional), religa

- [x] T024 `src/update.rs`: comando `Update` no `SubCommand`; reuso de `fetch_latest_release`/`has_update`/`update_command` (mover para `update.rs`).
- [x] T025 Fluxo: se atualizado → mensagem; senão → para o service (`daemon stop`), baixa asset da release,
      valida `.sig` se CA existir (`scripts/sign-release.sh verify`), substitui via temp+rename, religa.
- [x] T026 `update_command()` passa a sugerir `opentorrent update` no `--version` (US-029) e no modo offline.
- [x] T027 Testes: comparar versão (reuso), construção do nome do asset por tag, template do unit com binário
      atualizado; falhas de rede não corrompem o binário instalado.

## Phase 9: Qualidade (Gates obrigatórios)

- [x] T028 `cargo fmt --check` + `cargo clippy -- -D warnings` sem novos avisos.
- [x] T029 `cargo test` verde; atualizar `.specify`/docs do SDD se o design mudou durante a implementação.
- [x] T030 Revisão cruzada: constituição (IV/VIII), Rust Skills Check do plan.md concretizado no código
      (sem guard cruzando `.await`, sem `.unwrap()` em caminhos produtivos, Arc compartilhado).