# Research: US-037 — Unificar a interface na Home

## Desconhecidos do Technical Context

Não há NEEDS CLARIFICATION restantes (resolvidos com o usuário na especificação). Descobertas de código:

### Views e dispatch atual

- `enum View` (session_ui.rs:70-82): `Home`, `Menu`, `Session`, `Include`, `Completed`. **Decisão**: reduzir para `Home`/`Menu`.
- `handle_key` (505-518): despacha por view; remove os braços Session/Include/Completed.
- `handle_mouse` (917-942): `MouseEventKind::Down(MouseButton::Left)`; braços Session/Include removidos.
- `render` (1388-1394): `match self.view` — remove Session/Include/Completed.
- `view_label` (2622-2630) e `footer_hints` (2633-2645): variantes removidas.

### Comandos e actions

- `COMMANDS` (25): 8 entradas → 6 (`/add`, `/pause`, `/resume`, `/delete`, `/help`, `/exit`).
- `execute_command` (728-763): branches `/list` (→ `View::Session`) e `/history` (→ `View::Completed`) removidos.
- `handle_session_key` (765-789): traz `p`/`r`/`x`/`Delete` e `a`/`A`/`+` — as ações p/r/x/Delete migram para `handle_home_key`; `a`/`A`/`+` deixam de ter função.
- `handle_include_key`/`handle_include_typing_key`/`handle_completed_key`: removidos com as views.

### Estado órfão

- `Tui` (385-423): `include_index`, `include_input`, `input_cursor_pos`, `typing` — removidos; `prompt_cursor`/`prompt_add_mode`/`confirming_delete` permanecem.
- `submit_include` (1278-1298): removido; `submit_add_from_prompt` (1305-1325) já é o caminho de adição.

### Helpers e constantes órfãos (verificar com `rg` antes de remover)

- `actions_string` (2349), `in_action_button` (2648), `action_button_at` (2656), `session_row_line` (2533), `centered_write` (2317), `wrap_text` (1076), `render_input_cursor` (2185), `body_frame` (2693), `frame_rect` (2685), `frame_top_bottom` (2700), `center_content_top` (2707).
- Constantes: `ACTION_BTN_WIDTH` (37), `ACTION_GAP` (39), `ACTIONS_WIDTH` (41), `ACTION_1_START` (43), `ACTION_2_START` (45).
- `Layout` (149): campos `info_width`/`show_actions` só usados por `mouse_session`/`render_session` — remover campos se órfãos (o `Layout` da Home usa `info_width: 0`/`show_actions: false`).
- `session_table_line` (2581) e `session_table_header` (2586): parâmetro de ações usado apenas pela Session — manter a assinatura? **Decisão**: remover o parâmetro de ações se a Biblioteca não o usar (verificar chamadas; a Home passa `None`).

### Testes afetados (rg)

- `commands_cover_all_shortcuts` (2984): `COMMANDS.len() == 8` → 6.
- `filtered_commands_matches_by_query` (3443): `/li` esperava `/list` → ajustar para `/de`→`/delete` ou `/he`→`/help`.
- `command_item_line_marks_selected_only` (2975): usa `/list` → trocar por comando vigente.
- `session_footer_hints_mention_delete_shortcut` (3366): `View::Session` → substituir por teste dos atalhos da Home.
- `execute_command_notice_for_unknown` (3570): `COMMANDS` iterado — sem mudança estrutural, apenas o número de itens.
- `parse_add_command_returns_none_for_other_commands` (3483): `/list`/`/history` continuam `None` — mantém.

### Alternativas consideradas

- **Manter botões de ação na Biblioteca da Home**: rejeitado — o usuário pediu explicitamente a remoção da coluna de botões; ações via atalhos + menu de contexto (US-036) + comandos.
- **`#[allow(dead_code)]` em órfãos**: rejeitado — constituição/`anti-01`; código morto deve ser removido, não suprimido.
- **Manter `View` com as 3 variantes não usadas**: rejeitado — clippy não acusa enum de campos não usados, mas o usuário pediu remoção completa; enum mínimo `Home`/`Menu` remove estados inválidos (`m05`).

## Decisões consolidadas

- **Decision**: `View` = `{Home, Menu}`; remover views Session/Include/Completed.
- **Rationale**: Home já contém Biblioteca + Histórico; telas separadas são redundantes.
- **Alternatives considered**: manter views (rejeitado — duplicação de render).

- **Decision**: atalhos `p`/`P`, `r`/`R`, `x`/`X`, `Delete` na Home **somente com prompt vazio** (e fora de `prompt_add_mode`/`confirming_delete`).
- **Rationale**: nunca disputar texto digitado no prompt; estados de confirmação já têm teclas próprias (Y/S/N/Esc).
- **Alternatives considered**: atalho com prompt não-vazio (rejeitado — conflito com digitação).

- **Decision**: remover helpers/constantes/estado órfãos na fonte, com gate `cargo clippy -- -D warnings`.
- **Rationale**: constituição VIII + `m15`; remoção reduz compilação e superfície de teste.
- **Alternatives considered**: `#[allow(dead_code)]` (rejeitado).

## Ordem de entrega (com US-036)

- Primeiro US-037 (unificação): remove views, adiciona atalhos — base estável.
- Depois US-036 (menu de contexto): alvo é a Biblioteca da Home que permaneceu.
