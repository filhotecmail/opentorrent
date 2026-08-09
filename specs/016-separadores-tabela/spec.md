# US-028 — Ajuste de densidade da tabela com separadores sutis em linha fina

## História

Como **Usuário do sistema**, eu quero que a tabela de downloads utilize
separadores sutis em linha fina (box-drawing) com densidade compacta entre as
linhas, para que as barras de progresso permaneçam visualmente isoladas sem
gerar espaçamentos verticais excessivos, aproveitando melhor a área útil da tela.

## Critérios de aceite

- [x] Removidas as linhas em branco (row gaps) entre os itens da tabela.
- [x] Divisor sutil em linha fina (`─` contínuo, tom escuro `THEME.muted`) entre
      cada registro, com a mesma largura da tabela (colunas alinhadas).
- [x] Altura compacta por item: cada entrada ocupa a própria linha + 1 divisor
      fino (sem linha em branco), exibindo mais torrents na viewport.
- [x] As barras de progresso respeitam os limites do divisor superior/inferior
      (linhas de dados nas posições pares do bloco; divisor nas ímpares).
- [x] O destaque `> Highlight` cobre apenas a linha ativa, preservando o divisor
      fino (as linhas do divisor nunca recebem fundo de destaque).

## Cenários de teste

### Exibição de lista de downloads com divisores sutis em linha fina

- **Dado** múltiplos torrents com barras de progresso listados em sequência
- **Quando** a tabela da sessão é desenhada na TUI
- **Então** o sistema exibe uma linha fina delimitadora entre cada registro,
  mantendo as barras separadas com densidade de texto adequada.

### Densidade de tela otimizada na navegação

- **Dado** uma janela de terminal com altura limitada e 10+ torrents ativos
- **Quando** o usuário visualiza a lista
- **Então** a tabela exibe os itens com espaçamento reduzido e linhas finas,
  garantindo maior densidade de dados na viewport.

## Implementação

- `separator_line(width)`: `─` repetido pela largura da tabela (helper puro).
- `render_session`: cada entrada é seguida de `separator_line(table_width)`
  (exceto a última), substituindo o gap em branco da US-026; `seps` guarda os
  índices das linhas divisoras.
- Sobreposição dos divisores com `put_styled(..., THEME.muted, None)` após a
  escrita base — tom cinza/escuro sem invadir a linha ativa.
- `table_width` compartilhado entre cabeçalho, separador do cabeçalho e
  divisores (alinhamento perfeito).
- Mouse/matemática de linhas inalteradas (stride 2 já cobre linha + divisor);
  cliques no divisor não alteram a seleção.
