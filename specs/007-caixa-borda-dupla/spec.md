# Feature Specification: Enquadramento com bordas duplas e limitação de área em resolução fixa (1024x768)

**Feature Branch**: `feat/us-015-caixa-borda-dupla`

**Status**: Implementado

**Input**: User story US-015 — A interface de entrada e navegação delimitada por um quadro com bordas duplas e resolução limitada a 1024x768 (caracteres equivalentes), controlando a quebra de linha do conteúdo.

## Contexto

As telas de entrada (Include) e navegação (Menu) eram renderizadas soltas, com links longos se espalhando pela largura do terminal. A US-015:

- Desenha um quadro com **bordas duplas** (`╔═╗║╚╝`) ao redor dessas telas, centralizado.
- Limita a área amostrada à equivalência de **1024x768**: considerando célula de terminal de ~8x16 px, isso corresponde a **128 colunas × 48 linhas** (`FRAME_WIDTH`/`FRAME_HEIGHT`); quando o terminal é menor, o quadro encolhe proporcionalmente (nunca estoura) e permanece centralizado.
- Restringe o campo de entrada e todo o texto aos limites internos do quadro: links longos sofrem **quebra de linha automática** dentro do container; nada ultrapassa as bordas.
- No modo de digitação o bloco fica ancorado no topo (posição estável do cursor); fora dele, o bloco é centralizado verticalmente no interior.

## Critérios de aceite

1. Área de exibição/entrada envolvida por quadro de bordas duplas — `draw_box` em Include e Menu.
2. Dimensão limitada e fixa (1024x768 / 128x48 células) com clamp ao terminal — `frame_rect`.
3. Campo de entrada respeita os limites internos — `max_input_width = inner_width - prompt`.
4. Texto excedente sofre quebra automática dentro do container — `wrap_text` com largura interna; truncamento como garantia extra.
5. Quadro centralizado e íntegro independentemente do conteúdo digitado/colado — geometria recalculada por `frame_rect` a cada render.

## Cenários de teste

- Acesso ao menu de adição → container com bordas duplas, opções e prompt centralizados no interior, dentro do limite 1024x768.
- Colar magnet longo → quebra de linha dentro dos limites da caixa, sem estourar nem desalinhar as bordas.

## Notas técnicas

- crossterm 0.29: desenho via `cursor::MoveTo` + `style::Print` com caracteres Unicode de linha dupla.
- `frame_rect` usa `saturating_sub` e mínimo de 2 (apenas as bordas) para nunca exceder o terminal.
- O cursor (US-011) e o mapeamento de mouse (US-012/US-013) foram re-ancorados ao interior do quadro (`content_left`/`content_top`).
