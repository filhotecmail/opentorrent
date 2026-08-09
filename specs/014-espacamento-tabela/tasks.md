# US-026 — Tasks

## Implementação

- [x] Adicionar `row_stride` ao `Layout` (1 = consecutivo, 2 = com gap).
- [x] `render_session`: inserir 1 linha em branco (gap) entre as entradas
      (dados + pendentes), exceto após a última.
- [x] Highlight da linha selecionada via `session_row_line` (cobre apenas a linha,
      não o gap).
- [x] Sobreposição colorida da barra de progresso na posição correta
      (`session_row_line`).
- [x] `table_row_to_index`: mapeamento de clique com stride — cliques no gap são
      ignorados.
- [x] Atualizar `mouse_session`, `mouse_menu` e `mouse_include` para o novo
      mapeamento.
- [x] Cabeçalho/separador da tabela inalterados (AC-4).

## Testes

- [x] `session_row_line_accounts_for_vertical_gap`
- [x] `table_row_to_index_maps_with_stride`
- [x] Suíte existente (44 testes) verde + clippy `-D warnings` + machete.

## Publicação

- [ ] Specs em `specs/014-espacamento-tabela/`
- [ ] Commit + push da branch `feat/us-026-espacamento-tabela`
- [ ] PR para `master` com CI verde e merge
- [ ] Build release assinado v0.1.11 + instalação + release no GitHub
