# Feature Specification: US-036 — Menu de contexto com clique direito na lista de processamento

**Feature Branch**: `feat/us-036-context-menu-session`

**Created**: 2026-08-09

**Status**: Draft

**Input**: User description: "Implementar suporte a click com o botão direito do mouse nos itens da lista de processamento dos torrents, abrir um menu popup que mostre as opções: Pausar | Retomar | Parar | Excluir"

> **Nota (2026-08-09)**: após a decisão de unificar a interface na Home (US-037, spec 024), a lista de processamento é o painel **Biblioteca da Home**. Este spec passa a ter como alvo a Biblioteca da Home (não mais a view Session).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Menu de contexto com clique direito na lista de processamento (Priority: P1)

O usuário está na Home e clica com o **botão direito** do mouse sobre um torrent da **Biblioteca**. Um menu popup abre sobre a linha clicada, listando `Pausar`, `Retomar`, `Parar` e `Excluir`. O usuário navega com ↑/↓, confirma com Enter (ou clica na opção) e cancela com Esc (ou clica fora).

**Why this priority**: É o comportamento central da US — acesso rápido às ações do item diretamente pelo mouse, sem depender de atalhos de teclado.

**Independent Test**: Na Home, clicar com o botão direito em um torrent da Biblioteca → popup aparece sobre a linha; pressionar ↓↓ Enter → ação executada (ex.: Excluir abre a confirmação no prompt da Home).

**Acceptance Scenarios**:
1. **Given** a Home com um torrent na Biblioteca, **When** o usuário clica com o botão direito sobre a linha do torrent, **Then** um popup abre sobre a linha clicada com `Pausar`, `Retomar`, `Parar` e `Excluir`, e a linha clicada passa a ser a selecionada.
2. **Given** o popup aberto, **When** o usuário pressiona ↑/↓, **Then** a opção destacada se move entre as opções do menu.
3. **Given** o popup aberto com uma opção destacada, **When** o usuário pressiona Enter (ou clica com o botão esquerdo na opção), **Then** a ação é executada e o popup fecha.
4. **Given** o popup aberto, **When** o usuário pressiona Esc (ou clica com o botão esquerdo fora do popup), **Then** o popup fecha sem executar nada e a seleção da linha é mantida.
5. **Given** o popup aberto, **When** o usuário pressiona qualquer outra tecla, **Then** o popup permanece aberto (apenas ↑/↓/Enter/Esc têm efeito).

### User Story 2 - Ações do menu (Priority: P1)

Cada opção do menu executa a mesma ação dos atalhos existentes: `Pausar` pausa, `Retomar` retoma, `Parar` remove da fila mantendo os arquivos, e `Excluir` apaga com a confirmação no prompt da Home (US-035).

**Why this priority**: Semântica já validada — o menu reutiliza as ações existentes sem duplicar lógica.

**Independent Test**: Clicar com o direito num torrent ativo, escolher `Pausar` → notice "pausado: <nome>" e a linha muda para pausado.

**Acceptance Scenarios**:
1. **Given** um torrent ativo, **When** o usuário escolhe `Pausar`, **Then** o torrent é pausado e o notice exibe `pausado: <nome>`.
2. **Given** um torrent pausado, **When** o usuário escolhe `Retomar`, **Then** o torrent é retomado e o notice exibe `retomado: <nome>`.
3. **Given** um torrent na lista, **When** o usuário escolhe `Parar`, **Then** o torrent é removido da fila (arquivos mantidos) e o notice exibe `removido da fila: <nome>`.
4. **Given** um torrent na lista, **When** o usuário escolhe `Excluir`, **Then** o prompt da Home pergunta `excluir '<nome>'? [Y] ou [N]` (fluxo US-035).

### User Story 3 - Torrents pendentes (Priority: P2)

Para origens pendentes (em "inicializando", sem handle), o menu mostra apenas `Excluir`.

**Why this priority**: Consistência com os botões de ação existentes (pendentes não têm handle para pausar/parar).

**Acceptance Scenarios**:
1. **Given** uma origem pendente (inicializando) na Biblioteca, **When** o usuário clica com o botão direito sobre ela, **Then** o popup abre mostrando apenas `Excluir`.
2. **Given** o popup de um pendente aberto, **When** o usuário escolhe `Excluir`, **Then** o prompt da Home abre a confirmação de exclusão da origem pendente.

