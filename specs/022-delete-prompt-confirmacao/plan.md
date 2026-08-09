# Implementation Plan: US-035 — /delete com confirmação no prompt da Home

**Branch**: `feat/us-035-delete-prompt-confirmacao` | **Date**: 2026-08-09 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/022-delete-prompt-confirmacao/spec.md`

## Summary

Unificar a confirmação de exclusão definitiva no **prompt fixo da Home**, no mesmo padrão do `/add` (US-034): em vez do modal central Y/N (US-027), o prompt da base exibe `excluir '<nome>'? [Y] ou [N]` e o usuário responde `Y` (confirma) ou `N`/`Esc` (cancela). Todos os caminhos de exclusão — comando `/delete` (Home e popup), tecla `Delete` na Session e botão Excluir do mouse — passam a usar a mesma confirmação no prompt, sempre com `view = View::Home`.

## Technical Context

**Language/Version**: Rust 2021

**Primary Dependencies**: `crossterm` (eventos), `librqbit` 8.1.1 (`Session::delete`), `tokio`

**Storage**: N/A (estado em memória da TUI; `DeleteTarget` + `confirming_delete` — sem persistência)

**Testing**: `cargo test` (testes unitários no módulo `tests` de `session_ui.rs`)

**Target Platform**: Linux

**Project Type**: CLI/TUI (Rust)

**Performance Goals**: Sem alocação extra por frame; confirmação é estado (`Option<DeleteTarget>`) — custo zero no hot path de render

**Constraints**: Nenhuma nova dependência; `request_delete`/`confirm_delete`/`DeleteTarget` não mudam de semântica; `THEME.overlay_bg` permanece (ainda usado pelo diálogo Include)

**Scale/Scope**: Um arquivo (`src/session_ui.rs`), ~6 pontos de alteração + testes

## Constitution Check

*GATE: aplicável por envolver design de estado de UI.*

- **VI. Rust-Skills**: skills consultadas via `rust-router` (ver tabela abaixo)
- **VII. Cargo-Skill**: `.skill/context.md` carregado (Layer 1); regras aplicadas em design/implementação
- **VIII. Build Velocity**: `cargo check` no loop; sem dependências novas; testes unitários leves

## Rust Skills Check

*GATE: aplicável por envolver estado mutável de UI.*

| Skill consultada | Camada | Decisão/Impacto no design |
|------------------|--------|---------------------------|
| rust-router | 3 | Projeto CLI/TUI → roteou para `domain-cli` (L3) + `m03-mutability`/`m01-ownership` (L1) |
| domain-cli | 3 | Confirmação destrutiva no prompt: rótulo explícito com o nome do alvo, atalho Y/N explícito, Esc para cancelar (padrão CLI seguro) |
| m03-mutability | 1 | Reuso de `confirming_delete: Option<DeleteTarget>` como flag de modo (não novo campo); transição para `View::Home` feita no ponto de solicitação (`request_delete` call-sites) |
| m06-error-handling | 1 | Erros de exclusão continuam como notice (nunca `.unwrap()`); `confirm_delete` inalterado |
| m05-type-driven | 2 | Rótulo da pergunta em função pura `confirm_delete_prompt_label(name)` (testável sem session); `footer_hints` ganha flag de confirmação |
| m07-concurrency | 1 | Nenhuma mudança em async; exclusão continua `Session::delete` aguardada no handler |

## Project Structure

### Documentation (this feature)

```text
specs/022-delete-prompt-confirmacao/
├── spec.md              # Feature specification
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

**Structure Decision**: Single project; a US é contida no módulo `session_ui`, seguindo `proj-01`/`proj-03`.

## Implementation Steps

### Phase 1: Confirmação no prompt da Home (substitui o modal)

1. `render_home_prompt`: quando `confirming_delete.is_some()`, exibir o rótulo `excluir '<nome>'? [Y] ou [N]` (função pura `confirm_delete_prompt_label`), suprimir placeholder/input e não setar cursor.
2. `home_prompt_label` mantém-se (`> ` / `insira o link: `); a confirmação é tratada antes, no `render_home_prompt`.
3. `render()`: remover o bloco que desenha `render_confirm_delete` (overlay) quando `confirming_delete.is_some()`.
4. Remover o método `render_confirm_delete` e a função `confirm_delete_modal_lines` (mortos).

### Phase 2: Call-sites de exclusão apontam para a Home

1. `execute_command("/delete")`: `request_delete(self.row_index)` + `view = View::Home` (cobre também o `/delete` executado pelo popup, que antes não exibia o modal).
2. `handle_session_key` tecla `Delete`: `request_delete(self.row_index)` + `view = View::Home`; remover o bloco `if self.confirming_delete.is_some()` do Session (inacessível — a confirmação só vive na Home).
3. `mouse_session` botão Excluir (índice 2): `request_delete(idx)` + `view = View::Home`.
4. `handle_home_key` bloco `confirming_delete.is_some()` (US-027): manter Y/y/S/s → `confirm_delete()`, N/n/Esc → cancela; adicionar Ctrl+C → sai (AC US1-5).

### Phase 3: Hints do Footer

1. `footer_hints(view, prompt_add_mode, confirming_delete)`: `View::Home` com confirmação → `"Y exclui · N/Esc cancela"`.
2. Atualizar a chamada em `render_footer` e os testes existentes de `footer_hints`.

### Phase 4: Tests & validação

1. Remover testes de `confirm_delete_modal_lines` (3 testes mortos).
2. Novos testes: `confirm_delete_prompt_label` (nome + `[Y] ou [N]`), `footer_hints` no modo confirmação, `session_footer_hints_mention_delete_shortcut` com nova assinatura.
3. `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.

## Validation Plan

- `cargo test --locked --all-targets`
- Smoke test via tmux: `/delete` + Enter na Home → prompt pergunta com nome; `Y` exclui (linha some da Biblioteca); `N`/Esc cancela; `Delete` na Session → transita para a Home com a pergunta; `Y` exclui; `/delete` via popup (menu) mostra a pergunta.
- CI verde (fmt/clippy/test/machete) antes do merge.
