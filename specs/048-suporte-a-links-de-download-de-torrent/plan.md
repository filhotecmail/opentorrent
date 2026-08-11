# Implementation Plan: US-048 — Suporte a links de download de .torrent e drag & drop no input

**Branch**: `feat/us-048-suporte-a-links-de-download-de-torrent-h` | **Date**: 2026-08-11 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/048-suporte-a-links-de-download-de-torrent/spec.md`

## Summary

O `opentorrent` passa a aceitar **URLs de download de `.torrent`** que não terminam em `.torrent` e que
respondem **HTML com link de download** (ex.: fosstorrents `thankyou` → `window.location =
/files/download.php?file=....torrent`). Hoje o `librqbit` baixa a URL e falha ao decodificar HTML
("error decoding torrent"), deixando o item **vermelho em 0%** na lista. A resolução da URL acontece
**antes** de entregar a origem ao `librqbit`: baixa o corpo, detecta o tipo (torrent direto vs HTML), e
se HTML extrai o link `.torrent` (`href`/`location`/`window.location`), baixa-o, **valida por
bencode+infohash** (`librqbit::torrent_from_bytes_ext`) e entrega `AddTorrent::TorrentFileBytes`.
Complemento: **drag & drop** no campo de input do footer — o terminal cola o caminho absoluto do arquivo
arrastado (`Event::Paste`, bracketed paste); o fluxo reconhece um caminho de arquivo `.torrent` existente
e o resolve normalmente.

Ponto de entrada único e compartilhado: `resolve_source(source) -> ResolvedSource` usado por
`add_torrent_background` (TUI), `daemon::dispatch` (daemon) e `main.rs` (modo add), de modo que os três
fluxos ganhem o comportamento sem duplicação. Sem dependências novas (`reqwest` e `url` já existem).

## Technical Context

**Language/Version**: Rust 2021, toolchain stable

**Primary Dependencies**: `librqbit` 8.1.1 (`AddTorrent::TorrentFileBytes`, `torrent_from_bytes_ext` reexportado), `reqwest` 0.12 (já dependência, `rustls-tls`), `url` 2.5.8 (já dependência, para resolver link relativo), `crossterm` (`Event::Paste`), `anyhow` (erros do binário), `tokio` (`fs::write`, `spawn_blocking` para parse).

**Storage**: nenhum estado novo persistido — resolução é efêmera (bytes em memória). Sessão persistida existente (`~/.config/opentorrent`, US-009) inalterada.

**Testing**: `cargo test` (unit + integração). Testes unitários da extração de link `.torrent` de HTML (padrões `href`, `window.location`, `<a>`), testes de validação bencode (bytes válidos/inválidos), teste de resolução relativa de URL, teste de que caminho de arquivo `.torrent` local é resolvido como arquivo. Gate: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.

**Target Platform**: Linux (produção, daemon systemd); builds cross-platform preservados (nada novo Unix-only).

**Project Type**: CLI (binário único, múltiplos modos)

**Performance Goals**: sem degradação perceptível — resolução é uma requisição HTTP extra apenas quando a resposta é HTML; parse bencode rápido (<ms) com `spawn_blocking` fora do runtime async quando chamado em contexto da TUI.

**Constraints**: não adicionar crates novos (convenção / Build Velocity); falha assíncrona vira `PendingStatus::Error` (vermelho) sem travar navegação (AC-5 da US-006); sem locks held across `.await` (async-02); `only_files_regex`/`sub_folder`/list continuam funcionando (a resolução ocorre antes do `add_torrent`).

**Scale/Scope**: 1 URL por item pendente; dezenas de torrents.

## Constitution Check

*GATE: Passa.* VI (Rust-Skills) — atendido via registro abaixo + consulta ao `rust-router`/`cargo skill` antes de codificar. VII (Cargo-Skill) — `.skill/context.md` carregado; `cargo skill think` para design. VIII (Build Velocity) — sem dep nova; loop `cargo check`/`cargo test`. IX (Build & Release Pipeline) — `build.sh` no ciclo release; nada altera CI.

## Rust Skills Check

*GATE: Passa.* Skills consultadas na especificação e aplicadas ao plano:

| Skill consultada | Camada | Decisão/Impacto no design |
|------------------|--------|---------------------------|
| m01-ownership | 1 | `Vec<u8>`/`Bytes` movido em vez de clonado entre baixar→validar→entregar; `Arc<Session>` compartilhado (padrão já existente). |
| m06-error-handling | 1 | `anyhow` para o binário (err-02/04: `.context(...)` em cada borda); mensagens de erro minúsculas, sem pontuação final (err-10). |
| m07-concurrency | 1 | Requisição HTTP `await` fora de qualquer lock guard; parse bencode em `spawn_blocking` para não bloquear o runtime da TUI (async-03). |
| m10-performance | 2 | Sem `.clone()` de bytes grandes; buffer único; sem `format!` em loops; `with_capacity` ao montar strings pequenas. |
| m11-ecosystem | 2 | Sem crate novo: `reqwest` (download) + `url` (join relativo) + `librqbit::torrent_from_bytes_ext` (validação) cobrem o requisito; `librqbit-bencode`/`sha1` seriam redundantes. |
| m12-lifecycle | 2 | Download em memória (`Bytes`), sem arquivo temporário; nenhum recurso a limpar em erro. |
| m13-domain-error | 2 | Erros de domínio amigáveis: "nenhum link .torrent encontrado na página", "resposta não é um torrent" — categorizados para a UI (vermelho). |
| m09-domain | 2 | `ResolvedSource` enum expressa o estado pós-resolução (LocalPath vs TorrentBytes), evitando stringly-typed (type-09). |
| m05-type-driven | 2 | `ResolvedSource` (enum) + `torrent_from_bytes_ext` no pipeline tipam o fluxo; inválidos não representáveis após validação. |
| domain-cli | 3 | Entrada via `/add` (paste), modo add e daemon compartilham `resolve_source`; UX de erro na lista pendente preservada. |

## Project Structure

### Documentation (this feature)

```text
specs/048-suporte-a-links-de-download-de-torrent/
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
├── resolve.rs           # NOVO — resolve_source() + extração de link .torrent + validação bencode
├── main.rs              # usa resolve_source no fluxo add (modo non-interativo)
├── daemon.rs            # usa resolve_source no dispatch Add (daemon)
├── session_ui.rs        # usa resolve_source em add_torrent_background; paste de caminho .torrent
└── update.rs            # inalterado
```

**Structure Decision**: módulo novo pequeno `src/resolve.rs` (pipeline de resolução), por feature (proj-02), chamado nos 3 pontos de adição. `main.rs` mantém-se thin (proj-01).

## Complexity Tracking

> Sem violações de constituição a justificar.
