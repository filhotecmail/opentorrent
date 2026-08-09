# Tasks: US-034 — /add com prompt "insira o link:" na Home

**Feature**: specs/021-add-prompt-home | **Fases**: Setup → US1 → US2 → US3 → Integração

## Fase 1: Setup

- [x] T001 [P] Criar arquivos do spec (spec.md, plan.md, research.md, data-model.md, contracts/README.md, quickstart.md) em `specs/021-add-prompt-home/`

## Fase 2: Fundação — estado e parse (sem dependências)

- [x] T002 [P] Adicionar campo `prompt_add_mode: bool` a `Tui` (init `false`) em `src/session_ui.rs`
- [x] T003 [P] Criar função pura `parse_add_command(input: &str) -> Option<String>` em `src/session_ui.rs` (extrai origem após `/add `; `None` para `/add` puro, vazio, outros comandos)

## Fase 3: US1 — modo "insira o link:" no prompt da Home

- [x] T004 [US1] `execute_command("/add")`: ativar `prompt_add_mode`, limpar `input`/`prompt_cursor`, `view = View::Home`, `notice = None` (não abre `View::Include`) em `src/session_ui.rs`
- [x] T005 [US1] Novo `handle_add_prompt_key`: Enter → `submit_add_from_prompt(&self.input)`; Esc → cancela modo; chars/backspace → edição do prompt; Ctrl+C → sair — em `src/session_ui.rs`
- [x] T006 [US1] `handle_home_key`: no topo, se `prompt_add_mode`, delega para `handle_add_prompt_key` — em `src/session_ui.rs`
- [x] T007 [US1] `handle_home_key` Enter (modo normal): rotear via `parse_add_command` — inline → submit; `/add` sem origem → ativar modo; senão → `execute_command` — em `src/session_ui.rs`
- [x] T008 [US1] `render_home_prompt`: rótulo "insira o link:" e placeholder suprimido quando `prompt_add_mode` — em `src/session_ui.rs`
- [x] T009 [US1] `footer_hints(View::Home)`: indicar "Esc cancelar" no modo add — em `src/session_ui.rs`

## Fase 4: US2 — submit_add_from_prompt e /add inline

- [x] T010 [US2] Novo `submit_add_from_prompt(&mut self, source: &str) -> bool`: valida `AddTorrent::from_cli_argument` (erro/vazio → notice, texto preservado, `false`); válida → `PendingTorrent` + spawn `add_torrent_background` + `row_index` última + `prompt_add_mode = false` + `true` — em `src/session_ui.rs`
- [x] T015 [US2] Interação filtragem dinâmica × inline (descoberta no smoke test): helper puro `should_open_command_menu` (exceção `/add `) + `leave_menu_for_inline_add` (fecha popup ao digitar/colar o espaço de `/add `) + Backspace da Home reabre popup ao apagar o espaço — em `src/session_ui.rs`

## Fase 5: US3 — regressão da view Include

- [x] T011 [US3] Verificar que `a`/`A`/`+` na Session continuam abrindo `View::Include` (sem alteração de código — validação de regressão em `handle_session_key`; smoke test cenário 5 OK)

## Fase 6: Integração e qualidade

- [x] T012 [P] Testes unitários: `parse_add_command` (inline com/sem origem, `/add `, outros comandos) em `src/session_ui.rs`
- [x] T013 [P] Testes unitários: `home_prompt_label`/`footer_hints` no modo add, `should_open_command_menu` (prefixos vs inline) em `src/session_ui.rs`
- [x] T014 [P] `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test --locked --all-targets` (75 ok) + `cargo machete` + smoke test tmux (5 cenários do quickstart)

## Dependências

```
T002/T003 ──► T004 ──► T005 ──► T006 ──► T007 ──► T008 ──► T009
T003 ──► T010 (parse reutilizado pelo submit? não — T010 só usa o submit)
T004 ──► T010 (submit_add_from_prompt chamado por T005/T007)
T010 ──► T012/T013 (testes do submit)  │ T002–T010 ──► T014
```

Ordem de conclusão das histórias: US1 → US2 → US3 (verificação) → Integração.

## Execução paralela

- T001, T002, T003 independentes (arquivos/estado distintos) — paralelizáveis.
- T012/T013 independentes entre si — paralelizáveis após T003/T010.

## Estratégia de implementação

- MVP: US1 (T004–T009) entrega o valor central — `/add` + Enter mostra "insira o link:" e adiciona sem sair da Home.
- Incremental: US2 (T010) habilita o atalho inline; US3 é regressão (sem código novo).
- Fechamento: testes + gates de qualidade (T012–T014).
