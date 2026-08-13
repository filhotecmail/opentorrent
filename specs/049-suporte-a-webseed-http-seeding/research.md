# Research: US-049 — Suporte a webseed (HTTP seeding) para torrents com url-list

## Problema

Torrents que declaram `url-list` (webseed GetRight-style, BEP 19) — ex.: coleções do archive.org —
ficam em **0.00%** no OpenTorrent mesmo baixando o `.torrent` corretamente: o motor `librqbit` 8.1.1
**não suporta webseed** (nenhuma referência a `url-list`/webseed no source) e os trackers podem reportar
**0 seeds** (ex.: `bt1/bt2.archive.org` → `complete=0`). O resultado: arquivos nunca baixam.

**Causa raiz (camada domínio → design → mecânica)**: o motor só obtém dados por peers (BitTorrent). A
web expõe o mesmo conteúdo por HTTP (webseed), mas o OpenTorrent não consome essa fonte. Não há patch
viável no motor (a manutenção do `librqbit` é externa e o comportamento de webseed está fora do escopo
da crate).

## Evidência empírica (2026-08-12)

- `mint.torrent` (archive.org, 324694 bytes) — bencode raiz tem `url-list` com **3 bases GetRight**:
  `["https://archive.org/download/", "http://ia600505.us.archive.org/31/items/",
  "http://ia800505.us.archive.org/31/items/"]`; `info.name` = `linuxmint-20-20.1-20.2-20.3-iso-collection`;
  **123 arquivos**, `piece length` = 2097152, total ≈ 32 GiB. Cada arquivo real é seguido de um arquivo de
  **padding** (`.____padding_file/NN`, `attr = 'p'`, tamanho até o fim do piece).
- **Range request validado**: `GET https://archive.org/download/<name>/20.1/linuxmint-20.1-cinnamon-64bit-edge.iso`
  com `Range: bytes=0-1023` → `HTTP/2 302` → `location: https://dn721801.ca.archive.org/...` (CDN) → seguindo
  o redirect → `HTTP/2 206` com `content-range: bytes 0-1023/<len>` íntegro. `reqwest` (já dependência) segue
  redirects por padrão e envia o header `Range`.
- **Gancho sem patch (validado no source)**: `torrent_state/initializing.rs:201-208` — sem fastresume válido,
  `librqbit` roda `FileOps::initial_check` (hash SHA1 de todos os pieces no disco) antes de iniciar. Dados já
  corretos no layout final → marcados como `have`; o motor só pede a peers o que faltar.
- Trackers do mint: `udp://bt1.archive.org:6969/announce` e `udp://bt2.archive.org:6969/announce` → `complete=0`.

## Decisões de design

1. **Webseed como fonte primária** (escolha do usuário): quando o metainfo tiver `url-list` GetRight-style
   (URLs de base terminando em `/`), **baixar via HTTP com Range** cada arquivo no layout final de saída e,
   ao concluir (ou abandonar parcialmente), **entregar o `.torrent` ao motor** — o `initial_check` valida os
   pieces baixados e peers completam o restante. Sem webseed por arquivo (BEP 19 direto, sem barra final):
   comportamento atual por peers (scope boundary, spec FR-006).
2. **Layout de saída idêntico ao do motor** (requisito do `initial_check`):
   - **CLI (`run_add`)**: o probe `list_only` já devolve `output_folder` calculado pelo `librqbit`
     (`session.default + subfolder`, que para multi-file é o `info.name`). O webseed escreve exatamente aí.
   - **Daemon**: usa `default_output_folder()` (o mesmo com que o `Session` foi criado) + subfolder padrão do
     motor (`info.name` para multi-file, nada para single-file — `get_default_subfolder_for_torrent`).
3. **Mapeamento de URL (BEP 19 GetRight)**:
   - multi-file: `base_url + torrent_name + '/' + caminho_relativo` (ex.:
     `https://archive.org/download/<name>/20.1/<iso>`); `caminho_relativo` = `filename.to_pathbuf()` de
     `iter_file_details()`.
   - single-file: `base_url + info.name`.
   - `name` = `info.name` (o mesmo usado para o subfolder).
