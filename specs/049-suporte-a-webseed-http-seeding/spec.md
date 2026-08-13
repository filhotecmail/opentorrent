# Feature Specification: US-049 — Suporte a webseed (HTTP seeding) para torrents com url-list

**Feature Branch**: `feat/us-049-suporte-a-webseed-http-seeding-para-torr`

**Created**: 2026-08-12

**Status**: Draft

**Issue**: #108

**Input**: User description: "E como podemos estender, dar suporte a webseed sem precisar mexer na librqbit?" — Torrents que declaram `url-list` (webseed GetRight-style, BEP 19), como os do archive.org, não baixam no OpenTorrent: o motor `librqbit` não suporta webseed/HTTP seeding e os trackers podem reportar 0 seeds.

## Clarifications (Speckit Clarify)

1. **Estratégia**: A — webseed como fonte primária. Quando o `.torrent` tiver `url-list`, baixar os arquivos via HTTP (com Range requests) escrevendo no disco, e só então entregar o torrent ao motor — o `initial_check` do motor valida os pieces e completa via peers o que faltar. (Fallback híbrido/paralelo rejeitado: o motor não revalida pieces adicionados depois da adição.)
2. **Processo**: Spec Kit completo (US-049) com issue/branch via `create-new-us.sh`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Baixar torrent com url-list via webseed como fonte primária (Priority: P1)

O usuário adiciona (modo `add` ou `/add` na TUI) um `.torrent` que declara `url-list` (GetRight-style, ex.: coleções do archive.org). O OpenTorrent baixa os arquivos via HTTP (Range requests) diretamente no diretório de saída, mostrando progresso; ao concluir o download HTTP, entrega o torrent ao motor, que valida os pieces (initial_check) e completa via peers qualquer parte não coberta pelo webseed. O resultado: o torrent baixa sem depender de seeders do tracker.

**Why this priority**: É o requisito central — hoje torrents do archive.org ficam em 0.00% mesmo baixando o `.torrent` corretamente, pois estão sem seeds/tracker e o motor não suporta webseed.

**Independent Test**: `opentorrent add "<url do .torrent do archive.org>"` → arquivos são baixados via HTTP (progresso cresce) e no fim o torrent consta como completo, sem ficar em 0.00%.

**Acceptance Scenarios**:
1. **Given** um `.torrent` com campo `url-list` e 0 seeds no tracker, **When** o usuário o adiciona, **Then** o download inicia via HTTP e progride (não fica em 0.00%); com Range requests por arquivo.
2. **Given** um `.torrent` com `url-list` coberto parcialmente pelo webseed, **When** o download HTTP termina, **Then** o torrent é entregue ao motor, que valida os pieces completos via initial_check e obtém do peers o restante.
3. **Given** um `.torrent` sem `url-list` (ex.: artix), **When** o usuário o adiciona, **Then** o comportamento atual via peers é mantido (sem webseed intermediário).

### User Story 2 - Progresso e estados visíveis durante o download webseed (Priority: P2)

Durante o download HTTP (webseed), a UI/CLI reflete o progresso real (bytes baixados por arquivo e total), e o item na lista de transmissão não fica "vermelho/0%" como erro; estados pendentes existentes (`PendingStatus`) são reutilizados.

**Why this priority**: Visibilidade; sem progresso, o usuário não distingue "baixando via HTTP" de "travado".

**Independent Test**: Adicionar o torrent do archive.org e observar progresso crescente nos arquivos e no total.

**Acceptance Scenarios**:
1. **Given** um download webseed em andamento, **When** o usuário observa a lista, **Then** vê progresso crescente (bytes/percentual) sem erro.
2. **Given** uma falha de HTTP (ex.: 404 em um arquivo do `url-list`), **When** o download falha, **Then** o item mostra erro claro e os demais arquivos seguem; o torrent é entregue ao motor mesmo que parcial, completando via peers.

### User Story 3 - Robusteza: múltiplas bases `url-list` e redirects (Priority: P3)

O webseed deve suportar múltiplas URLs base em `url-list` (tentando em ordem), redirects HTTP (ex.: archive.org 302 → CDN `ia*.org`) e Range `206`/`content-range`. Falhas de rede são detectadas e tratadas com retry/fallback para a próxima base.

**Why this priority**: Robustez; archive.org redireciona e expõe várias bases.

**Independent Test**: Requisitar um arquivo com `https://archive.org/download/...` → segue o 302 para `ia*.org` e baixa com Range 206 (já validado manualmente).

**Acceptance Scenarios**:
1. **Given** um `url-list` com várias bases, **When** a primeira falha (timeout/404), **Then** tenta a próxima base antes de reportar erro.
2. **Given** uma URL que responde 302 para o CDN, **When** baixada, **Then** o redirect é seguido e o Range responde `206` com `content-range` íntegro.

## Technical Notes (Specify)

