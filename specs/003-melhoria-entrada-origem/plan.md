# Implementation Plan: US-011 - Melhoria na entrada de texto do prompt de origem

**Branch**: `feat/us-011-input-improvements` | **Date**: 2026-08-08

**Input**: Feature specification from `specs/003-melhoria-entrada-origem/spec.md`

## Summary

Melhorar o campo de entrada de origem no menu de inclusão (Include view) com:
1. Posicionamento automático do cursor ao entrar no modo de digitação
2. Quebra de linha visual automática (soft wrap) para textos longos
3. Preservação do layout da UI (rodapé fixo abaixo do campo)
4. Suporte a bracketed paste mode para colagem limpa

## Technical Context

**Language/Version**: Rust 2021, toolchain stable 1.97.1

**Primary Dependencies**: `crossterm 0.28.1` (Event::Paste, EnableBracketedPaste), `tokio`

**Target Platform**: Linux terminal moderno (GNOME Terminal, Windows Terminal, etc.)

**Existing Code**: `src/session_ui.rs` - Include view com `typing` state e `include_input` String

## Constitution Check

- **VI. Rust-Skills**: skills consultadas (m01-ownership, m07-concurrency, m12-lifecycle)
- **VII. Cargo-Skill**: `cargo skill think` rodado, contexto ativo
- **VIII. Build Velocity**: `cargo check` no loop; perfis otimizados

## Rust Skills Check

| Skill | Layer | Decision/Impact |
|-------|-------|-----------------|
| m01-ownership | 1 | `include_input` permanece `String` owned; wrap view usa slices |
| m07-concurrency | 1 | Key reader thread + async event loop; sem lock atravessando await |
| m12-lifecycle | 2 | Terminal state (raw mode, bracketed paste) gerenciado por RAII guards |
| m06-error-handling | 1 | `anyhow::Result` propagado; sem unwrap em produção |

## Implementation Plan

### Phase 1: Terminal Setup - Bracketed Paste Mode

1. Modificar `run_interactive` para habilitar `EnableBracketedPaste` ao entrar no raw mode
2. Adicionar `DisableBracketedPaste` no cleanup (Drop guard)

### Phase 2: Input Field State Enhancement

1. Adicionar campos ao `Tui`:
   - `input_cursor_pos: usize` - posição do cursor no texto lógico (bytes)
   - `input_prompt_width: usize` - largura do prompt "origem: " para offset do cursor
   - `input_visible_lines: Vec<String>` - linhas visuais após wrap (cache para render)

2. Função `wrap_input_text(text: &str, available_width: usize) -> Vec<String>` para soft wrap

### Phase 3: Cursor Positioning on Mode Entry

1. Em `handle_include_key` ao entrar em `typing = true`:
   - `input_cursor_pos = include_input.len()` (fim do texto)
   - Garantir que cursor seja renderizado na posição correta

### Phase 4: Paste Handling (Event::Paste)

1. No loop principal de eventos, adicionar match para `Event::Paste(text)`:
   - Inserir texto na posição do cursor
   - Atualizar `input_cursor_pos`
   - Recalcular wrap

### Phase 5: Soft Wrap Rendering

1. Modificar `render_include` para:
   - Calcular largura disponível para o campo (terminal width - prompt - margins)
   - Chamar `wrap_input_text` para obter linhas visuais
   - Renderizar cada linha visual em linha separada
   - Posicionar cursor na linha/coluna correta baseada em `input_cursor_pos`

### Phase 6: Footer Positioning

1. Rodapé ("Enter para confirmar, Esc para cancelar") renderizado **após** as linhas do campo de entrada
2. Calcular offset vertical dinamicamente baseado em `input_visible_lines.len()`

### Phase 7: Navigation Keys (Backspace, Left/Right, Home/End)

1. Atualizar handlers para navegação por índice lógico + conversão para posição visual
2. Backspace: remover char anterior ao cursor, ajustar posição
3. Setas: mover `input_cursor_pos` dentro dos limites [0, len]

### Phase 8: Validation

1. `cargo check`, `clippy -D warnings`, `fmt`, `test`
2. Teste manual: colar link magnético 500+ chars, verificar wrap, cursor, rodapé

---

## Files to Modify

- `src/session_ui.rs` - Lógica principal da TUI (Include view, input handling, render)

---

## Complexity Tracking

> Sem violações de constituição estimadas. A complexidade está no cálculo de wrap visual e posicionamento de cursor, mas é contida no módulo `session_ui.rs`.

---

## Test Scenarios Mapping

| Scenario | Implementation |
|----------|----------------|
| Cursor auto-position on mode entry | `handle_include_key` seta `input_cursor_pos` ao ativar `typing` |
| Long paste with wrap | `Event::Paste` + `wrap_input_text` + render multi-linha |
| Footer fixed below input | Render footer após `input_visible_lines.len()` linhas |
| Navigation keys | Handlers atualizam `input_cursor_pos` corretamente |