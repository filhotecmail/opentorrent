# Research: US-036 — Menu de contexto com clique direito na Biblioteca

## Desconhecidos do Technical Context

Não há NEEDS CLARIFICATION restantes (resolvidos com o usuário na especificação). Descobertas de código:

### Entrada de mouse atual

- `handle_event` (session_ui.rs:459-501): trata `Event::Mouse`; `let click = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left));` — apenas cliques esquerdos redesenham. **Decisão**: incluir `MouseButton::Right` no `click` (abrir popup exige redraw).
- `handle_mouse` (917-942): aceita apenas `MouseButton::Left`; na Home chama `mouse_home`. **Decisão**: aceitar `Right`; clique direito na Home abre o popup de contexto sobre a linha clicada.
- `mouse_home` (944-955): usa `self.layout` (grava em `render_library_panel`, 1600-1608) para mapear linha → índice (`table_row_to_index`). **Decisão**: o clique direito reutiliza exatamente esse mapeamento (`entry_index` = linha clicada) e seta `row_index`.

### Ações reutilizadas

- `pause_row` (1171), `resume_row` (1183), `remove_row` (1194), `request_delete` (1216) — operam sobre `self.row_index`. **Decisão**: `execute_context_action` seta `row_index = context_menu.entry_index` antes de despachar; pendentes tratados pelos branches existentes (notice "aguarde a resolução" / remoção do pendente).

### Padrão de popup (US-031)

- `render_menu` (1760-1845): `fill_rect` com `THEME.popup_bg` + `draw_box` + linhas com highlight (`THEME.highlight_bg`) + grava `self.layout` para clique. **Decisão**: o popup de contexto segue o mesmo padrão, ancorado na linha clicada (não acima do prompt) e gravando `context_rect` em vez de `layout` (para não conflitar com o mapeamento da Biblioteca).

### Indexação de itens

- Biblioteca renderiza torrents + pendentes em sequência (`render_library_panel` 1489-1576), `row_index` único cobre ambos (torrents primeiro, depois pendentes). **Decisão**: `entry_index` usa a mesma indexação; `context_menu_options(entry_index, session_len)` distingue pendente (`entry_index >= session_len`) → só `Excluir`.

### Alternativas consideradas

- **Popup acima do prompt (como o menu de comandos)**: rejeitado — o usuário pediu menu sobre a linha clicada (contexto).
- **Estado via booleano `context_open`**: rejeitado — `Option<ContextMenu>` modela ausência/presença e guarda `entry_index`/`selected`/`anchor_row` (estados inválidos não representáveis, `m05`).
- **Ações via strings**: rejeitado — `enum ContextAction` com `context_action_label` (nome-09/type-09; testável).

## Decisões consolidadas

- **Decision**: `Option<ContextMenu>` + `Option<(u16,u16,u16,u16)>` (`context_rect`) em `Tui`.
- **Rationale**: ausência = fechado; `context_rect` mapeia cliques nas opções sem conflitar com `layout` da Biblioteca.
- **Alternatives considered**: `context_open: bool` (rejeitado — sem dados da âncora/seleção).

- **Decision**: `enum ContextAction { Pause, Resume, Stop, Delete }` com `context_menu_options`/`context_action_label`.
- **Rationale**: tipado, testável puro, sem strings (type-09).
- **Alternatives considered**: strings/índices (rejeitado).

- **Decision**: reutilizar `pause_row`/`resume_row`/`remove_row`/`request_delete` definindo `row_index` antes do dispatch.
- **Rationale**: zero duplicação de lógica de sessão; erros fluem para `handle_event` → notice.
- **Alternatives considered**: duplicar chamadas de `session.pause/delete` no handler (rejeitado — duplicação).

## Dependência de entrega

- Requer US-037 (unificação na Home) entregue: o alvo é a Biblioteca da Home; sem a US-037 o menu atuaria na Session. Ambas podem compartilhar a branch/PR desde que a US-037 venha primeiro (ver `plan.md` do 024).
