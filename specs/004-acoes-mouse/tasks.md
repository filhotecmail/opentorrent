---

description: "Task list for US-012 - Correção do mapeamento e detecção de cliques do mouse"

# Tasks: US-012 (Mouse nas áreas de ação da sessão)

## Fase 1: Infraestrutura de mouse

- [x] T001 Habilitar `EnableMouseCapture` no setup de `run_interactive`
- [x] T002 Desabilitar `DisableMouseCapture` no cleanup (exit + Drop guard)
- [x] T003 Encaminhar `Event::Mouse` no `key_reader`
- [x] T004 Tratar `Event::Mouse` no loop principal → `Tui::handle_mouse`

## Fase 2: Renderização dos botões de ação

- [x] T005 Renderizar `[Pausar ]/[Retomar] [Parar  ] [Excluir]` em cada linha da sessão
- [x] T006 Largura informativa dinâmica (`ROW_INFO_WIDTH` limitada por `cols`)
- [x] T007 Truncar nome do torrent para caber na linha

## Fase 3: Mapeamento de cliques

- [x] T008 Gravar `Layout` (top/left/rows_offset/row_count/info_width) no render da sessão
- [x] T009 Derivar coluna dos botões: `left + info_width + 1`
- [x] T010 Mapear offset → botão via `action_button_at` (função pura)
- [x] T011 Clique na parte informativa move a seleção
- [x] T012 Executar pausar/retomar/parar/excluir conforme o botão
- [x] T013 `clamp_row_index` após remoções por clique

## Fase 4: Versionamento e validação

- [x] T014 `build.sh`: bump de versão patch a cada build + compilação verbose
- [x] T015 Testes unitários: `action_button_at`, `actions_string`, `truncate`
- [x] T016 Pipeline local: fmt, clippy -D warnings, test, machete
- [x] T017 CI remoto: fmt, clippy, test, machete, cobertura
