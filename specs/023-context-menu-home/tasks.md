# Tasks: US-036 — Menu de contexto com clique direito na Biblioteca

**Feature**: specs/023-context-menu-home | **Fases**: Setup → Estado → Interação → Render → Integração

**Dependência**: requer a US-037 (unificação na Home) entregue — o menu atua na Biblioteca da Home.

## Fase 1: Setup

- [ ] T001 [P] Criar arquivos do spec (plan.md, research.md, data-model.md, contracts/README.md, quickstart.md, checklists/requirements.md) em `specs/023-context-menu-home/`

## Fase 2: Estado do menu de contexto

- [ ] T002 [P] `enum ContextAction { Pause, Resume, Stop, Delete }` (derive `Copy`) e `struct ContextMenu { entry_index, selected, anchor_row }` em `src/session_ui.rs`
- [ ] T003 [P] Campos `context_menu: Option<ContextMenu>` e `context_rect: Option<(u16,u16,u16,u16)>` em `Tui` (init `None`) em `src/session_ui.rs`
- [ ] T004 [P] Helpers puros `context_menu_options(entry_index, session_len) -> Vec<ContextAction>` e `context_action_label(action) -> &'static str` em `src/session_ui.rs`

## Fase 3: Abertura e interação (mouse + teclado)

- [ ] T005 [P] `handle_mouse`: aceitar `MouseButton::Right`; na Home com popup fechado, clique direito resolve a linha (mesmo mapeamento de `mouse_home`), seta `row_index` e abre `context_menu` (anchor = linha clicada) em `src/session_ui.rs`
- [ ] T006 [P] `handle_mouse` com popup aberto: clique esquerdo dentro de `context_rect` executa a opção clicada; fora fecha em `src/session_ui.rs`
- [ ] T007 [P] `handle_home_key`: com `context_menu.is_some()`, tratar apenas ↑/↓ (mover `selected`), Enter (executar), Esc (fechar) — antes do fluxo normal em `src/session_ui.rs`
- [ ] T008 [P] `handle_event`: `Down(MouseButton::Right)` também marca `redraw = true` em `src/session_ui.rs`
- [ ] T009 [P] `execute_context_action(action)`: seta `row_index = entry_index`, fecha o popup e despacha Pause→`pause_row`, Resume→`resume_row`, Stop→`remove_row`, Delete→`request_delete` em `src/session_ui.rs`

## Fase 4: Render

- [ ] T010 [P] `render_home` (após `render_library_panel`): se `context_menu.is_some()`, desenhar popup ancorado em `anchor_row` (`fill_rect` + `draw_box` + highlight, padrão US-031), clamp vertical, gravando `context_rect` em `src/session_ui.rs`
- [ ] T011 [P] `footer_hints(View::Home)`: quando popup aberto, `"↑/↓ navegar · Enter executa · Esc fecha"` em `src/session_ui.rs`

## Fase 5: Testes e integração

- [ ] T012 [P] Testes unitários: `context_menu_options` (torrent → 4 opções; pendente → `[Delete]`), `context_action_label`, mapeamento de clique na `context_rect` (helper puro) em `src/session_ui.rs`
- [ ] T013 [P] `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test --locked --all-targets` + `cargo machete` + smoke test tmux (6 cenários do quickstart)

## Dependências

```
T002/T003/T004 (estado + helpers puros) ──► T005/T006/T007/T008/T009
T005–T009 ──► T010/T011 (render usa estado)
T002–T011 ──► T012/T013
```

Ordem de conclusão das histórias: US1 → US2 → US3 (abrir → interagir → render) → Integração.

## Execução paralela

- T002, T004, T008 independentes (helpers/evento sem dependência de Tui).
- T005/T006/T007/T009 dependem de T002/T003.
- T010/T011 dependem de T005–T009.

## Estratégia de implementação

- MVP: US1 (T002–T005) entrega o valor central — clique direito abre o popup com as opções.
- Incremental: US2 (T006–T009) habilita teclado + clique nas opções; US3 (T010/T011) é o render do popup.
- Fechamento: testes + gates de qualidade (T012/T013).
