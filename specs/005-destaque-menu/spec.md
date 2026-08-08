# Feature Specification: Sincronização da estilização visual e destaque de seleção nos menus

**Feature Branch**: `feat/us-013-destaque-menu`

**Status**: Implementado

**Input**: User story US-013 — O destaque de cor do texto deve acompanhar com precisão o item ativamente selecionado pelos cursores de navegação.

## Contexto

O menu interativo renderizava o indicador `> ... <` na opção selecionada, mas o índice da linha destacada era calculado como `2 + menu_index` — enquanto as opções começam na linha **3** (`MENU_ITEMS_OFFSET`). O resultado: o destaque colorido caía sempre na linha **acima** do cursor. A US-013 corrige isso e adiciona:

- Destaque de cor exclusivamente na linha com o cursor de seleção.
- Atualização simultânea (indicador + cor) por teclas, atalhos e ponteiro do mouse.
- Atalhos de teclado (`a`/`c`/`l`/`s`) atualizam o destaque da opção acionada antes de executar o comando.
- Clique do mouse no menu e nas opções do diálogo de inclusão move o cursor.

## Critérios de aceite

1. Destaque exclusivo na linha com o cursor `> ... <` — corrigido: `highlighted.push(MENU_ITEMS_OFFSET + menu_index)`.
2. Itens não selecionados mantêm estilização padrão — apenas o índice destacado recebe cor (via `centered_write`).
3. Teclas de navegação e ponteiro do mouse atualizam indicador + cor em tempo real — `menu_index` é a fonte única para ambos; `mouse_menu`/`mouse_include` movem o cursor por clique.
4. Atalhos de caractere atualizam o destaque antes de executar — `handle_menu_key` seta `menu_index = idx` e então chama `select_menu_item()` (que passa a usar `self.menu_index`).

## Cenários de teste

- Seta para baixo move indicador e cor juntos para a segunda opção, removendo o destaque da primeira.
- Renderização inicial destaca apenas a primeira opção (índice 0 → linha 3).
- Teste unitário `menu_item_line_marks_selected_only` garante cursor e estrutura das linhas.

## Notas técnicas

- `menu_item_line(idx, selected)` é função pura testável; o índice de linha do destaque usa a mesma fonte (`menu_index`) do indicador.
- `Layout` do menu: `rows_offset = MENU_ITEMS_OFFSET`, `row_count = MENU_ITEMS.len()` — reutiliza a infraestrutura de mouse da US-012.
