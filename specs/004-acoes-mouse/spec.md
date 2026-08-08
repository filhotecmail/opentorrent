# Feature Specification: Correção do mapeamento e detecção de cliques do mouse nas áreas de ação

**Feature Branch**: `feat/us-012-acoes-mouse`

**Status**: Implementado

**Input**: User story US-012 — Clicar com o mouse em qualquer parte da região dos botões de ação na linha de progresso para pausar, retomar, parar ou excluir.

## Contexto

A interface interativa (US-009/US-010) listava as linhas da sessão sem botões de ação clicáveis e o leitor de eventos ignorava eventos de mouse. A US-012 adiciona:

- Captura de eventos de mouse na interface interativa (`EnableMouseCapture` + encaminhamento de `Event::Mouse` pelo canal de eventos).
- Botões de ação renderizados em cada linha da sessão: `[Pausar ]/[Retomar] [Parar  ] [Excluir]`.
- Mapeamento dinâmico de coordenadas x/y: a geometria do último render é armazenada e reutilizada pelo manipulador de cliques, sendo recalculada a cada frame (redimensionamento de janela é refletido imediatamente).

## Critérios de aceite

1. Mapeamento preciso das coordenadas x/y dos botões em cada linha — resolvido por `Layout` (top/left/rows_offset/row_count/info_width) gravado no render e lido no clique.
2. Clique sobre o texto do botão ou dentro da área delimitada dispara a ação — mapeamento puro `action_button_at(offset)` com testes.
3. Cálculo dinâmico considerando tamanho e deslocamentos da linha — largura informativa recalculada por `cols`; coluna dos botões derivada de `left + info_width + 1`.
4. O manipulador de eventos captura todos os cliques nas áreas ativas — `key_reader` encaminha `Event::Mouse`; o loop principal despacha para `Tui::handle_mouse`.

## Cenários de teste

- Clique no botão de uma linha → ação correspondente executada (pausar/retomar alterna, parar = delete sem arquivos, excluir = delete com arquivos).
- Redimensionar a janela → novo render recalcula o layout; clique na nova posição responde corretamente.
- Clique na parte informativa da linha → apenas move a seleção.

## Notas técnicas

- crossterm 0.29: `MouseEventKind::Down(MouseButton::Left)` com `column`/`row` absolutos (1-based).
- Botões: largura 9 (`[Pausar ]`), gap 1, total 29 colunas (`ACTIONS_WIDTH`).
- Ações assíncronas (`pause`, `unpause`, `delete`) seguem a regra do repo: nenhum lock atravessa `.await`.
