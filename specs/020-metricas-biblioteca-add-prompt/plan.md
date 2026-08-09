# Implementation Plan: US-033 — Métricas na Biblioteca, histórico tabular e /add via prompt

**Branch**: `feat/us-033-metricas-biblioteca-add-prompt` | **Date**: 2026-08-09

**Input**: Feature specification from `specs/020-metricas-biblioteca-add-prompt/spec.md`

## Summary

Evoluir a Home da TUI em três frentes em `src/session_ui.rs`:

1. **Métricas em tempo real na Biblioteca**: estender `SessionRow` com velocidades, ETA e upload; formatar DOWN/UP SPEED, ETA (`HH:MM:SS`) e RATIO com guarda contra divisão por zero; placeholders `—`/`Done`.
2. **Histórico tabular**: substituir a lista crua `format_completed` por tabela com ID/NOME/TAMANHO/RATIO/DATA CONCLUSÃO/AÇÕES, com exclusão de arquivo via fluxo US-027 (`DeleteTarget::File`).
3. **/add 100% via prompt**: remover `View::Include` por completo; `/add` preenche o prompt (`/add ` + cursor ativo); Enter com origem executa adição assíncrona sem sair da Home.

## Technical Context

**Language/Version**: Rust 2021

**Primary Dependencies**: `crossterm`, `librqbit` 8.1.1 (`TorrentStats.live: Option<LiveStats>`, `LiveStats.download_speed.mbps: f64`, `upload_speed.mbps`, `TorrentStats.uploaded_bytes`), `tokio`, `size_format::SizeFormatterBinary`, `chrono`

**Testing**: `cargo test` (testes unitários no módulo `tests` de `session_ui.rs`)

**Target Platform**: Linux

**Existing Code**: `SessionRow`, `session_table_header`/`session_table_line` (US-019/020), `render_history_panel`/`format_completed` (US-010), `render_home_prompt` (US-031), `submit_include`/`View::Include` (US-009/014, a remover), `request_delete`/`confirm_delete` (US-027), `DeleteTarget`.

## Constitution Check

- **VI. Rust-Skills**: skills consultadas (m01-ownership, m07-concurrency, m10-performance)
- **VIII. Build Velocity**: `cargo check` no loop; perfis otimizados; remoção de código morto (View::Include) reduz superfície de build

## Rust Skills Check

*GATE: aplicável por envolver design de tipos, ownership e async.*

| Skill consultada | Camada | Decisão/Impacto no design |
|------------------|--------|---------------------------|
| m01-ownership | 1 | `SessionRow` derivado de `handle.stats()` é owned local por frame; sem `clone()` de dados de rede (valores `Copy`) |
| m07-concurrency | 1 | Adição assíncrona mantém o padrão US-014 (`Arc<Mutex<Vec<PendingTorrent>>>` + `tokio::spawn`), sem trocar de view |
| m10-performance | 2 | Formatação de métricas apenas nas linhas renderizadas; sem alocação por item além do texto de linha |

## Implementation Steps

### Phase 1: Métricas na Biblioteca

- [ ] T001 Estender `SessionRow` com `down_speed_mbps`, `up_speed_mbps` (`Option<f64>`), `eta_seconds` (`Option<u64>`) e `uploaded_bytes` (`u64`), preenchidos em `session_rows()` a partir de `stats.live`.
- [ ] T002 Funções puras `format_speed(mbps)`, `format_eta(seconds)` e `format_ratio(uploaded, downloaded)` (guarda divisão por zero), com testes.
- [ ] T003 Adicionar colunas de métricas a `session_table_header`/`session_table_line` (parâmetros opcionais; alinhamento preservado).

### Phase 2: Histórico tabular

- [ ] T004 `history_table_header(name_width)` e `history_table_line(...)`: ID/NOME/TAMANHO/RATIO/DATA CONCLUSÃO/AÇÕES.
- [ ] T005 `render_history_panel` montando tabela (RATIO cruzado com a sessão quando finished, senão `—`).
- [ ] T006 `DeleteTarget::File { path, name }` + branch em `confirm_delete` (`std::fs::remove_file`) e suporte em `name()`.

### Phase 3: /add via prompt

- [ ] T007 `execute_command("/add")` → preenche `input = "/add "`, `prompt_cursor = 5`, `view = View::Home`.
- [ ] T008 `handle_home_key` Enter: rota `/add ...` para `submit_add_from_prompt()`; `/add` vazio → notice pedindo origem.
- [ ] T009 `submit_add_from_prompt`: valida sintaxe preservando o prompt em erro; registra `PendingTorrent`; spawn de `add_torrent_background`; permanece na Home; `row_index` aponta para a nova origem.

### Phase 4: Remoção de View::Include

- [ ] T010 Remover variante `View::Include` e membros `include_index`, `include_input`, `typing`.
- [ ] T011 Remover `handle_include_key`, `handle_include_typing_key`, `render_include`, `render_input_cursor`, `mouse_include`, `submit_include` e branches órfãos (`handle_key`, `handle_event` Paste, `handle_mouse`, `view_label`, `footer_hints`, `render`).
- [ ] T012 Redirecionar atalhos legados (`a`/`+` em Session) para o fluxo do prompt.

### Phase 5: Integração e validação

- [ ] T013 Testes unitários: formatação de métricas, linhas do histórico, parse do `/add` via prompt, placeholders.
- [ ] T014 Atualizar testes existentes afetados (assinatura de `session_table_line`, remoção de `View::Include`).
- [ ] T015 `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.

## Validation Plan

- `cargo test --locked --all-targets` (formatação de métricas, linhas de tabela, fluxo /add).
- Smoke test via tmux: Biblioteca com colunas de métricas; Histórico tabular com Excluir; `/add` preenchendo prompt e adicionando sem sair da Home.
- CI verde (fmt/clippy/test/machete) antes do merge.