4. **Extração do `url-list`**: o `TorrentMetaV1` do `librqbit` **não** desserializa `url-list` (campo
   desconhecido é ignorado pelo serde). Extrair do bencode bruto reusando o deserializador já no gráfico:
   `librqbit-bencode` (`BencodeDeserializer`) com um struct `#[serde(rename = "url-list")]`, aceitando lista
   OU string única (BEP 19 permite ambos) via `#[serde(untagged)]`. **Zero dep nova** (crate já compilada,
   transitiva do `librqbit`).
5. **Download HTTP**:
   - 1 Range request por arquivo cobrindo o arquivo inteiro (`Range: bytes=0-<len-1>`); aceitar `206`
     (content-range confere) ou `200` (servidor ignora Range); validar `content-length` == `length` do
     torrent, senão rejeitar o arquivo (FR/edge: dados divergentes não batem o SHA1).
   - Concorrência entre arquivos limitada (semáforo, ex. 4-8); arquivos grandes sequenciais por Range único
     na v1 (archive.org serve bem; segmentação multi-Range é otimização futura, YAGNI).
   - Bases em ordem (FR-003): falha/timeout/404 em uma base → próxima; redirects seguidos pelo reqwest.
   - Padding (`attr = 'p'`): **criar** arquivo vazio/espanso (`set_len`) sem HTTP — o `initial_check` hasheia
     o padding como zeros e ele precisa existir para os pieces validarem.
   - Arquivos `size 0`: criar vazio sem HTTP.
6. **Modelo de execução (escolha do usuário)**: **task em background no daemon**:
   - `dispatch(Add)` → resolve → extrai `url-list` → se presente, **spawna task** que baixa via HTTP e, ao
     terminar, chama `session.add_torrent` com os bytes já resolvidos; responde `Ok` imediatamente
     ("adicionando via webseed").
   - Registro de tarefas webseed no daemon (compartilhado, `Arc<Mutex<Vec<_>>>`) exposto no snapshot:
     `DaemonState.webseed: Vec<WebseedSnapshot>` (id, source, nome, bytes_downloaded/total, arquivos
     done/total, estado). A TUI renderiza essas linhas e remove quando o torrent entra na sessão.
   - CLI (`run_add`): após o probe `list_only` e antes do re-add, se houver `url-list` → roda o webseed com
     barra `MultiProgress`; depois segue o fluxo normal (`existing_state` decide skip/add).
7. **Progresso (FR-007)**: CLI usa `ProgressBar` do `MultiProgress` já existente; daemon/TUI via snapshot
   webseed. Estados pendentes reusados (`PendingStatus`), sem "erro vermelho" enquanto baixa.
8. **Idempotência/cancelamento**: dados parciais ficam no disco; ao re-adicionar, `initial_check` retoma por
   piece (sem estado persistido extra — nada novo no storage).

## Riscos / trade-offs

- **URL base divergente do layout real** (ex.: webseed com outro esquema de path): o download falha 404 e o
  torrent vai para peers com o erro registrado — não bloqueia a adição (FR-009).
- **Content-Length divergente do torrent**: arquivo marcado como erro; os demais seguem (edge case).
- **`url-list` não-GetRight** (sem barra final / direta por arquivo): ignorado nesta iteração → peers
  (assumption da spec). Baixa complexidade; fluxos assim são raros.
- **Download grande bloqueia a fila?** Não: task background isolada no daemon; CLI é por-torrent (já é o
  comportamento do `run_add`).
- **Padding espanso via `set_len`**: em filesystems espansos lê como zeros (bate o SHA1 do padding do
  archive.org); em FS sem suporte (ex.: alguns FUSE) o custo é gravar zeros — correto, só mais lento.
- **Segurança**: caminhos de arquivo validados pelo próprio `librqbit` (`to_pathbuf` rejeita `..`/separadores
  suspeitos); o webseed grava com os mesmos caminhos que o motor usaria (sem path traversal extra).
