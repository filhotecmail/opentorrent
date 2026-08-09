# Implementation Plan: US-031 — TUI com painéis divididos, prompt na base e menu de comandos flutuante

**Branch**: `feat/us-031-tui-paineis-comandos` | **Date**: 2026-08-09

**Input**: Feature specification from `specs/019-tui-paineis-comandos/spec.md`

## Summary

Redesenhar a tela inicial da TUI no estilo do opencode (OpenTUI):

1. **Full-window reativo**: substituir o quadro fixo 128x48 (`frame_rect`/`body_frame`) por layout com margens laterais de 2 colunas e 1 linha, ocupando todo o terminal; bordas de linha fina `┌─┐│└┘`.
2. **`View::Home`** (substitui `View::Title`): painel superior "Biblioteca de downloads" (tabela compacta da sessão), painel intermediário "Histórico de downloads" (completos) e prompt fixo na base com cursor.
3. **`View::Menu` reescrito como popup flutuante**: aberto ao digitar `/` no prompt, lista `COMMANDS` com descrições alinhadas, navegação ↑/↓/j/k, seleção por Enter (executa) ou Tab (completa o prompt), filtragem dinâmica conforme o texto digitado, highlight background no item selecionado.

## Technical Context

**Language/Version**: Rust 2021

**Primary Dependencies**: `crossterm` (Event, KeyCode, Color, cursor), `librqbit`, `tokio`

**Target Platform**: Linux

**Existing Code**: `src/session_ui.rs` — `Tui::handle_title_key`/`handle_menu_key`/`render_title`/`render_menu`, `frame_rect`/`body_frame`/`draw_box`/`frame_top_bottom`, `View` enum, `Frame` (diff-rendering), `Theme`.

## Constitution Check

- **VI. Rust-Skills**: skills consultadas (m01-ownership, m07-concurrency, m12-lifecycle)
- **VIII. Build Velocity**: `cargo check` no loop; perfis otimizados

## Implementation Steps

### Phase 1: Constantes e comandos

- [ ] T001 Definir `COMMANDS: [(&str, &str); 7]` com `/add /list /pause /resume /delete /help /exit`.
- [ ] T002 Atualizar `View` enum: `Title` → `Home`.
- [ ] T003 Ajustar `view_label`/`footer_hints` para `View::Home`.

### Phase 2: Layout full-window reativo

- [ ] T004 Reescrever `frame_rect`/`body_frame`: margens (2 colunas laterais, 1 linha topo/base), sem clamp 128x48.
- [ ] T005 Reescrever `frame_top_bottom`/`draw_box`: bordas finas `┌─┐│└┘` com `THEME.border`.
- [ ] T006 Atualizar testes de `frame_rect`/`body_frame`/bordas.

### Phase 3: Home com painéis

- [ ] T007 `render_home`: desenhar painel Biblioteca (tabela compacta com `session_table_line` sem ações, título do painel) e painel Histórico (itens de `list_completed_downloads` com `format_completed` em verde), dentro de caixas de linha fina; prompt na base com cursor.
- [ ] T008 `handle_home_key` (substitui `handle_title_key`): inserir caracteres no prompt, `/` abre popup, Enter executa comando, Esc limpa, Backspace apaga.
- [ ] T009 Navegação na Biblioteca da Home com ↑/↓ (move `row_index`).

### Phase 4: Menu flutuante de comandos

- [ ] T010 `render_menu` reescrito como popup acima do prompt: lista filtrada de `COMMANDS`, comando à esquerda e descrição à direita, highlight no selecionado.
- [ ] T011 `handle_menu_key`: navegação, filtragem, Enter executa, Tab completa, Esc fecha.
- [ ] T012 `execute_command` (`/add → Include`, `/list → Session`, `/pause`/`/resume`/`/delete` agem na linha selecionada, `/help` notice, `/exit` sai).

### Phase 5: Integração e testes

- [ ] T013 Paste na Home/Menu insere no prompt (e abre popup se `/`).
- [ ] T014 Atualizar fluxos: Esc em Session/Completed volta para Home (não Menu).
- [ ] T015 Testes unitários: comandos filtrados, execução, layout das bordas finas.
- [ ] T016 `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.

## Validation Plan

- `cargo test --locked --all-targets` (testes de layout, comandos e funções puras).
- Smoke test via tmux: Home com painéis, digitar `/`, navegar e executar `/add`.
- CI verde (fmt/clippy/test/machete) antes do merge.
