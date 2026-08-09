# Tasks: US-037 — Unificar a interface na Home

**Feature**: specs/024-unificar-home | **Fases**: Setup → Remoções → Atalhos → Limpeza → Integração

## Fase 1: Setup

- [ ] T001 [P] Criar arquivos do spec (plan.md, research.md, data-model.md, contracts/README.md, quickstart.md, checklists/requirements.md) em `specs/024-unificar-home/`

## Fase 2: Remoções estruturais

- [ ] T002 [P] `View`: remover variantes `Session`, `Include`, `Completed` (fica `Home`/`Menu`) em `src/session_ui.rs`
- [ ] T003 [P] `COMMANDS`: remover `/list` e `/history` (8 → 6) em `src/session_ui.rs`
- [ ] T004 [P] `execute_command`: remover branches `/list` (→ `View::Session`) e `/history` (→ `View::Completed`) em `src/session_ui.rs`
- [ ] T005 [P] `handle_key`/`handle_mouse`/`render`: remover braços de dispatch Session/Include/Completed em `src/session_ui.rs`
- [ ] T006 [P] Remover handlers `handle_session_key`, `handle_include_key`, `handle_include_typing_key`, `handle_completed_key` em `src/session_ui.rs`
- [ ] T007 [P] Remover `mouse_session`, `mouse_include` e `submit_include` em `src/session_ui.rs`
- [ ] T008 [P] Remover renders `render_session`, `render_include`, `render_completed`, `render_input_cursor` em `src/session_ui.rs`
- [ ] T009 [P] Remover campos `include_index`, `include_input`, `input_cursor_pos`, `typing` de `Tui` (declaração, init e usos) em `src/session_ui.rs`
- [ ] T010 [P] `view_label`/`footer_hints`: remover variantes Session/Include/Completed em `src/session_ui.rs`

## Fase 3: Atalhos de tecla única na Home

- [ ] T011 [P] Helper puro `home_action_for_key(key_code, prompt_empty, prompt_add_mode, confirming_delete) -> Option<HomeAction>` em `src/session_ui.rs`
- [ ] T012 [P] `handle_home_key`: aplicar `home_action_for_key` antes do fluxo normal (p/P pause, r/R resume, x/X remove, Delete → request_delete) — apenas com prompt vazio, fora de add mode/confirmação em `src/session_ui.rs`
- [ ] T013 [P] `footer_hints(View::Home)`: `"p pausar · r retomar · x remover · Del excluir · / comandos · ↑/↓ seleciona"` em `src/session_ui.rs`

## Fase 4: Limpeza de órfãos (gate clippy)

- [ ] T014 [P] Remover helpers órfãos (confirmar com `rg`): `actions_string`, `in_action_button`, `action_button_at`, `session_row_line`, `centered_write`, `wrap_text`, `body_frame`, `frame_rect`, `frame_top_bottom`, `center_content_top` em `src/session_ui.rs`
- [ ] T015 [P] Remover constantes de ações (`ACTION_BTN_WIDTH`, `ACTION_GAP`, `ACTIONS_WIDTH`, `ACTION_1_START`, `ACTION_2_START`) e campos `info_width`/`show_actions` de `Layout` se órfãos (ajustar `render_library_panel`) em `src/session_ui.rs`
- [ ] T016 [P] Remover parâmetro de ações de `session_table_line`/`session_table_header` se a Biblioteca for a única chamadora (ajustar chamadas/testes) em `src/session_ui.rs`

## Fase 5: Testes e integração

- [ ] T017 [P] Atualizar testes: `COMMANDS.len()` → 6, `command_item_line` (trocar `/list`), `filtered_commands` (`/li` → comando vigente), `session_footer_hints_mention_delete_shortcut` → atalhos da Home em `src/session_ui.rs`
- [ ] T018 [P] Remover testes órfãos: `actions_string_*`, `session_row_line_*`, `action_button_at` tests, `frame_rect_*`, `body_frame_*`, `center_content_top_*`, `wrap_text_*` em `src/session_ui.rs`
- [ ] T019 [P] Testes novos: `home_action_for_key` (prompt vazio vs digitado, add mode, confirmação), `footer_hints` com atalhos em `src/session_ui.rs`
- [ ] T020 [P] `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test --locked --all-targets` + `cargo machete` + smoke test tmux (6 cenários do quickstart)

## Dependências

```
T002–T010 (remoções) ──► T011 ──► T012 ──► T013
T002–T010 ──► T014/T015/T016 (órfãos só aparecem após remoções)
T002–T016 ──► T017/T018/T019 ──► T020
```

Ordem de conclusão das histórias: US1 → US2 → US3 (remoções → atalhos → limpeza) → Integração.

## Execução paralela

- T002, T003, T004, T006, T007, T008 são remoções em blocos distintos — podem ser preparadas em paralelo mas devem ser aplicadas num único fluxo (o compilador acusa referências pendentes entre elas).
- T011 é pura (sem dependências de Tui) — pode ser feita antes das remoções.
- T014/T015/T016 dependem do estado final pós-remoção.
- T017/T018/T019 dependem de T002–T016.

## Estratégia de implementação

- **Um único fluxo de edição** para T002–T010: como as remoções são interdependentes (handlers chamam métodos, renders usam helpers), o código não compila entre passos intermediários — aplicar numa sequência única e rodar `cargo check` ao final da fase.
- Atalhos (T011–T013) adicionam comportamento novo sobre a base limpa.
- Limpeza (T014–T016): remover cada órfão apenas após `rg` confirmar zero usos.
- Fechamento: testes + gates de qualidade (T017–T020).
