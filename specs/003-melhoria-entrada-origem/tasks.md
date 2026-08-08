---

description: "Task list for US-011 - Melhoria na entrada de texto do prompt de origem"
---

# Tasks: US-011 (Melhoria na entrada de texto - cursor, wrap, paste)

**Input**: Design documents from `/specs/003-melhoria-entrada-origem/`

**Prerequisites**: plan.md, spec.md

## Phase 1: Terminal Setup - Bracketed Paste Mode

- [ ] T001 Habilitar `EnableBracketedPaste` em `run_interactive` (raw mode guard)
- [ ] T002 Adicionar `DisableBracketedPaste` no cleanup (Drop guard)

## Phase 2: Input Field State Enhancement

- [ ] T003 Adicionar `input_cursor_pos: usize` ao struct `Tui`
- [ ] T004 Adicionar `input_prompt_width: usize` ao struct `Tui`
- [ ] T005 Implementar `fn wrap_text(text: &str, width: usize) -> Vec<String>` (soft wrap por palavras)
- [ ] T006 Adicionar cache `input_visible_lines: Vec<String>` ou calcular on-demand no render

## Phase 3: Cursor Positioning on Mode Entry

- [ ] T007 Em `handle_include_key`, ao ativar `typing = true`: setar `input_cursor_pos = include_input.len()`

## Phase 4: Paste Handling

- [ ] T008 No loop principal de eventos (`run_interactive`), adicionar match para `Event::Paste(text)`
- [ ] T009 Inserir texto colado na posição `input_cursor_pos` do `include_input`
- [ ] T010 Atualizar `input_cursor_pos` após inserção
- [ ] T011 Invalidar cache de linhas visuais (ou recalcular no próximo render)

## Phase 5: Soft Wrap Rendering

- [ ] T012 Modificar `render_include` para calcular largura disponível (cols - prompt - margins)
- [ ] T013 Renderizar `include_input` usando `wrap_text` → múltiplas linhas visuais
- [ ] T014 Posicionar cursor na linha/coluna correta baseada em `input_cursor_pos` e wrap
- [ ] T015 Garantir que cursor seja desenhado (crossterm `cursor::MoveTo` + `Show`)

## Phase 6: Footer Positioning

- [ ] T016 Renderizar rodapé **após** as linhas do campo de entrada (offset = 2 + include_index + visible_lines.len())
- [ ] T017 Rodapé não deve ser sobrescrito quando campo expande

## Phase 7: Navigation Keys

- [ ] T018 Backspace: remover char em `input_cursor_pos - 1`, decrementar posição
- [ ] T019 Seta Esquerda/Direita: mover `input_cursor_pos` dentro [0, len]
- [ ] T020 Home/End (Ctrl+A/Ctrl+E): mover para início/fim do texto
- [ ] T021 Enter: confirmar (já existe), Esc: cancelar (já existe)

## Phase 8: Validation

- [ ] T022 `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`
- [ ] T023 `cargo test` (unitários existentes)
- [ ] T024 Teste manual: colar link magnético 500+ chars → wrap, cursor, rodapé OK
- [ ] T024 Teste manual: redimensionar terminal durante edição → re-wrap correto

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1** (Terminal Setup): Sem dependências - pode começar imediatamente
- **Phase 2** (State): Depende de Phase 1 - BLOQUEIA phases 3-7
- **Phase 3** (Cursor Position): Depende de Phase 2
- **Phase 4** (Paste): Depende de Phase 2
- **Phase 5** (Wrap Render): Depende de Phase 2, 3
- **Phase 6** (Footer): Depende de Phase 5
- **Phase 7** (Navigation): Depende de Phase 2, 3
- **Phase 8** (Validation): Depende de todas as anteriores

### Parallel Opportunities

- T003-T006 (state + wrap function) podem ser feitos em paralelo
- T008-T011 (paste) e T012-T015 (render) podem ser desenvolvidos em paralelo após Phase 2
- T018-T021 (navigation) independentes entre si

---

## Notes

- O campo `include_input` já existe no `Tui` - apenas adicionar cursor position
- `wrap_text` deve ser pura (sem side effects) para testabilidade
- Cursor visual: usar `queue!(w, cursor::MoveTo(col, row), cursor::Show)`
- Bracketed paste mode: `execute!(stdout, event::EnableBracketedPaste?)` no setup
- Manter `strip_ansi` nos testes se necessário