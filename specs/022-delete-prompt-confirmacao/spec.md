# Feature Specification: US-035 — /delete com confirmação no prompt da Home

**Feature Branch**: `feat/us-035-delete-prompt-confirmacao`

**Created**: 2026-08-09

**Status**: Draft

**Input**: User description: "mesma dinâmica do /add para /delete .. ao digitar /delete, ele vai usar a mesma prompt area para perguntar se deseja excluir o torrent selecionado [Y] or [N]"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Confirmação de exclusão no prompt da Home (Priority: P1)

O usuário está na Home, digita `/delete` e pressiona Enter. Em vez de abrir um diálogo central (modal), o prompt fixo da base muda o rótulo para a pergunta de confirmação — `excluir '<nome do torrent selecionado>'? [Y] ou [N]` — e o usuário responde `Y` (confirma e exclui) ou `N` (cancela), permanecendo na Home.

**Why this priority**: É o comportamento central da US — unifica a exclusão no prompt da base, no mesmo padrão do `/add` (US-034), eliminando a troca de contexto para o modal.

**Independent Test**: Na Home, digitar `/delete`, Enter, ver o prompt exibindo a pergunta com o nome do torrent selecionado, responder `Y`, e ver o torrent sumir da Biblioteca com notice de exclusão.

**Acceptance Scenarios**:
1. **Given** a Home com um torrent selecionado na Biblioteca, **When** o usuário digita `/delete` e Enter, **Then** o prompt mostra `excluir '<nome>'? [Y] ou [N]` e a view permanece Home.
2. **Given** a pergunta de confirmação aberta, **When** o usuário pressiona `Y` (ou `y`/`S`/`s`), **Then** o torrent é removido da sessão (e arquivos do disco), um notice confirma a exclusão e o prompt volta ao estado normal.
3. **Given** a pergunta de confirmação aberta, **When** o usuário pressiona `N` (ou `n`) ou `Esc`, **Then** a exclusão é cancelada, um notice informa o cancelamento e o prompt volta ao estado normal.
4. **Given** a pergunta de confirmação aberta, **When** o usuário digita qualquer outra tecla, **Then** a confirmação permanece aberta (aguardando Y/N).
5. **Given** a pergunta de confirmação aberta, **When** o usuário pressiona Ctrl+C, **Then** o aplicativo encerra.

### User Story 2 - Tecla Delete na Session usa a mesma confirmação (Priority: P1)

O usuário que está na view Session e pressiona a tecla `Delete` (atalho de exclusão) vê a mesma pergunta no prompt da Home — sem modal — e responde `Y`/`N`.

**Why this priority**: O usuário decidiu substituir o modal Y/N (US-027) em **todos** os caminhos de exclusão, não apenas no comando — padrão do `/add` (que unificou no prompt).

**Independent Test**: Na Session, posicionar a seleção em um torrent, pressionar `Delete`, ver a Home com a pergunta no prompt, responder `Y`, e confirmar que o torrent saiu da sessão.

**Acceptance Scenarios**:
1. **Given** a view Session com um torrent selecionado, **When** o usuário pressiona `Delete`, **Then** a view transita para a Home e o prompt mostra `excluir '<nome>'? [Y] ou [N]`.
2. **Given** a pergunta de confirmação aberta pela tecla Delete, **When** o usuário responde `Y`, **Then** a exclusão é efetivada (mesmos efeitos do US1).
3. **Given** a pergunta de confirmação aberta pela tecla Delete, **When** o usuário responde `N` ou `Esc`, **Then** a exclusão é cancelada e a Home é exibida com o prompt normal.

### User Story 3 - Modal central removido (Priority: P2)

O diálogo central de confirmação Y/N (US-027), com overlay e `confirm_delete_modal_lines`, deixa de ser exibido em qualquer situação — toda confirmação passa pelo prompt da Home.

**Why this priority**: Limpeza do fluxo legado; evita dois caminhos visuais para a mesma confirmação.

