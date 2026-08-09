# Implementation Plan: US-034 — /add com prompt "insira o link:" na Home

**Branch**: `feat/us-034-add-prompt-home` | **Date**: 2026-08-09 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/021-add-prompt-home/spec.md`

## Summary

Evoluir o fluxo de adição de torrents pela Home em `src/session_ui.rs`: `/add` + Enter sem origem ativa um modo no próprio prompt fixo da base ("insira o link:"), onde o usuário digita/cola a origem e pressiona Enter — sem trocar de view. `/add <origem>` inline adiciona direto. A view Include é preservada para os atalhos `a`/`A`/`+` da Session.

## Technical Context

**Language/Version**: Rust 2021

**Primary Dependencies**: `crossterm`, `librqbit` 8.1.1 (`AddTorrent::from_cli_argument`), `tokio`

**Storage**: N/A (estado em memória da TUI; `PendingTorrent` compartilhado via `Arc<Mutex<Vec<...>>>` — padrão US-014)

**Testing**: `cargo test` (testes unitários no módulo `tests` de `session_ui.rs`)

**Target Platform**: Linux

**Project Type**: CLI/TUI (Rust)

**Performance Goals**: Sem alocação extra por frame; o modo add é um boolean de estado — custo zero no hot path de render

**Constraints**: Nenhuma nova dependência; não remover `View::Include`/`submit_include`

**Scale/Scope**: Um arquivo (`src/session_ui.rs`), ~6 pontos de alteração + testes

## Constitution Check

*GATE: aplicável por envolver design de estado de UI e async.*

- **VI. Rust-Skills**: skills consultadas via `rust-router` (ver tabela abaixo)
- **VII. Cargo-Skill**: `.skill/context.md` carregado (Layer 1); regras aplicadas em design/implementação
- **VIII. Build Velocity**: `cargo check` no loop; sem dependências novas; testes unitários leves

## Rust Skills Check

*GATE: aplicável por envolver estado mutável de UI e async.*

| Skill consultada | Camada | Decisão/Impacto no design |
|------------------|--------|---------------------------|
| rust-router | 3 | Projeto CLI/TUI → roteou para `domain-cli` (L3) + `m01-ownership`/`m07-concurrency` (L1) |
| domain-cli | 3 | Ergonomia de prompt: estado de digitação explícito, Esc para cancelar, feedback via notice |
| m03-mutability | 1 | Estado do prompt mutável apenas no handler de eventos; `prompt_add_mode: bool` simples (não enum/typestate — escopo pequeno) |
| m06-error-handling | 1 | Erros de origem reutilizam notice + preservação do texto digitado (padrão `submit_include`); sem `.unwrap()` novo |
| m07-concurrency | 1 | Adição assíncrona mantém padrão US-014 (`Arc<Mutex<Vec<PendingTorrent>>>` + `tokio::spawn`), sem lock atravessando `.await` |
| m05-type-driven | 2 | Extração de origem inline via função pura `parse_add_command` (testável sem session) em vez de lógica embutida no handler |

## Project Structure

### Documentation (this feature)

```text
specs/021-add-prompt-home/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (speckit.tasks)
```

### Source Code (repository root)

```text
src/
└── session_ui.rs        # Única superfície de mudança desta US
```

**Structure Decision**: Single project; a US é contida no módulo `session_ui` (`Tui` + helpers livres + testes `#[cfg(test)]`), seguindo `proj-01`/`proj-03`.

## Implementation Steps

### Phase 1: Modo "insira o link:" no prompt da Home

1. Adicionar campo `prompt_add_mode: bool` (init `false`) a `Tui`.
2. `execute_command("/add")`: ativar `prompt_add_mode`, limpar `input`/`prompt_cursor`, manter `view = View::Home`, `notice = None`. (Não abre `View::Include`.)
3. `handle_home_key`: no topo, se `prompt_add_mode`, delega para novo `handle_add_prompt_key` (Enter → submit; Esc → cancela; chars/backspace → edição do prompt; paste entra via `handle_event` na Home).
4. `handle_home_key` Enter (modo normal): extrair origem inline com `parse_add_command(&cmd)`; se `Some(origem)` → `submit_add_from_prompt(origem)`; se `None` e é `/add` → ativar modo; senão → `execute_command`.
5. `render_home_prompt`: quando `prompt_add_mode`, rótulo "insira o link:" e placeholder suprimido.
6. `footer_hints(View::Home)`: indicar "Esc cancelar" no modo add.

### Phase 2: submit_add_from_prompt (validação + background)

1. `fn submit_add_from_prompt(&mut self, source: &str)`: valida sintaxe com `AddTorrent::from_cli_argument` (erro → notice + preserva texto, mantém modo se vindo do prompt); registra `PendingTorrent`, spawn de `add_torrent_background`, `view = View::Home`, `row_index` na nova origem.

### Phase 3: Tests & validação

1. Testes unitários: `parse_add_command` (inline com/sem origem), transição de estado do `execute_command`/`handle_home_key` (se viável sem session), placeholder do prompt, notice de erro preservando texto.
2. `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.

## Validation Plan

- `cargo test --locked --all-targets`
- Smoke test via tmux: `/add` + Enter → "insira o link:" no prompt; colar magnet + Enter → linha "inicializando" na Biblioteca; Esc cancela; `/add magnet:...` adiciona direto; `a` na Session ainda abre Include.
- CI verde (fmt/clippy/test/machete) antes do merge.
