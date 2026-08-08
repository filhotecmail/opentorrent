# US-019 — Redesign arquitetural do layout TUI com padronização visual, hierarquia e paleta profissional

## Contexto

A interface TUI cresceu incrementalmente (US-009/010 → US-011/012/013 → US-016) e cada tela
adotou seu próprio arranjo visual: título centralizado, menu com quadro duplo, sessão como lista
simples e diálogo de inclusão com quadro. Esta US padroniza a arquitetura visual em **três seções
fixas** (Header, Body, Footer), introduz uma **paleta unificada de tema escuro**, migra as listagens
para **tabelas estruturadas com colunas alinhadas** e transforma o formulário de adição em um
**modal centralizado sobreposto** à tela principal.

## Critérios de aceitação

- **AC-1 — Estrutura em três blocos:** a tela é dividida em Header (título + versão + view atual),
  Body (conteúdo e navegação) e Footer (atalhos + status/aviso), todos fixos e presentes em qualquer
  view.
- **AC-2 — Espaçamentos padronizados:** padding de 1 a 2 caracteres nas bordas do Body e margens
  internas nos blocos de conteúdo.
- **AC-3 — Tabela estruturada:** as listagens da sessão e dos downloads migram para tabelas com
  colunas alinhadas (ID, Progresso, Status, Nome do Arquivo, Ações), sem desalinhamento por
  truncamento.
- **AC-4 — Destaque do item ativo:** o item selecionado em qualquer menu/tabela recebe **fundo de
  realce (highlight background)** e indicador `>`; os demais permanecem neutros.
- **AC-5 — Paleta unificada escura:** bordas em cinza neutro, texto primário branco/prata, acentos
  em ciano, verde para downloads completos, amarelo para avisos, vermelho para erros e highlight
  com contraste.
- **AC-6 — Formulário como modal:** a adição de torrent é um diálogo centralizado com overlay que
  escurece o Body e com caixa de entrada delimitada.

## Cenários de teste

1. **Renderização da estrutura de três blocos:** qualquer tela exibe Header fixo no topo, Footer na
   base e o conteúdo do Body entre eles.
2. **Exibição em tabela alinhada:** com nomes de comprimentos variados, as colunas (ID, Progresso,
   Nome, Ações) permanecem perfeitamente alinhadas, com nomes truncados com `…`.
3. **Destaque de seleção com fundo:** ao navegar por teclado ou mouse, o item focado altera fundo e
   texto com alto contraste em tempo real.

## Decisões de implementação

- `Cell` ganha `bg: Option<Color>`; `Frame` ganha `fill_rect` (usada pelo overlay do modal) e o
  `apply_diff` (US-016) já agrupa runs por `fg`/`bg`.
- `Theme` + `THEME` concentram toda a paleta; `draw_box` usa `THEME.border`; o Footer colore avisos
  em amarelo e erros em vermelho; downloads completos usam `THEME.success` (verde).
- `body_frame` centraliza o quadro da equivalência 1024x768 **dentro do Body** (reaproveita
  `frame_rect` com a altura do Body).
- `session_table_header`/`session_table_line` montam as colunas fixas: marcador(2) + ID(4) +
  progresso(9) + status(13) + nome(n) + [ações(29)], com larguras idênticas entre cabeçalho e dados.
- O modal (`render_include`) preenche o Body com `THEME.overlay_bg` antes de desenhar o diálogo.
- O destaque do item ativo usa `THEME.highlight_bg` (fundo) + texto branco, mantendo o indicador
  `> ... <` do menu.