**Acceptance Scenarios**:
1. **Given** qualquer view (Home ou Session), **When** uma confirmação de exclusão é solicitada, **Then** nenhum modal/overlay central é renderizado — apenas o prompt da base.
2. **Given** o código-fonte da US-035, **Then** `render_confirm_delete` e `confirm_delete_modal_lines` deixam de existir (ou ficam sem uso).

---

## Technical Context

**Language/Version**: Rust 2021, toolchain stable

**Primary Dependencies**: `crossterm` (eventos, cor, cursor), `librqbit` 8.1.1 (`Session::delete`, `TorrentIdOrHash`), `tokio` (async)

**Target Platform**: Linux terminal moderno (GNOME Terminal, Windows Terminal, etc.)

**Existing Code**: `src/session_ui.rs` — `Tui` (campos `input`, `prompt_cursor`, `prompt_add_mode`, `confirming_delete`), `handle_home_key`/`handle_session_key` (US-027: S/N/Esc quando `confirming_delete`), `request_delete` (monta `DeleteTarget`), `confirm_delete` (efetiva exclusão e notice), `render_confirm_delete` + `confirm_delete_modal_lines` (modal central), `render_home_prompt` (rótulo `> `/`insira o link:`), `render()` (overlay quando `confirming_delete`). Padrão de prompt modo no fluxo `/add` (US-034): `prompt_add_mode`, `handle_add_prompt_key`, `home_prompt_label`.

## Design Overview

### Modo "excluir?" no prompt da Home

- Estado: reusar `confirming_delete: Option<DeleteTarget>` como flag de modo ativo (ou campo equivalente `confirm_delete_mode`). Quando ativo, `render_home_prompt` exibe a pergunta com o nome do alvo (`excluir '<nome>'? [Y] ou [N]`) como rótulo, suprimindo o placeholder padrão.
- `execute_command("/delete")`: em vez de apenas `request_delete`, ativa a confirmação no prompt — permanece na Home.
- `handle_session_key` tecla `Delete`: `request_delete` + transita para `View::Home` com a confirmação ativa no prompt.
- Teclas no modo de confirmação (Home): `Y`/`y`/`S`/`s` → `confirm_delete()` (efetiva + notice + desativa modo); `N`/`n`/`Esc` → cancela (notice + desativa modo); Ctrl+C → sai; demais teclas ignoradas (confirmação permanece).
- `render_home_prompt`: quando o modo de confirmação está ativo, mostra a pergunta (não aceita texto digitado de origem).
- `render()`: remove o overlay/modal (`render_confirm_delete`); a confirmação aparece só no prompt da Home.
- `render_confirm_delete` e `confirm_delete_modal_lines` (e seus testes) são removidos.

### Reaproveitamento

- `request_delete` e `confirm_delete` permanecem como estão (montagem do alvo e efetivação da exclusão já são corretos).
- O padrão de "modo no prompt" (US-034) é estendido: `home_prompt_label` passa a considerar também o modo de confirmação.
- `DeleteTarget::Pending` (origem pendente em "inicializando"/"erro") segue suportado: a pergunta mostra a origem pendente como nome.

## Out of Scope

- Reduzir "o tamanho das fontes" da lista de processamentos: descartado pelo usuário — uma TUI não controla a fonte do terminal (decisão registrada em Clarifications).
- Alterar `request_delete`/`confirm_delete`/`DeleteTarget` (lógica de exclusão permanece intacta).
- Alterar o layout dos painéis da Home (Biblioteca/Histórico).

## Clarifications

### Session 2026-08-09

- Q: Como não dá para reduzir a fonte em uma TUI, qual abordagem para "aproveitar espaços" na lista de processamentos da visão principal? → A: nada a fazer (parte descartada da US).
- Q: O /delete no prompt substitui o modal Y/N atual (US-027) em todos os caminhos de exclusão, ou mantém o modal para a tecla Delete na view Session? → A: substituir tudo (comando /delete e tecla Delete passam para a confirmação no prompt da Home).
- Q: A confirmação no prompt deve exibir o nome do torrent selecionado na pergunta? → A: sim, com nome.
