# US-020 — Renderização de barra de progresso visual estilo VCL/GUI (Block-based ProgressBar)

## Contexto

A coluna de progresso da tabela da sessão (US-019) exibia apenas o percentual em texto
(`42.5%`). Esta US substitui por um **componente gráfico de barra contínua** baseada em blocos
(`█` preenchido / `░` restante), com **cor por estado do download** e o percentual exato adjacente,
estilo `TProgressBar` de aplicações VCL.

## Critérios de aceitação

- **AC-1 — Barra em blocos contínuos:** substitui o texto simples por `█` (preenchido) e `░`
  (restante), sem caracteres `[====>----]`.
- **AC-2 — Largura da coluna:** a barra ocupa toda a largura da coluna PROGRESSO da tabela,
  adaptando-se ao tamanho da janela (a parte informativa da linha é limitada e o nome absorve o
  espaço restante).
- **AC-3 — Cor por estado:** verde para em andamento/concluído, amarelo para pausado, vermelho
  para erro e ciano (azul) para inicializando — via paleta unificada (`THEME`).
- **AC-4 — Percentual exato:** exibido adjacente à barra (ex.: ` 50.0%`), legível sem poluir o
  preenchimento.

## Cenários de teste

1. **Barra em bloco contínuo (50%):** a metade esquerda preenchida com blocos sólidos coloridos e
   a direita com o traçado neutro (`░`).
2. **Atualização dinâmica:** o avanço do percentual move os blocos da esquerda para a direita de
   forma proporcional a cada frame (diff-rendering da US-016 reescreve apenas a barra).

## Decisões de implementação

- `progress_bar(pct)` monta a barra: `bar_width = PROGRESS_COL_W - 7` blocos + espaço + percentual
  com 1 casa decimal (ex.: ` 50.0%`), sempre com `PROGRESS_COL_W = 31` chars no total.
- `progress_color(state, finished)` mapeia o estado para a cor do tema (verde/amarelo/vermelho/ciano).
- `render_session` monta cada linha com `session_table_line(marker, id, bar, state, name, ...)` e
  depois **sobrepõe a barra com `put_styled`** na coluna 7 com a cor do estado — na linha selecionada
  o fundo de destaque (US-019) é preservado (`bg = THEME.highlight_bg`).
- Colunas redefinidas: prefixo (7) + progresso (31) + status (13) + nome + [ações]; `ROW_FIXED_W = 53`
  e `ROW_INFO_WIDTH = 90`; largura do nome = `info_width - ROW_FIXED_W` (mínimo 1).
- Origens pendentes (US-014) ganham barra 0% com ciano (inicializando) ou vermelho (erro).
- Testes: 4 novos de `progress_bar`/`progress_color` + atualização dos testes de tabela (40 no total).
