# Contracts: US-036 — Menu de contexto com clique direito na Biblioteca

Contratos internos do módulo `session_ui` (helpers puros — sem interface externa nova).

## `context_menu_options(entry_index: usize, session_len: usize) -> Vec<ContextAction>`

- **Entrada**: índice do item clicado + número de torrents da sessão (`session_rows().len()`).
- **Saída**:
  - `entry_index < session_len` → `[Pause, Resume, Stop, Delete]`.
  - `entry_index >= session_len` (pendente) → `[Delete]`.
- **Contrato de teste**:
  - `context_menu_options(0, 3)` → 4 opções.
  - `context_menu_options(3, 3)` → `[Delete]` (pendente).
  - `context_menu_options(5, 3)` → `[Delete]`.

## `context_action_label(action: ContextAction) -> &'static str`

- `Pause` → `"Pausar"`, `Resume` → `"Retomar"`, `Stop` → `"Parar"`, `Delete` → `"Excluir"`.

## `Tui::execute_context_action(action: ContextAction)`

- **Pré**: `self.context_menu` é `Some`.
- **Pós**: `row_index = entry_index`; popup fechado (`context_menu = None`, `context_rect = None`); dispatch:
  - `Pause` → `pause_row().await` (notice "pausado: <nome>" ou aviso de pendente).
  - `Resume` → `resume_row().await` (notice "retomado: <nome>").
  - `Stop` → `remove_row().await` (notice "removido da fila: <nome>").
  - `Delete` → `request_delete(entry_index)` (confirmação Y/N no prompt da Home, US-035).
- **Pós (erro async)**: erro propagado via `Result` → `handle_event` → notice.

## Mouse

- `handle_mouse`: aceita `Down(MouseButton::Left | MouseButton::Right)`. Home + clique direito → abre popup. Popup aberto + clique esquerdo dentro de `context_rect` → executa a opção na linha clicada; fora → fecha.
- `handle_event`: `Down(MouseButton::Right)` conta como clique (redraw = true).
