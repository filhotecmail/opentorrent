# Data Model: US-036 — Menu de contexto com clique direito na Biblioteca

Sem persistência nova. Alterações de estado em `Tui` (src/session_ui.rs):

## `enum ContextAction` (novo)

- Variantes: `Pause`, `Resume`, `Stop`, `Delete`. Deriva `Copy`, `Clone`, `Debug`, `PartialEq`, `Eq`.

## `struct ContextMenu` (novo)

- `entry_index: usize` — índice do item clicado (torrents + pendentes, mesma indexação de `row_index`).
- `selected: usize` — opção destacada (índice em `context_menu_options(entry_index, session_len)`).
- `anchor_row: u16` — linha da tela do item clicado (âncora do popup).

## `Tui` (campos novos)

- `context_menu: Option<ContextMenu>` — `None` no construtor; `Some` apenas enquanto o popup está aberto.
- `context_rect: Option<(left, top, width, height)>` — geometria do popup no último render; `None` quando fechado.

## Invariantes

- `context_menu.is_some()` ⟺ popup aberto.
- Com popup aberto, `context_rect.is_some()` (gravado no mesmo render).
- Com popup aberto, `handle_home_key` processa apenas ↑/↓/Enter/Esc (demais teclas ignoradas).
- Com popup aberto, `handle_mouse`: clique esquerdo dentro de `context_rect` executa a opção; fora fecha.
- `context_menu.entry_index` ∈ [0, session_rows().len() + pending.len()).
- Para `entry_index < session_len` → 4 opções; senão (pendente) → 1 opção (`Delete`).

## Fluxos

- **Abrir**: clique direito na Biblioteca → `row_index = entry_index`, `context_menu = Some(...)`.
- **Executar**: `execute_context_action(action)` → `row_index = entry_index`, `context_menu = None`, `context_rect = None`, dispatch (Pause/Resume/Stop/Delete).
- **Fechar**: Esc ou clique fora → `context_menu = None`, `context_rect = None` (seleção da linha mantida).