- **Gancho sem patch no motor**: `librqbit` faz `initial_check` de todos os SHA1 no disco na adição sem fastresume (`.torrent_state/initializing.rs`). Dados já corretos em disco (baixados via HTTP) são reconhecidos como completos; o motor só baixa via peers o que faltar.
- **Mapeamento de URL**: o `url-list` do archive.org é `https://archive.org/download/`; a URL final é `base_url + torrent_name + '/' + caminho_relativo_do_arquivo`. O `.torrent` (dicionário raiz) tem `url-list`; o campo `info` (bencode) tem `name` + `files[].path`/`length` (multi-file) ou `length` (single-file).
- **Range requests**: `reqwest` (já dependência) suporta `Range` header; arquivo archive.org responde `206 partial content` com `content-range` (validado manualmente: 302 → CDN → 206).
- **Vazão/limpeza**: baixar em paralelo com limites razoáveis; arquivos `.____padding_file/*` (pad archive.org) devem ser criados, não baixados; quando um arquivo for maior que a fatia HTTP, usar múltiplos Range para paralelizar.
- **Integração**: o fluxo webseed entra no mesmo ponto de resolução/adição já existente (US-048, `resolve.rs`/`ResolvedSource`), preferencialmente no modo daemon/headless e CLI, decidindo: tem `url-list` → webseed; senão → peers.
- **Crates**: nada novo — `reqwest`, `url`, `bytes` e bencode já presentes.
- **Cuidado com infohash**: não re-calcular infohash; os bytes do arquivo devem ser idênticos aos descritos no torrent para o `initial_check` validar por SHA1.

## Edge Cases

- `url-list` presente mas vazio / não-GetRight (URLs diretas por arquivo): tratar apenas bases GetRight (URLs terminando em `/`); senão seguir para peers.
- Arquivo com `size 0` no torrent: criar arquivo vazio sem HTTP.
- HTTP 404/410 em um arquivo: registrar erro parcial, continuar os demais, entregar o que restou ao motor.
- Redirect para outro host com tls próprio: seguir redirects (default do reqwest, limitado), validar `206`/`200`.
- `Content-Length` divergente do torrent: rejeitar/marcar erro (dados não correspondem ao SHA1).
- Cancelamento/parada do daemon durante webseed: dados parciais ficam no disco; ao re-adicionar, `initial_check` retoma (idempotência por piece).
- Torrent single-file com `url-list` e sem pasta: URL usa apenas `base + name`.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: O sistema MUST detectar o campo `url-list` (GetRight-style) no metainfo ao receber um `.torrent` (URL, arquivo ou bytes) antes de iniciar download por peers.
- **FR-002**: O sistema MUST baixar cada arquivo do webseed via HTTP com Range requests, na ordem/direção `base_url + nome_torrent + caminho_relativo`, escrevendo no layout final de saída.
- **FR-003**: O sistema MUST tentar as bases do `url-list` em ordem até um sucesso (404/timeout → próxima base).
- **FR-004**: O sistema MUST seguir redirects HTTP (ex.: 302 archive.org → CDN) e aceitar `206`/`content-range` e `200`.
- **FR-005**: Após concluir (ou abandonar parcialmente) o download HTTP, o sistema MUST entregar o torrent ao motor para o `initial_check` validar os pieces e completar via peers o faltante.
- **FR-006**: O sistema MUST manter comportamento atual (peers) quando não houver `url-list`.
- **FR-007**: O sistema MUST exibir progresso (bytes por arquivo e total) durante o download webseed, sem "erro vermelho".
- **FR-008**: O sistema MUST tratar arquivos de padding (`____padding_file`) criando-os sem download e ignorá-los na contagem de progresso.
- **FR-009**: O sistema MUST ser resistente a falha parcial (um arquivo 404): o restante continua e o resultado parcial é entregue ao motor.

### Key Entities *(include if feature involves data)*

- **WebseedConfig**: `url_list: Vec<String>` (bases), extraído do metainfo; validação GetRight (termina em `/`).
- **WebseedFile**: `relative_path: String`, `length: u64`, `webseed_base: String` (selecionada), `is_padding: bool`, estado (pendente/baixando/ok/erro).
- **TorrentBytes/Aqui**: bytes do `.torrent` já resolvidos (US-048 `ResolvedSource`), reutilizados para extrair metainfo e para `AddTorrent`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `opentorrent add "<mint archive.torrent>"` (0 seeds) inicia download via HTTP e, ao final, o torrent consta completo ou com progresso real >0% (não 0.00%) em ≤ 2 minutos de teste.
- **SC-002**: downloads via webseed não dependem de seeders do tracker (torrents com `url-list` e 0 seeds baixam).
- **SC-003**: 100% dos testes automatizados existentes continuam passando (161 testes atuais).
- **SC-004**: um `url-list` com múltiplas bases falha a primeira e usa a segunda sem erro ao usuário (teste unitário/integrado).

## Assumptions

- Adotar apenas webseed "GetRight-style" de bases (`url-list` com URLs terminando em `/`); fluxos de webseed por-arquivo (sem barra final) serão ignorados para peers nesta iteração.
- O layout de saída do webseed replica o layout do torrent (mesmo `output_folder`/`sub_folder`), para o `initial_check` encontrar os arquivos no lugar.
- Não será adicionada UI nova; o progresso reusa os displays existentes (lista/`MultiProgress`/daemon).
- O download webseed roda no mesmo processo da adição (CLI/daemon), não em serviço separado.