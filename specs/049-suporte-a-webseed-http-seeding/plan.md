# Implementation Plan: US-049 — Suporte a webseed (HTTP seeding) para torrents com url-list

**Branch**: `feat/us-049-suporte-a-webseed-http-seeding-para-torr` | **Date**: 2026-08-12 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/049-suporte-a-webseed-http-seeding/spec.md`

## Summary

Torrents que declaram **`url-list`** (webseed GetRight, BEP 19 — ex.: coleções do archive.org) ficam em
**0.00%** porque o motor `librqbit` não suporta webseed e os trackers reportam 0 seeds. **Webseed como
fonte primária** (escolha do usuário): ao resolver um `.torrent` com `url-list`, o OpenTorrent **baixa os
arquivos via HTTP com Range** no layout final de saída (com progresso) e, ao concluir, **entrega o torrent
ao motor** — o `initial_check` do `librqbit` valida as peças baixadas por SHA1 e os peers completam o que
faltar. Sem patch no motor; sem dependência nova (`librqbit-bencode` já transitiva; `reqwest`+`url` já
existem).

**Modelo de execução** (escolha do usuário): **task em background no daemon** — `dispatch(Add)` responde
"adicionando via webseed" e spawna uma task que baixa via HTTP e, ao concluir, chama `session.add_torrent`;
o progresso é exposto no snapshot IPC (`DaemonState.webseed`) para a TUI. No CLI `add`, o webseed roda no
fluxo `run_add` entre o probe `list_only` e o re-add, com barra `MultiProgress`.

## Technical Context

**Language/Version**: Rust 2021, toolchain stable

**Primary Dependencies**: `librqbit` 8.1.1 (`torrent_from_bytes_ext`, `TorrentMetaV1Info::iter_file_details`,
`attrs().padding`), `librqbit-bencode` 3.1.0 (deserializer do bencode — já transitiva, adicionar direto),
`reqwest` 0.12 (Range header, follow redirects), `url` 2.5.8 (construção/join de URLs), `bytes` 1 (payloads),
`tokio` (task background, semáforo), `indicatif` (barra de progresso CLI), `anyhow`.

**Storage**: nenhum estado novo persistido — os dados webseed são o próprio conteúdo do torrent no layout
final; task background efêmera (in-memory) no daemon. Sessão persistida existente (US-009) inalterada.

**Testing**: `cargo test` (unit + integração). Testes: extração de `url-list` (lista, string única, ausente,
não-GetRight), mapeamento de URL (multi-file/single-file, encoding de path), servidor HTTP local
(fixtures) cobrindo `206`+`content-range`, `200` (Range ignorado), 404 → próxima base (FR-003/FR-004),
redirect (302→206), padding/size-0 criados sem download, arquivo já completo pulado, e integração no
daemon (dispatch Add com webseed spawna; sem url-list → add direto). Gate: `cargo fmt`, `cargo clippy -- -D
warnings`, `cargo test`, `cargo machete`.

**Target Platform**: Linux (produção, daemon systemd + socket Unix IPC); builds cross-platform preservados
(nada novo Unix-only no CLI).

**Project Type**: CLI (binário único, múltiplos modos)

**Performance Goals**: download HTTP tão rápido quanto o servidor permitir; concorrência limitada (semáforo,
~4 default) para não saturar a stack; progresso a baixo custo (atualizações throttled, sem rebuild da TUI
por byte); sem degradação do fluxo peers (url-list ausente não paga custo).

**Constraints**: zero dependência nova (usar `librqbit-bencode` transitivo; confirmar no lock); dados
webseed devem bater o SHA1 (`initial_check`) — validar `content-length`; nunca apagar arquivos existentes;
sem locks held across `.await` (async-02); erro parcial não bloqueia adição (FR-009); mensagens de domínio
pt-BR minúsculas sem pontuação final (err-10).

**Scale/Scope**: decenas de torrents; arquivos de até dezenas de GiB (coleção mint ≈ 32 GiB, 123 arquivos).

## Constitution Check

*GATE: Passa.* VI (Rust-Skills) — atendido via registro abaixo + `rust-router`/`cargo skill` antes de
codificar. VII (Cargo-Skill) — `.skill/context.md` carregado; `cargo skill think`/`write` para design
Rust. VIII (Build Velocity) — sem dep nova; loop `cargo check`/`cargo test`; enum é barato no runtime.
IX (Build & Release Pipeline) — `build.sh` no ciclo release; nada altera CI.

## Rust Skills Check

*GATE: Passa.* Skills consultadas na especificação e aplicadas ao plano:

| Skill consultada | Camada | Decisão/Impacto no design |
|------------------|--------|---------------------------|
| m01-ownership | 1 | `bytes::Bytes`/`Vec<u8>` movido entre resolve→webseed→`add_torrent` sem clone; `Arc<Session>` e registro de tasks compartilhados por `Arc<Mutex<Vec<T>>>` (dedup no daemon). |
| m02-resource | 1 | Sem RAII extra: arquivos abertos com `File::create`/`OpenOptions` e fechados por escopo; buffer de escrita reusado (não alocado por chunk). |
| m07-concurrency | 1 | Task background `tokio::spawn`; semáforo `tokio::sync::Semaphore` p/ concorrência limitada; registro compartilhado `Arc<Mutex<Vec<WebseedProgress>>>` lido pelo snapshot (lock curto, sem `.await` dentro). |
| m06-error-handling | 1 | `anyhow` para o binário (err-02); `.context(...)`/`.with_context(...)` em cada borda de rede (err-04); erro parcial vira `WebseedState::Error(String)` sem abortar a task (err-09 preserve source); sem `.unwrap()` em paths de produção (err-05). |
| m10-performance | 2 | Write em buffer reusado + `tokio::fs::write`/`AsyncWriteExt` em chunks; sem `format!` em loops; `with_capacity` no planejamento; padding espanso via `set_len` (O(1), não grava zeros) onde o FS suporta. |
| m11-ecosystem | 2 | Sem crate novo: `librqbit-bencode` (deserializer `url-list`) é transitiva do `librqbit` — adicionar no Cargo.toml com versão do lock; `reqwest` cobre Range/redirects. |
| m05-type-driven | 2 | Tipos imutáveis `WebseedConfig`/`WebseedFile` (parse-dont-validate, api-07); `WebseedState` enum (type-03) eliminando stringly-typed do estado. |
| m13-domain-error | 2 | Erros de domínio categorizados: "base webseed falhou (404)", "tamanho divergente ({got} != {want})" — exibíveis na TUI (vermelho só em falha definitiva, não no progresso). |
| m12-lifecycle | 2 | Task background com ciclo definido: register → download → add_torrent → unregister; idempotência por piece no re-add (cancelamento deixa dados parciais válidos). |
| m09-domain | 2 | Módulo `src/webseed.rs` expressa o domínio (config/files/progresso) separado do transporte (reqwest), reutilizável por CLI e daemon. |
| domain-cli | 3 | Barra `MultiProgress` no `run_add` seguindo o formato das barras existentes; scanlines de webseed na TUI reusando `PendingStatus`/linhas de progresso. |
| m04-zero-cost | 1 | Sem `Box<dyn>` de progresso onde genérico/callback impl Trait serve; sem monomorfização excessiva (uma função genérica de download). |

## Project Structure

### Documentation (this feature)

```text
specs/049-suporte-a-webseed-http-seeding/
├── plan.md              # This file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/           # Phase 1
└── tasks.md             # Phase 2 (/speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── webseed.rs           # NOVO — extract_url_list, WebseedConfig/WebseedFile/WebseedProgress,
│                        #        download_webseed (Range HTTP, bases em ordem, padding)
├── ipc.rs               # DaemonState + WebseedSnapshot (novo campo webseed)
├── daemon.rs            # dispatch(Add): url-list → spawn task background; registro + build_state
├── session_ui.rs        # TUI: renderiza linhas webseed do snapshot daemon; sem mudança no fluxo local
├── main.rs              # run_add: webseed entre probe list_only e re-add (barra MultiProgress)
└── resolve.rs           # inalterado (bytes .torrent já resolvidos); webseed consome os bytes
```

**Structure Decision**: módulo novo pequeno `src/webseed.rs` (domínio webseed, proj-02), consumido por
daemon (task background), CLI (`run_add`) e, indiretamente, TUI (via snapshot). `ipc.rs` ganha o snapshot.
A extração de `url-list` usa bencode via `librqbit-bencode` (aderente à decisão de zero dep nova).

## Complexity Tracking

> Sem violações de constituição a justificar.