---

## Technical Context

**Language/Version**: Rust 2021, toolchain stable

**Primary Dependencies**: `crossterm` (eventos de mouse: `MouseButton::Right`), `librqbit` 8.1.1 (`Session::pause`/`unpause`/`delete`), `tokio`

**Target Platform**: Linux terminal moderno

**Existing Code**: `src/session_ui.rs` — `handle_mouse` (hoje só aceita `MouseButton::Left`), `mouse_home` (mapeia clique esquerdo da Biblioteca via `Layout`), `pause_row`/`resume_row`/`remove_row`/`request_delete` (ações por `row_index`), `render_menu` (padrão de popup com `draw_box` + highlight, US-031), `confirm_delete_prompt_label` (US-035). Mouse já habilitado (`EnableMouseCapture` em `main.rs`). Após a US-037, a Biblioteca da Home é a única lista de processamento (render_library_panel), com `Layout` gravado em `self.layout`.

## Design Overview

### Estado do menu de contexto

- Novo campo `context_menu: Option<ContextMenu>` em `Tui`, com:
  - `entry_index: usize` — índice do item clicado (torrents + pendentes, mesma indexação de `row_index`).
  - `selected: usize` — opção destacada (índice em `context_menu_options`).
  - `anchor_row: u16` — linha da tela do item clicado (para ancorar o popup).
- Novo campo `context_rect: Option<(u16, u16, u16, u16)>` — geometria (left, top, width, height) do popup no último render, para mapear cliques do mouse nas opções.

### Abertura (clique direito na Home)

- `handle_mouse`: aceitar `MouseButton::Right` além de `Left`; na Home, clique direito resolve a linha clicada na Biblioteca (mesmo mapeamento de `mouse_home`), define `row_index = entry_index` e abre o `context_menu` com `anchor_row` da linha clicada.
- Opções do menu: helper puro `context_menu_options(entry_index, session_len) -> Vec<ContextAction>` — se `entry_index < session_len` → `[Pause, Resume, Stop, Delete]`; senão (pendente) → `[Delete]`.
- Helper puro `context_action_label(action) -> &'static str`: `Pausar` / `Retomar` / `Parar` / `Excluir`.

### Interação com o popup aberto

- `handle_home_key`: no topo, se `context_menu.is_some()` → ↑/↓ movem `selected`, Enter executa a ação, Esc fecha, demais teclas ignoradas.
- `handle_mouse` com popup aberto: clique esquerdo dentro de `context_rect` executa a opção clicada; clique fora fecha o popup.
- Execução: helper `execute_context_action(action)` — define `row_index = entry_index`, fecha o popup e despacha: `Pause`→`pause_row()`, `Resume`→`resume_row()`, `Stop`→`remove_row()`, `Delete`→`request_delete()` (prompt US-035).
- `handle_event`: o redraw deve considerar também clique direito (`Down(MouseButton::Right)`).

### Render

- Na Home, após `render_library_panel`, se `context_menu.is_some()`: desenhar o popup ancorado em `anchor_row` (estilo `render_menu` — `draw_box` + highlight, fundo `THEME.popup_bg`), com topo no item clicado e clamp vertical dentro do Body. Gravar `context_rect`.
- `footer_hints(View::Home)` com popup aberto: `"↑/↓ navegar · Enter executa · Esc fecha"`.

## Out of Scope

- Alterar a semântica de `pause_row`/`resume_row`/`remove_row`/`request_delete` (ações reutilizadas).
- Alterar o layout da tabela da Biblioteca.
- Clique direito em outros painéis/views (escopo: lista de processamento da Home).

## Clarifications

### Session 2026-08-09

- Q: Como o menu popup deve ser controlado depois de aberto? → A: teclado + clique (↑/↓ + Enter + Esc, padrão do menu de comandos US-031, e clique na opção executa).
- Q: Qual a semântica de `Parar` e `Excluir`? → A: iguais aos atuais — `Parar` remove da fila mantendo arquivos; `Excluir` apaga torrent + arquivos com confirmação no prompt da Home (US-035).
- Q: Onde o popup deve aparecer? → A: sobre a linha clicada (menu de contexto ancorado no item).
- Q: Como tratar pendentes (sem handle)? → A: mostrar apenas `Excluir`.
- Q (US-037): com a unificação na Home, a lista de processamento é a Biblioteca — menu de contexto atua nela.
