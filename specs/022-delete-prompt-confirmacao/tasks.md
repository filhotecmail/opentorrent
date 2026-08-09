# Tasks: US-035 — /delete com confirmação no prompt da Home

**Feature**: specs/022-delete-prompt-confirmacao | **Fases**: Setup → Prompt → Call-sites → Footer → Integração

## Fase 1: Setup

- [x] T001 [P] Criar arquivos do spec (spec.md, plan.md, research.md, data-model.md, contracts/README.md, quickstart.md, checklists/requirements.md) em `specs/022-delete-prompt-confirmacao/`

## Fase 2: Per pergunta no prompt da Home (substitui o modal)

- [x] T002 [P] Criar função pura `confirm_delete_prompt_label(name: &str) -> String` (`excluir '<name>'? [Y] ou [N] `) em `src/session_ui.rs`
- [x] T003 [P] `render_home_prompt`: quando `confirming_delete.is_some()`, exibir o rótulo da pergunta (sem input e sem cursor) — em `src/session_ui.rs`
- [x] T004 [P] `render()`: remover o bloco que desenha `render_confirm_delete` (overlay) — em `src/session_ui.rs`
- [x] T005 [P] Remover `render_confirm_delete` e `confirm_delete_modal_lines` (código morto) — em `src/session_ui.rs`

## Fase 3: Call-sites de exclusão apontam para a Home

- [x] T006 [P] `execute_command("/delete")`: `request_delete` + `view = View::Home` (cobre o popup) — em `src/session_ui.rs`
- [x] T007 [P] `handle_session_key` `Delete`: `request_delete` + `view = View::Home`; remover o bloco `confirming_delete.is_some()` do Session — em `src/session_ui.rs`
- [x] T008 [P] `mouse_session` botão Excluir (índice 2): `request_delete` + `view = View::Home` — em `src/session_ui.rs`
- [x] T009 [P] `handle_home_key` bloco de confirmação: manter Y/S/N/Esc + adicionar Ctrl+C → sai — em `src/session_ui.rs`

## Fase 4: Hints do Footer

- [x] T010 [P] `footer_hints(view, prompt_add_mode, confirming_delete)`: Home com confirmação → "Y exclui · N/Esc cancela"; atualizar chamada em `render_footer` — em `src/session_ui.rs`

## Fase 5: Integração e qualidade

- [x] T011 [P] Remover testes de `confirm_delete_modal_lines` (3) — em `src/session_ui.rs`
- [x] T012 [P] Testes unitários: `confirm_delete_prompt_label` (nome + `[Y] ou [N]`), `footer_hints` no modo confirmação, ajuste de `session_footer_hints_mention_delete_shortcut` — em `src/session_ui.rs`
- [x] T013 [P] `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test --locked --all-targets` + `cargo machete` + smoke test tmux (5 cenários do quickstart)

## Dependências

```
T002 ──► T003 ──► T004 ──► T005        (prompt: do rótulo à limpeza)
T006/T007/T008 independentes ──► T009   (call-sites: após request_delete intacto)
T002 ──► T010                           (hints: depende do modo em render)
T003–T010 ──► T011/T012 ──► T013
```

Ordem de conclusão das histórias: US1 (prompt) → US2 (call-sites) → US3 (limpeza) → Integração.

## Execução paralela

- T001, T002, T006, T007, T008 independentes entre si — paralelizáveis.
- T011/T012 dependem de T005/T010 — paralelizáveis entre si após essas.

## Estratégia de implementação

- MVP: US1 (T002–T005) entrega o valor central — `/delete` pergunta no prompt da Home sem modal.
- Incremental: US2 (T006–T009) unifica tecla Delete e mouse no mesmo prompt.
- Fechamento: testes + gates de qualidade (T011–T013).
