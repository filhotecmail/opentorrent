---

description: "Task list for US-013 - Sincronização do destaque de seleção nos menus"

# Tasks: US-013 (Destaque do menu sincronizado com o cursor)

## Fase 1: Correção do índice de destaque

- [x] T001 Extrair `MENU_ITEMS_OFFSET = 3` (primeira opção do menu na lista de linhas)
- [x] T002 Corrigir `render_menu`: `highlighted.push(MENU_ITEMS_OFFSET + menu_index)` (antes `2 + menu_index` — destacava a linha errada)
- [x] T003 Extrair `menu_item_line(idx, selected)` como função pura

## Fase 2: Atalhos de teclado

- [x] T004 `handle_menu_key` seta `menu_index` antes de `select_menu_item()` (AC-4)
- [x] T005 `select_menu_item()` passa a usar `self.menu_index`

## Fase 3: Ponteiro do mouse

- [x] T006 `mouse_menu`: clique move o cursor (indicador + destaque) no menu
- [x] T007 `mouse_include`: clique nas opções do diálogo move o cursor
- [x] T008 Gravar `Layout` no `render_menu` e no `render_include` (fora do modo digitação)
- [x] T009 `render_include_base` retorna a geometria `(left, top)`

## Fase 4: Validação

- [x] T010 Testes: `menu_item_line_marks_selected_only` (+ constantes de estrutura)
- [x] T011 Pipeline local: fmt, clippy -D warnings, test (16), machete
- [x] T012 CI remoto: fmt, clippy, test, machete, cobertura
