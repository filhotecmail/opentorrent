# Tasks: US-049 — Suporte a webseed (HTTP seeding) para torrents com url-list

**Input**: Design documents from `/specs/049-suporte-a-webseed-http-seeding/`

**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/README.md

**Tests**: Testes unitários/integração são OBRIGATÓRIOS (spec organization). Gate: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.

**Organization**: Tasks agrupadas: (1) módulo webseed core + testes; (2) IPC/snapshot do daemon; (3) integração daemon (task background); (4) integração CLI; (5) TUI; (6) validação final e release local (fluxo combinado: commit em stage, release, instalar).

## Phase 1: Módulo webseed (core)

**Purpose**: `src/webseed.rs` — extração de `url-list`, modelagem e download HTTP com Range

- [ ] T001 Adicionar `librqbit-bencode = "3.1.0"` a `[dependencies]` (versão já no lock — custo de build zerado). Registrar `pub mod webseed;` em main.rs.
- [ ] T002 [P] `extract_url_list(bytes: &[u8]) -> Option<Vec<String>>` — `BencodeDeserializer` + struct `#[serde(rename = "url-list")]` com `#[serde(untagged)]` (lista ou string única); filtra bases GetRight (terminam em `/`). Testes: lista 3 bases (fixture mint), string única, ausente → `None`, não-GetRight → `None`.
- [ ] T003 `WebseedConfig { bases, name }` e `WebseedFile { relative_path, length, is_padding }` (data-model.md) + `build_webseed_files(info, only_files)` a partir de `iter_file_details()`/`attrs().padding`. Testes: multi-file com padding (fixture mint), single-file, `only_files` filtrando e contando progresso sem padding.
- [ ] T004 [P] `file_url(base, config, file) -> url::Url` — multi-file `base + name + '/' + path`; single-file `base + name`; componentes encodados. Testes: path com espaços/acentos; single-file.
- [ ] T005 [P] `download_webseed(client, config, files, output_folder, concurrency, progress)` — Range por arquivo, bases em ordem, aceita `206`/`200`, valida content-length, grava no layout final, reusa arquivos completos, **padding/size-0 criados sem HTTP**, semáforo de concorrência, callback de progresso. Testes com servidor HTTP local (padrão do `resolve.rs`): 206+content-range, 200 (Range ignorado), 404→próxima base, redirect, len divergente → erro parcial, arquivo já completo pulado, padding criado sem request.

**Checkpoint**: `webseed.rs` compila, testes verdes; `cargo check` + `cargo clippy -- -D warnings` limpos.

## Phase 2: IPC/snapshot do daemon

**Purpose**: expor o progresso das tasks webseed para a TUI

- [ ] T006 `ipc.rs`: adicionar `WebseedSnapshot { id, source, name, downloaded_bytes, total_bytes, done_files, total_files, state }` e campo `webseed: Vec<WebseedSnapshot>` em `DaemonState` (com `#[serde(default)]` para compat evolutiva). Teste: roundtrip de encode/decode com campos preenchidos.

**Checkpoint**: protocolo IPC evolui sem quebrar compatibilidade (Default/deser).

## Phase 3: Integração daemon (task background)

**Purpose**: `dispatch(Add)` detecta `url-list` e spawna task webseed em background

- [ ] T007 `daemon.rs`: registro compartilhado de tasks webseed (`Arc<Mutex<Vec<WebseedProgress>>>` + `Vec<WebseedSnapshot>` derivado); no `dispatch(Add)`, após `resolve_source`, se `TorrentBytes` tem `url-list` → registra, spawna task e responde `Ok { message: "adicionando via webseed" }`.
- [ ] T008 A task background: calcula `output_folder` (`default_output_folder()` + subfolder do motor — `info.name` p/ multi-file), baixa com progresso atualizando o registro, e ao concluir chama `session.add_torrent(bytes → TorrentFileBytes, None)` e remove do registro. Em erro definitivo, seta `state=error` (permite re-add).
- [ ] T009 `build_state` inclui o registro webseed em `DaemonState.webseed`. Teste de integração (RUN_SLOW_TESTS com server HTTP local): dispatch Add de `.torrent` com url-list responde rápido e o torrent entra na sessão após o download; sem url-list → add direto.

**Checkpoint**: `cargo test` integração daemon + clippy limpos.

## Phase 4: Integração CLI (`run_add`)

**Purpose**: webseed no modo non-interativo com barra de progresso

- [ ] T010 `main.rs::run_add`: no fluxo por-torrent, após o probe `list_only` e antes do re-add, se `extract_url_list(bytes)` → roda `download_webseed` no `output_folder` do probe com `MultiProgress` bar (total + arquivo atual); depois segue `existing_state`/re-add (validado por `initial_check`). Sem `url-list` → fluxo atual.

**Checkpoint**: `cargo run -- add <url archive.org> -o /tmp/dl` mostra barra e completa sem seeds.

## Phase 5: TUI (modo daemon)

**Purpose**: linhas de progresso webseed na Biblioteca

- [ ] T011 `session_ui.rs`: renderiza `snapshot.webseed` como linhas de progresso (nome + barra/percentual) sem estado de erro; quando a task some do snapshot (torrent entrou na sessão) a linha é removida naturalmente; falha da task → linha `Error` com mensagem de domínio.

**Checkpoint**: `/add` de url-list com daemon ativo mostra progresso e, ao concluir, o torrent aparece na Biblioteca.

## Phase 6: Validação final e release local

- [ ] T012 Gate completo: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`, `cargo check --target x86_64-pc-windows-gnu`.
- [ ] T013 Regressão real (SC-001): `opentorrent add "<mint archive.torrent>" -o /tmp/dl-mint` (0 seeds) — progresso HTTP >0% e download completo em ≤2 min de teste (dados parciais já no disco ajudam); artix (sem url-list) via peers inalterado (FR-006).
- [ ] T014 Regressão SC-004: teste unitário/integrado com primeira base falhando (404) e segunda base ok.
- [ ] T015 Ciclo release local (**sem PR**): commit em stage/branch; `./build.sh release` (bump → v0.1.41 + assinatura); instalar `/usr/local/bin/opentorrent` (backup `opentorrent.old`); reiniciar daemon; avisar o usuário para testar.

## Checkpoints finais

- `cargo check`/test nos 3 pontos (daemon task, CLI, TUI);
- gate verde (fmt/clippy/test/machete + windows check);
- release local instalado e daemon reiniciado para teste do usuário.