# US-026 — Adição de espaçamento vertical e separadores de linha na tabela de downloads

## História

Como **Usuário do sistema**, eu quero que as linhas da tabela de downloads possuam
espaçamento vertical (padding/row gap) ou linhas separadoras sutis entre os itens,
para que as barras de progresso contíguas de cada torrent não fiquem coladas
verticalmente em um único bloco contínuo, melhorando a legibilidade e a distinção
visual.

## Critérios de aceite

- [x] A tabela inclui espaçamento vertical de 1 linha em branco (row gap) entre cada
      download, separando visualmente as barras de progresso.
- [x] As barras de progresso de linhas consecutivas possuem separação visual clara,
      com altura delimitada e independente por item.
- [x] O cursor de seleção (`> Highlight`) envolve apenas a linha do torrent, sem
      invadir ou sobrepor a linha de separação da entrada adjacente.
- [x] O cabeçalho da tabela mantém o separador horizontal atual, preservando o
      alinhamento com as colunas (`ID`, `PROGRESSO`, `STATUS`, `NOME`, `AÇÕES`).

## Cenários de teste

### Separação visual de barras em torrents sequenciais concluídos

- **Dado** que existem múltiplos torrents com 100% de progresso listados
  sequencialmente (IDs 0 a 3)
- **Quando** a tabela da sessão é renderizada na TUI
- **Então** o sistema exibe uma lacuna visual distinta (gap de 1 linha) entre o
  bloco verde de cada item, evitando a fusão gráfica das barras.

### Navegação com cursor em tabela com espaçamento vertical

- **Dado** que o usuário utiliza as setas para alternar entre as linhas da tabela
- **Quando** a seleção avança do ID 3 para o ID 4
- **Então** o destaque azul da linha selecionada delimita apenas o item ativo atual,
  preservando o espaçamento entre as barras de progresso superior e inferior.

## Implementação

- `Layout.row_stride`: linhas de tela por entrada (1 = consecutivas, 2 = com gap).
- `render_session`: cada entrada renderizada é seguida de 1 linha em branco
  (exceto a última); highlight e sobreposição da barra usam `session_row_line`.
- `session_row_line(idx)`: linha do bloco de uma entrada (`4 + idx * 2`).
- `table_row_to_index(row, first, count, stride)`: mapeia clique do mouse para o
  índice da entrada, ignorando cliques na linha de gap.
- `mouse_session`/`mouse_menu`/`mouse_include`: passam a usar `table_row_to_index`.
