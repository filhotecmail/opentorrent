# Feature Specification: US-037 — Unificar a interface na Home (remover telas Sessão/Completos/Incluir)

**Feature Branch**: `feat/us-037-unificar-home`

**Created**: 2026-08-09

**Status**: Draft

**Input**: User description: "Remover as telas separadas (Sessão via /list, Completos via /history e Incluir via a/A/+) e unificar tudo na Home. As ações Pausar/Retomar/Parar/Excluir saem dos botões da Sessão e entram no menu de contexto (clique direito, US-036) e em atalhos de teclado na própria Home."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Home é a única tela (Priority: P1)

O usuário abre o cliente e a Home mostra a Biblioteca de downloads (tabela com estado, barra, métricas) e o Histórico de completos. Não existem mais as telas "Sessão" (`/list`), "Completos" (`/history`) nem "Incluir" (`a`/`A`/`+`). Todos os comandos do menu flutuante são: `/add`, `/pause`, `/resume`, `/delete`, `/help`, `/exit`.

**Why this priority**: É o núcleo da unificação — remoção estrutural que simplifica o modelo mental do usuário (uma tela, um prompt).

**Independent Test**: Executar o cliente; digitar `/` → o menu flutuante lista apenas 6 comandos (sem `/list` e `/history`). Digitar `a`, `A` ou `+` na Home → nada de tela separada abre; o caractere vai para o prompt.

**Acceptance Scenarios**:
1. **Given** a Home, **When** o usuário digita `/`, **Then** o popup de comandos lista apenas `/add`, `/pause`, `/resume`, `/delete`, `/help`, `/exit`.
2. **Given** a Home, **When** o usuário executa `/list` ou `/history`, **Then** a mensagem de comando desconhecido é exibida (não abre tela).
3. **Given** a Home, **When** o usuário pressiona `a`, `A` ou `+`, **Then** nenhuma view separada abre — o caractere segue o fluxo normal do prompt.
4. **Given** a Home, **When** o usuário pressiona Esc, **Then** nada muda (a Home não sai para outra tela).
5. **Given** qualquer estado, **When** o usuário pressiona Ctrl+C, **Then** o cliente encerra.

### User Story 2 - Ações na própria Home (Priority: P1)

Pausar/Retomar/Parar/Excluir passam a funcionar na Home: **atalhos de tecla única quando o prompt está vazio** (`p`/`P` pausa, `r`/`R` retoma, `x`/`X` para, `Delete` exclui) e via **menu de contexto com clique direito** (US-036). Com texto no prompt, as teclas são digitadas normalmente. Os comandos `/pause`, `/resume`, `/delete` continuam disponíveis.

**Why this priority**: Garante que a unificação não perde nenhuma ação — tudo que a Sessão fazia é acessível na Home.

**Independent Test**: Na Home com um torrent ativo selecionado, pressionar `p` com o prompt vazio → notice "pausado: <nome>"; pressionar `r` → "retomado: <nome>"; `x` → "removido da fila"; `Delete` → confirmação no prompt.

**Acceptance Scenarios**:
1. **Given** a Home com o prompt vazio e um torrent ativo selecionado, **When** o usuário pressiona `p` ou `P`, **Then** o torrent é pausado (notice `pausado: <nome>`).
2. **Given** a Home com o prompt vazio e um torrent pausado selecionado, **When** o usuário pressiona `r` ou `R`, **Then** o torrent é retomado (notice `retomado: <nome>`).
3. **Given** a Home com o prompt vazio e um torrent selecionado, **When** o usuário pressiona `x` ou `X`, **Then** o torrent é removido da fila mantendo os arquivos (notice `removido da fila: <nome>`).
4. **Given** a Home com o prompt vazio e um item selecionado, **When** o usuário pressiona `Delete`, **Then** o prompt da Home mostra `excluir '<nome>'? [Y] ou [N]` (fluxo US-035).
5. **Given** a Home com **texto digitado no prompt** (ex.: `/add`), **When** o usuário pressiona `p`, `r` ou `x`, **Then** o caractere é digitado no prompt — nenhuma ação de torrent é disparada.
6. **Given** a Home com um item selecionado, **When** o usuário clica com o botão direito sobre ele, **Then** o menu de contexto abre com as ações (US-036).

### User Story 3 - Botões de ação e view Session removidos (Priority: P2)

A coluna de botões `[Pausar]/[Retomar] [Parar] [Excluir]` da view Sessão é removida. A Biblioteca da Home permanece sem botões (já o caso atual). Pendentes (em "inicializando") continuam exibidos na Biblioteca e suportam remoção/exclusão via atalhos/menu, com aviso de "aguarde a resolução" para pausar/retomar.

**Why this priority**: Consistência da unificação — nenhum resquício de tela separada ou de botões de ação.

**Acceptance Scenarios**:
1. **Given** a Biblioteca da Home em terminal largo, **When** o usuário procura a coluna de botões de ação, **Then** ela não existe (a linha mostra marcador, estado, barra, nome e métricas apenas).
2. **Given** uma origem pendente (inicializando) selecionada na Home, **When** o usuário pressiona `p` ou `r`, **Then** o notice exibe `torrent em inicialização — aguarde a resolução` (nenhum crash).
3. **Given** uma origem pendente selecionada na Home, **When** o usuário pressiona `x` ou `Delete`, **Then** a requisição pendente é removida/descartada com a confirmação de exclusão (fluxo existente).

