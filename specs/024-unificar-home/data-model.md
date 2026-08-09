# Data Model: US-037 — Unificar a interface na Home

Sem persistência nova. Alterações de estado em `Tui` (src/session_ui.rs):

## `View` (enum)

- **Antes**: `Home`, `Menu`, `Session`, `Include`, `Completed`.
- **Depois**: `Home`, `Menu`.

## `Tui` (campos removidos)

- `include_index: usize` — removido.
- `include_input: String` — removido.
- `input_cursor_pos: usize` — removido.
- `typing: bool` — removido.

Campos preservados: `input`, `prompt_cursor`, `menu_index`, `row_index`, `prompt_add_mode`, `confirming_delete`, `layout`, `pending`, `prev_frame`, `prev_cursor`, `running`.

## Invariantes

- `view ∈ {Home, Menu}` sempre.
- Com `prompt_add_mode == true` → `view == View::Home`.
- Com `confirming_delete.is_some()` → prompt da Home mostra a confirmação Y/N; atalhos p/r/x/Delete **desabilitados**.
- Atalhos de tecla única (`p`/`P`/`r`/`R`/`x`/`X`/`Delete`) **ativos apenas quando** `input.is_empty() && !prompt_add_mode && confirming_delete.is_none()`.

## Ações (não mudam de assinatura)

- `pause_row()` / `resume_row()` / `remove_row()` / `request_delete(row_index)` — reutilizadas pelos atalhos e pelo menu de contexto (US-036).
- `confirm_delete()` — fluxo Y/N intacto (US-035).

## `Layout` (campos)

- Se `info_width`/`show_actions` ficarem sem leitores após a remoção de `mouse_session`/`render_session`: remover do struct e ajustar o construtor da Biblioteca (`info_width: 0`, `show_actions: false` deixam de existir).
