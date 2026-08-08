---

description: "Task list for US-009 + US-010 — interface interativa e sessão persistente"
---

# Tasks: US-009 + US-010 (Interface interativa e sessão persistente)

**Input**: Design documents from `/specs/001-tela-inicial-menu/` e `/specs/002-sessao-fila-torrents/`

**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Tests**: testes unitários solicitados na validação (clippy/fmt/test do projeto).

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Estrutura de módulos e dependências

- [ ] T001 Adicionar `src/session_ui.rs` e `src/downloads.rs` ao crate; registrar módulos em `src/main.rs`
- [ ] T002 Tornar `Opts.subcommand` `Option<SubCommand>` (None → modo interativo)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Base de sessão persistente e utilitários que todas as histórias usam

**⚠️ CRITICAL**: nenhum trabalho de user story antes desta fase

- [ ] T003 Implementar sessão persistente: `SessionOptions` com
  `SessionPersistenceConfig::Json { folder: Some(~/.config/opentorrent/session) }`
- [ ] T004 Implementar `downloads.rs::list_completed_downloads(dir)` — varredura
  recursiva com `CompletedItem { path, name, size, created }`
- [ ] T005 Implementar `downloads.rs::format_completed(item)` — formatação com
  size_format e data; testes unitários para ambos
- [ ] T006 Implementar `session_ui.rs::render_welcome()` — arte ASCII
  "OPENTORRENT" centralizada + dicas

**Checkpoint**: fundação pronta — user stories podem começar

---

## Phase 3: User Story US-010 (Tela inicial e menu) — Priority: P1

**Goal**: Tela inicial centralizada com arte ASCII e menu `/` funcional.

**Independent Test**: executar `opentorrent` sem args, ver título, digitar `/`,
ver menu com as 4 opções; atalhos A/C/L/S e navegação ↑/↓/Enter.

### Tests for US-010

- [ ] T007 [P] [US10] Teste unitário de `parse_menu_input` (comandos válidos,
  inválidos → None, texto sem `/` → None)

### Implementation for US-010

- [ ] T008 [US10] Loop de eventos de teclado com crossterm (raw mode, restore
  em todos os caminhos de saída)
- [ ] T009 [US10] Campo de entrada: só processa comandos iniciados com `/`;
  demais teclas ignoradas (FR-003a)
- [ ] T010 [US10] Menu suspenso com 4 opções: Adicionar (A), Acompanhar (C),
  Listar completos (L), Sair (S) — navegação ↑/↓/Enter + atalhos (FR-004a)
- [ ] T011 [US10] Comando desconhecido `/xyz` → ignorar e limpar campo (edge case)
- [ ] T012 [US10] Opção Sair (S) encerra e retorna ao terminal (FR-007)

**Checkpoint**: US-010 funcional e testável isoladamente.

---

## Phase 4: User Story US-009 (Sessão, fila e inclusão) — Priority: P1

**Goal**: Listagem de sessão restaurada com ações por linha e inclusão de novos
torrents sem interromper a fila.

**Independent Test**: executar sem args com torrents prévios, ver restauração;
P/R/X por linha; A/+ abre menu de inclusão; adicionar origem e ver item na fila.

### Implementation for US-009

- [ ] T013 [US9] Restauração automática: confirmar que `Session::new_with_opts`
  com persistence restaura torrents salvos (FR-002, FR-002a)
- [ ] T014 [US9] Listagem de sessão: `session.with_torrents` → linhas com estado,
  percentual, nome; renderização contínua (FR-003)
- [ ] T015 [US9] Ações por linha P (pausar), R (retomar), X (remover da fila,
  delete_files=false) (FR-003a)
- [ ] T016 [US9] Atalho A/+ abre menu de inclusão (FR-004a): opções "digitar
  origem" ou "voltar"
- [ ] T017 [US9] Fluxo de inclusão: entrada de origem → probe list_only →
  re-add com TorrentFileBytes → limpar tela, atualizar lista, continuar fila
  (FR-005, FR-006)
- [ ] T018 [US9] Cancelamento da inclusão volta à listagem sem alterar a fila
  (FR-007)

**Checkpoint**: US-009 funcional e testável isoladamente.

---

## Phase 5: User Story US-010 (Listar completos) — Priority: P2

**Goal**: Opção L lista downloads completos com metadados.

**Independent Test**: selecionar L com downloads no diretório padrão e ver
pasta, nome, tamanho e data.

### Implementation for US-010

- [ ] T019 [US10] Opção L → `list_completed_downloads(default_output_folder)`
  formatada (FR-005, FR-005a)
- [ ] T020 [US10] Lista vazia → mensagem "nenhum download completo" (edge case)

**Checkpoint**: todas as US funcionais.

---

## Phase 6: Integração e validação

- [ ] T021 Integrar menu: "Adicionar" → T017 (US9), "Acompanhar" → T014 (US9),
  "Listar completos" → T019, "Sair" → T012 (FR-006a revisado)
- [ ] T022 `cargo check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo fmt --check`, `cargo test`
- [ ] T023 Teste funcional manual: fluxo completo das duas US
- [ ] T024 Atualizar README com o novo modo interativo

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: sem dependências
- **Foundational (Phase 2)**: depende do Setup — BLOQUEIA as user stories
- **US-010 tela/menu (Phase 3)**: depende do Foundational
- **US-009 (Phase 4)**: depende do Foundational
- **Listar completos (Phase 5)**: depende de US-010 (T006) e T004/T005
- **Integração (Phase 6)**: depende de todas as US

### Within Each User Story

- Núcleo antes de integração
- Story completa antes de avançar para a próxima prioridade

### Parallel Opportunities

- T007 e T009-T012 podem rodar em paralelo com T013-T018 (arquivos/módulos
  distintos: `session_ui.rs` vs persistência)

---

## Notes

- Commitar após cada tarefa ou grupo lógico
- Respeitar convenções do AGENTS.md (sem `.unwrap()` em produção, PT-BR,
  `clippy -D warnings`)
- Não usar `serde` extra — persistência é nativa do librqbit
