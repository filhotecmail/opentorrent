---

description: "Task list for US-033 - Métricas na Biblioteca, histórico tabular e /add via prompt"
---

# Tasks: US-033 (Métricas Biblioteca + Histórico tabular + /add via prompt)

**Input**: Design documents from `/specs/020-metricas-biblioteca-add-prompt/`

**Prerequisites**: plan.md, spec.md

## Phase 1: Métricas na Biblioteca

- [ ] T001 Estender `SessionRow` em `src/session_ui.rs` com `down_speed_mbps`/`up_speed_mbps` (`Option<f64>`), `eta_seconds` (`Option<u64>`) e `uploaded_bytes` (`u64`); preencher em `session_rows()` a partir de `stats.live`
- [ ] T002 Criar funções puras `format_speed`, `format_eta` e `format_ratio` (guarda divisão por zero) + testes
- [ ] T003 Adicionar colunas de métricas a `session_table_header`/`session_table_line` mantendo alinhamento

## Phase 2: Histórico tabular

- [ ] T004 Criar `history_table_header` e `history_table_line` (ID/NOME/TAMANHO/RATIO/DATA CONCLUSÃO/AÇÕES)
- [ ] T005 Reescrever `render_history_panel` em `src/session_ui.rs` para montar a tabela (RATIO cruzado com sessão quando finished)
- [ ] T006 Adicionar `DeleteTarget::File { path, name }` e branch de `std::fs::remove_file` em `confirm_delete`

## Phase 3: /add via prompt

- [ ] T007 `execute_command("/add")` em `src/session_ui.rs`: preencher `input = "/add "`, `prompt_cursor = 5`, `view = View::Home`
- [ ] T008 `handle_home_key` Enter: rotear `/add <origem>` para `submit_add_from_prompt()`; `/add` vazio → notice
- [ ] T009 Criar `submit_add_from_prompt`: valida sintaxe preservando prompt em erro, registra pendente, spawn assíncrono, permanece na Home

## Phase 4: Remoção de View::Include

- [ ] T010 Remover variante `View::Include` e membros `include_index`, `include_input`, `typing`
- [ ] T011 Remover `handle_include_key`, `handle_include_typing_key`, `render_include`, `render_input_cursor`, `mouse_include`, `submit_include` e branches órfãos
- [ ] T012 Redirecionar atalhos legados (`a`/`+` em Session) para o fluxo do prompt

## Phase 5: Integração e validação

- [ ] T013 Testes unitários novos (métricas, histórico, fluxo /add)
- [ ] T014 Atualizar testes existentes afetados (assinatura `session_table_line`, remoção de `View::Include`)
- [ ] T015 `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test` + `cargo machete`
