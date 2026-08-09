---

description: "Task list for US-031 - TUI com painéis divididos, prompt na base e menu de comandos flutuante"
---

# Tasks: US-031 (Painéis Biblioteca/Histórico + prompt na base + menu de comandos flutuante)

**Input**: Design documents from `/specs/019-tui-paineis-comandos/`

**Prerequisites**: plan.md, spec.md

## Phase 1: Constantes e comandos

- [ ] T001 Definir `COMMANDS: [(&str, &str); 7]` (`/add /list /pause /resume /delete /help /exit`) com descrições
- [ ] T002 Renomear `View::Title` → `View::Home` (e usos em `view_label`, `footer_hints`, `handle_key`, `render`)
- [ ] T003 Atualizar `MENU_ITEMS`/`MENU_SHORTCUTS` para o novo conjunto de comandos

## Phase 2: Layout full-window reativo (estilo opencode)

- [ ] T004 Reescrever `frame_rect` e `body_frame`: margens fixas (2 colunas laterais, 1 linha), ocupando todo o terminal sem clamp 128x48
- [ ] T005 Trocar bordas duplas por linhas finas em `frame_top_bottom`/`draw_box` (`┌─┐│└┘`)
- [ ] T006 Atualizar testes de `frame_rect`/`body_frame`/bordas

## Phase 3: Home com painéis Biblioteca/Histórico

- [ ] T007 Implementar `render_home`: painel Biblioteca (tabela compacta da sessão) + painel Histórico (completos) com caixas de linha fina
- [ ] T008 Implementar prompt fixo na base com cursor ativo e placeholder "Enter a command or / for options"
- [ ] T009 `handle_home_key`: digitação no prompt, `/` abre popup, Enter executa comando, Esc limpa

## Phase 4: Menu flutuante de comandos

- [ ] T010 Reescrever `render_menu` como popup flutuante acima do prompt (comando à esquerda, descrição à direita, highlight no selecionado)
- [ ] T011 Reescrever `handle_menu_key`: navegação ↑/↓/j/k, filtragem dinâmica, Enter executa, Tab completa, Esc fecha
- [ ] T012 Implementar `execute_command` mapeando `/add /list /pause /resume /delete /help /exit`

## Phase 5: Integração e validação

- [ ] T013 Tratar Paste na Home/Menu (inserir no prompt)
- [ ] T014 Ajustar navegação de retorno (Session/Completed → Home)
- [ ] T015 Testes unitários novos (filtragem de comandos, execução, layout)
- [ ] T016 `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test` + `cargo machete`
