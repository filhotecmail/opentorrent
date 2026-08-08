---

description: "Task list for US-015 - Quadro com bordas duplas e área limitada (1024x768)"

# Tasks: US-015 (Quadro com bordas duplas 1024x768)

## Fase 1: Infraestrutura do quadro

- [x] T001 Constantes `FRAME_WIDTH=128` e `FRAME_HEIGHT=48` (equivalência 1024x768 com célula ~8x16px)
- [x] T002 `frame_rect(cols, rows)`: dimensões limitadas ao terminal e centralizadas (sem estouro)
- [x] T003 `frame_top_bottom` + `draw_box`: bordas duplas `╔═╗║╚╝`
- [x] T004 `write_boxed_lines`: escreve conteúdo interno com destaque
- [x] T005 `center_content_top`: centralização vertical do bloco

## Fase 2: Aplicação nas telas

- [x] T006 `render_menu` dentro do quadro (navegação)
- [x] T007 `render_include` dentro do quadro (entrada)
- [x] T008 Campo de entrada com `max_input_width` limitado pelo interior do quadro
- [x] T009 Quebra de linha automática (wrap_text) restrita à largura interna
- [x] T010 Truncamento de todas as linhas para `inner_width` (nada estoura as bordas)
- [x] T011 Modo digitação ancorado no topo (cursor estável); fora dele, conteúdo centralizado

## Fase 3: Cursor e mouse

- [x] T012 `render_input_cursor` posicionado relativo ao interior do quadro
- [x] T013 Remoção de `render_include_base` (substituído pelo render boxed)
- [x] T014 `Layout` (menu/opções) usa `content_top`/`content_left` do quadro

## Fase 4: Validação

- [x] T015 Testes: frame_rect (grande/pequeno/minúsculo), frame_top_bottom, center_content_top
- [x] T016 Pipeline local: fmt, clippy -D warnings, test (21), machete
- [x] T017 CI remoto: fmt, clippy, test, machete, cobertura
