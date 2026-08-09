# US-019 — Tasks

## Implementação

- [x] `Cell` com `bg: Option<Color>` e `Frame::fill_rect` (overlay do modal)
- [x] Paleta `Theme` + `THEME` (bordas, texto, acento, sucesso, aviso, erro, muted, highlight, header, footer, overlay)
- [x] `render()` com Header/Body/Footer fixos; `render_header`/`render_footer` com fundos do tema
- [x] `body_frame` (quadro 1024x768 contido no Body) e `frame_rect` reaproveitada
- [x] Tabela estruturada na sessão: `session_table_header`/`session_table_line` (ID, Progresso, Status, Nome, Ações)
- [x] Marcador de linha com 2 chars (`> `/`  `) alinhado ao cabeçalho da tabela
- [x] Modal centralizado no include com overlay (`render_include` escurece o Body)
- [x] `render_completed` dentro do Body com itens em verde (sucesso) e estado vazio destacado
- [x] `draw_box` com `THEME.border`; destaque do item ativo com `THEME.highlight_bg`
- [x] Testes: `body_frame`, `session_table_*`, `fill`/`fill_rect` (36 no total)
- [x] Pipeline local: fmt, clippy (-D warnings), test (36), machete — OK

## Publicação

- [ ] Commit + push da branch `feat/us-019-redesign-tui`
- [ ] PR para master com vinculação da US
- [ ] Merge squash após CI verde
- [ ] Master: build release assinado v0.1.7 (bump automático)
- [ ] Instalar binário em `/usr/local/bin`
- [ ] Release v0.1.7 com binário + assinatura `.sig`
- [ ] Smoke test tmux do redesign (Header/Body/Footer, tabela, modal) + revisão final