## Technical Context

**Language/Version**: Rust 2021, toolchain stable

**Primary Dependencies**: `crossterm` (eventos de teclado e mouse), `librqbit` 8.1.1, `tokio`

**Target Platform**: Linux terminal moderno

**Existing Code**: `src/session_ui.rs` — enum `View` (Home, Menu, Session, Include, Completed), `COMMANDS` (8 entradas), `handle_session_key`/`handle_include_key`/`handle_include_typing_key`/`handle_completed_key`, `mouse_session`/`mouse_include`, `render_session`/`render_include`/`render_completed`/`render_input_cursor`, `submit_include`, campos `include_index`/`include_input`/`input_cursor_pos`/`typing`, helpers órfãos (`actions_string`, `action_button_at`, `in_action_button`, `session_row_line`, `centered_write`, `wrap_text`, `render_input_cursor`, `body_frame`, `frame_rect`/`frame_top_bottom`/`center_content_top` parcialmente), constantes de ações (`ACTIONS_WIDTH`, `ACTION_BTN_WIDTH`, `ACTION_GAP`, `ACTION_1_START`, `ACTION_2_START`). A Home (`render_home`) já contém Biblioteca (`render_library_panel`) e Histórico (`render_history_panel`).

## Design Overview

### Remoções estruturais

- `View`: reduzir para `Home` e `Menu` (remover `Session`, `Include`, `Completed`).
- `COMMANDS`: reduzir para 6 entradas (`/add`, `/pause`, `/resume`, `/delete`, `/help`, `/exit`); remover os branches `/list` e `/history` de `execute_command`.
- Remover handlers/renders/mouse das views removidas e `submit_include`, `render_input_cursor`.
- Remover campos `include_index`, `include_input`, `input_cursor_pos`, `typing` de `Tui`.
- Remover helpers e constantes que ficam órfãos (verificados por `cargo clippy -- -D warnings`): `actions_string`, `in_action_button`, `action_button_at`, `session_row_line`, `centered_write`, `wrap_text`, `body_frame`, `frame_rect`, `frame_top_bottom`, `center_content_top` (após conferir usos), `ACTIONS_WIDTH`, `ACTION_BTN_WIDTH`, `ACTION_GAP`, `ACTION_1_START`, `ACTION_2_START`, `ACTION_*` e testes associados.
- `view_label`: apenas `Home → "INÍCIO"` e `Menu → "COMANDOS"`.
- `footer_hints`: remover as variantes Session/Include/Completed; `View::Home` ganha os novos atalhos.

### Atalhos de tecla única na Home (prompt vazio)

- Em `handle_home_key`, antes do fluxo normal: se `!prompt_add_mode`, `!confirming_delete` e `input.is_empty()`:
  - `p`/`P` → `pause_row().await`
  - `r`/`R` → `resume_row().await`
  - `x`/`X` → `remove_row().await`
  - `Delete` → `request_delete(self.row_index)` (confirmação no prompt, US-035)
- `footer_hints(View::Home, prompt_add_mode=false, confirming_delete=false)`:
  `"p pausar · r retomar · x remover · Del excluir · / comandos · ↑/↓ seleciona"`.

### Mouse na Home

- `handle_mouse`: manter `View::Home` → `mouse_home` (clique esquerdo move seleção); `View::Menu` → `mouse_menu`. Remover os braços Session/Include.
- Clique **direito** na Biblioteca abre o menu de contexto (US-036 — ver spec 023); `handle_event` deve tratar `Down(MouseButton::Right)` como evento que redesenha.

### Biblioteca permanece sem ações

- `render_library_panel` não muda o layout (já passa `None` em ações); a tabela segue com marcador/estado/barra/nome/métricas. Nada de coluna de botões.

## Out of Scope

- Alterar o layout dos painéis da Home (Biblioteca/Histórico).
- Alterar a semântica de `pause_row`/`resume_row`/`remove_row`/`request_delete`/`confirm_delete`.
- Alterar `/add`/`/pause`/`/resume`/`/delete`/`/help`/`/exit` do prompt.
- O menu de contexto em si (ver spec 023, US-036).

## Clarifications

### Session 2026-08-09

- Q: O que fazer com `/list` e `/history`? → A: remover por completo (menu e execução); a Home já mostra Biblioteca + Histórico.
- Q: E a view Incluir (`a`/`A`/`+`)? → A: remover por completo; a adição é só via `/add` no prompt da Home (US-034).
- Q: Como Pausar/Retomar/Parar/Excluir funcionam sem a Sessão? → A: atalhos de tecla única na Home quando o prompt está vazio (p/P, r/R, x/X, Delete) + menu de contexto com clique direito (US-036) + comandos `/pause` `/resume` `/delete`.
- Q: A Biblioteca da Home ganha botões de ação? → A: não — a coluna de botões é removida; ações ficam nos atalhos, no menu de contexto e nos comandos do prompt.
