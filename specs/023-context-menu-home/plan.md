# Implementation Plan: US-036 — Menu de contexto com clique direito na Biblioteca

**Branch**: `feat/us-036-context-menu-session` | **Date**: 2026-08-09 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/023-context-menu-home/spec.md`

## Summary

Adicionar menu popup de contexto ao clicar com o **botão direito** em um item da lista de processamento (Biblioteca da Home, após a US-037). O popup é ancorado na linha clicada e oferece `Pausar`, `Retomar`, `Parar` e `Excluir` (apenas `Excluir` para pendentes sem handle). Interação por teclado (↑/↓/Enter/Esc) e mouse (clique na opção executa; clique fora fecha). Reutiliza `pause_row`/`resume_row`/`remove_row`/`request_delete` e o padrão de popup da US-031.

## Technical Context

**Language/Version**: Rust 2021

**Primary Dependencies**: `crossterm` (`MouseButton::Right`, `Event::Mouse`), `librqbit` 8.1.1, `tokio`

**Storage**: N/A (estado em memória da TUI)

**Testing**: `cargo test` (testes unitários no módulo `tests` de `session_ui.rs`)

**Target Platform**: Linux

**Project Type**: CLI/TUI (Rust)

**Performance Goals**: Popup é overlay sobre a Home — sem impacto no hot path de render da tabela

**Constraints**: Nenhuma nova dependência; mouse já habilitado (`EnableMouseCapture`); reutilizar ações existentes; a US-037 (unificação) deve estar entregue antes (o menu atua na Biblioteca da Home)

**Scale/Scope**: Um arquivo (`src/session_ui.rs`), ~5 pontos de alteração + testes

## Constitution Check

*GATE: aplicável por envolver design de estado de UI e entrada de mouse.*

- **VI. Rust-Skills**: skills consultadas via `rust-router` (ver tabela abaixo)
- **VII. Cargo-Skill**: `.skill/context.md` carregado; regras `m05`/`m07` aplicadas
- **VIII. Build Velocity**: `cargo check` no loop; sem dependências novas; `MouseEventKind::Down(MouseButton::Right)` é padrão crossterm já disponível

## Rust Skills Check

*GATE: aplicável por envolver estado mutável de UI e eventos async.*

| Skill consultada | Camada | Decisão/Impacto no design |
|------------------|--------|---------------------------|
| rust-router | 3 | Projeto CLI/TUI → `domain-cli` (L3) + `m05-type-driven`/`m07-concurrency` |
| domain-cli | 3 | Ergonomia: popup de contexto com navegação por teclado e clique (padrão do menu de comandos US-031); Esc/clique-fora cancela |
| m05-type-driven | 2 | Estado do popup modelado como `Option<ContextMenu>` (ausência = fechado); ações tipadas como enum `ContextAction` (não strings) — estados inválidos não representáveis |
| m07-concurrency | 1 | Ações de sessão são async (`session.pause/unpause/delete`): executar via `.await` nos handlers, nunca em `render`; reutilizar `pause_row`/`resume_row`/`remove_row` (sem lock através de `.await`) |
| m06-error-handling | 1 | Erros de execução da ação propagam para `handle_event` → notice (padrão existente); popup fechado antes de executar |
| m01-ownership | 1 | `ContextAction` por valor (enum pequeno, `Copy`); `context_menu` consome `self` via método que fecha o popup e despacha |

## Project Structure

### Documentation (this feature)

```text
specs/023-context-menu-home/
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

### Phase 1: Estado do menu de contexto

1. `enum ContextAction { Pause, Resume, Stop, Delete }` (`Copy`); `struct ContextMenu { entry_index, selected, anchor_row }`.
2. Campos em `Tui`: `context_menu: Option<ContextMenu>`, `context_rect: Option<(u16, u16, u16, u16)>` (init `None`).
3. Helpers puros: `context_menu_options(entry_index, session_len) -> Vec<ContextAction>` e `context_action_label(action) -> &'static str`.

### Phase 2: Abertura e interação (mouse + teclado)

1. `handle_mouse`: aceitar `MouseButton::Right` além de `Left`; na Home, resolver a linha clicada (mesmo mapeamento de `mouse_home`), setar `row_index` e abrir `context_menu` (anchor = linha clicada); com popup aberto, clique esquerdo dentro de `context_rect` executa a opção, fora fecha.
2. `handle_home_key`: com `context_menu.is_some()`, tratar ↑/↓ (mover `selected`), Enter (executar), Esc (fechar), demais teclas ignoradas — antes do fluxo normal.
3. `handle_event`: `Down(MouseButton::Right)` também marca `redraw = true`.
4. `execute_context_action(action)`: seta `row_index = context_menu.entry_index`, fecha o popup e despacha Pause→`pause_row`, Resume→`resume_row`, Stop→`remove_row`, Delete→`request_delete`.

### Phase 3: Render

1. `render_home` (ou após `render_library_panel`): se `context_menu.is_some()`, desenhar popup ancorado em `anchor_row` com `fill_rect` + `draw_box` + highlight (padrão US-031), clamp vertical dentro do Body, gravando `context_rect`.
2. `footer_hints(View::Home)` com popup aberto: `"↑/↓ navegar · Enter executa · Esc fecha"`.

### Phase 4: Tests & validação

1. Testes unitários: `context_menu_options` (torrent → 4 opções; pendente → só `Delete`), `context_action_label`, mapeamento de clique em `context_rect` (helper puro se viável).
2. `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test --locked --all-targets`, `cargo machete`.

## Validation Plan

- `cargo test --locked --all-targets`
- Smoke test via tmux: clique direito (enviar escape de mouse por tmux ou via `tmux send-keys` com mouse) em torrent → popup com 4 opções; ↑/↓ + Enter executa; Esc fecha; clique fora fecha; pendente → só `Excluir`; `Excluir` abre a confirmação Y/N no prompt.
- CI verde (fmt/clippy/test/machete) antes do merge.
