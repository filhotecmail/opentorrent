# Implementation Plan: US-037 — Unificar a interface na Home

**Branch**: `feat/us-037-unificar-home` | **Date**: 2026-08-09 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/024-unificar-home/spec.md`

## Summary

Remover as views separadas `Session`/`Include`/`Completed` e os comandos `/list`/`/history`, deixando a Home (Biblioteca + Histórico + prompt fixo) como única tela. As ações Pausar/Retomar/Parar/Excluir saem dos botões da Sessão e passam a atalhos de tecla única na Home (prompt vazio: `p`/`P`, `r`/`R`, `x`/`X`, `Delete`) e ao menu de contexto da US-036. Limpeza completa de handlers, renders, helpers e constantes órfãos (`cargo clippy -- -D warnings` como gate).

## Technical Context

**Language/Version**: Rust 2021

**Primary Dependencies**: `crossterm`, `librqbit` 8.1.1, `tokio`

**Storage**: N/A (estado em memória da TUI)

**Testing**: `cargo test` (testes unitários no módulo `tests` de `session_ui.rs`)

**Target Platform**: Linux

**Project Type**: CLI/TUI (Rust)

**Performance Goals**: Sem overhead no hot path de render — remoção de código não adiciona alocação

**Constraints**: Nenhuma nova dependência; sem alterar `pause_row`/`resume_row`/`remove_row`/`request_delete`/`confirm_delete`; `cargo clippy -- -D warnings` obrigatório (dead code é erro)

**Scale/Scope**: Um arquivo (`src/session_ui.rs`), ~25 pontos de remoção/edição + testes

## Constitution Check

*GATE: aplicável por envolver design de estado de UI, remoção estrutural e clippy estrito.*

- **VI. Rust-Skills**: skills consultadas via `rust-router` (ver tabela abaixo)
- **VII. Cargo-Skill**: `.skill/context.md` carregado; regras `name-`/`anti-` aplicadas na limpeza
- **VIII. Build Velocity**: `cargo check` no loop; remoção de código reduz tempo de compilação; sem novas deps

## Rust Skills Check

*GATE: aplicável por envolver estado mutável de UI, remoção de código e clippy estrito.*

| Skill consultada | Camada | Decisão/Impacto no design |
|------------------|--------|---------------------------|
| rust-router | 3 | Projeto CLI/TUI → `domain-cli` (L3) + `m05-type-driven`/`m06-error-handling` (L2/L1) |
| domain-cli | 3 | Ergonomia: atalhos de tecla única só com prompt vazio (nunca disputar texto digitado); footer_hints documenta os atalhos |
| m05-type-driven | 2 | `View` reduzido a `Home`/`Menu` elimina estados inválidos (não há mais tela sem prompt); campos órfãos (`include_*`, `typing`) removidos com o tipo |
| m06-error-handling | 1 | Fluxos de erro existentes (`notice` + `clamp_row_index`) preservados; sem `.unwrap()` novo |
| m03-mutability | 1 | Estado dos atalhos é `input.is_empty()` (leitura de `String`) — nenhum campo novo; `prompt_add_mode`/`confirming_delete` continuam soberanos |
| m15-anti-pattern | 2 | Remover código morto na fonte (não `#[allow(dead_code)]`) — `anti-09`/`anti-01`; helpers órfãos eliminados junto com seus testes |

## Project Structure

### Documentation (this feature)

```text
specs/024-unificar-home/
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

### Phase 1: Remoções estruturais

1. `View`: remover `Session`, `Include`, `Completed` (ficam `Home`, `Menu`).
2. `COMMANDS`: remover `/list` e `/history` (8 → 6 entradas).
3. `execute_command`: remover os branches `/list` e `/history`.
4. Remover handlers `handle_session_key`, `handle_include_key`, `handle_include_typing_key`, `handle_completed_key`.
5. Remover `mouse_session`, `mouse_include`; `handle_mouse` passa a cobrir só Home/Menu.
6. Remover `render_session`, `render_include`, `render_completed`, `render_input_cursor`, `submit_include`.
7. Remover campos `include_index`, `include_input`, `input_cursor_pos`, `typing` de `Tui` (init e usos).
8. `view_label`/`footer_hints`: remover variantes Session/Include/Completed.

### Phase 2: Atalhos na Home

1. `handle_home_key`: antes do fluxo normal, se `!prompt_add_mode && confirming_delete.is_none() && input.is_empty()`:
   - `p`/`P` → `pause_row().await`
   - `r`/`R` → `resume_row().await`
   - `x`/`X` → `remove_row().await`
   - `Delete` → `request_delete(self.row_index)`
2. `footer_hints(View::Home)`: `"p pausar · r retomar · x remover · Del excluir · / comandos · ↑/↓ seleciona"`.

### Phase 3: Limpeza de órfãos (gate clippy)

1. Remover helpers não mais usados (verificar com `rg` antes de cada remoção): `actions_string`, `in_action_button`, `action_button_at`, `session_row_line`, `centered_write`, `wrap_text`, `body_frame`, `frame_rect`, `frame_top_bottom`, `center_content_top`.
2. Remover constantes de ações: `ACTIONS_WIDTH`, `ACTION_BTN_WIDTH`, `ACTION_GAP`, `ACTION_1_START`, `ACTION_2_START`; ajustar `Layout`/`session_table_line`/`session_table_header` se `show_actions`/ações ficarem órfãos.
3. Remover testes órfãos (ex.: `actions_string_shows_resume_when_paused`, `session_row_line_accounts_for_thin_separator`, `action_button_at` tests, `frame_rect_*`, `body_frame_*`, `center_content_top_*`, `wrap_text_*`, `session_footer_hints_mention_delete_shortcut` → substituir por teste dos atalhos da Home).
4. Atualizar testes afetados: `COMMANDS.len()` 8 → 6, `filter_commands("/li")` (sem `/list`), `command_item_line` (usar outro comando), `parse_add_command("/list")`/`"/history"` (continuam `None` — sem `/` + add), `execute_command_notice_for_unknown`.

### Phase 4: Tests & validação

1. Testes novos: helper puro `home_action_for_key(key, prompt_empty, prompt_add_mode, confirming_delete) -> Option<HomeAction>` (ou equivalente) cobrindo: p/P/r/R/x/X/Delete com prompt vazio vs texto digitado; estados `prompt_add_mode`/`confirming_delete` soberanos; footer_hints com os atalhos.
2. `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test --locked --all-targets`, `cargo machete`.

## Validation Plan

- `cargo test --locked --all-targets`
- Smoke test via tmux: `/` lista 6 comandos; `/list`/`/history` → "comando desconhecido"; `p` com prompt vazio pausa; `r` retoma; `x` remove; `Delete` abre confirmação; digitar `p` com `/add` no prompt digita o caractere (sem pausar); `a`/`A`/`+` não abrem tela; Esc na Home não faz nada.
- CI verde (fmt/clippy/test/machete) antes do merge.
