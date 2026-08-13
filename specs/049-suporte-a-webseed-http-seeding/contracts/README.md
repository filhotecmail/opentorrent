# Contracts: US-049 — Suporte a webseed (HTTP seeding) para torrents com url-list

Contratos internos do módulo `src/webseed.rs` e das integrações (daemon/CLI/TUI). Nenhum protocolo
externo novo além de HTTP Range (BEP 19).

## 1. `extract_url_list(bytes: &[u8]) -> Option<Vec<String>>`

- Entrada: bytes brutos do `.torrent` (já resolvidos por US-048).
- Extrai o campo **raiz** `url-list` via `librqbit-bencode` (`BencodeDeserializer`) com struct
  `#[serde(rename = "url-list")]` aceitando lista OU string única (`#[serde(untagged)]`).
- Retorna **bases GetRight** (terminando em `/`), na ordem original.
- Retorno `None`: `url-list` ausente OU nenhuma base GetRight → o chamador mantém o fluxo por **peers**
  (FR-006).
- Nunca falha por bencode inválido (já validado em US-048); em caso de impossibilidade de parse, `None`.

## 2. `build_webseed_files(info) -> Vec<WebseedFile>`

- Fonte: `info.iter_file_details()` (API pública do `librqbit`).
- `relative_path` = `file.filename.to_pathbuf()?.to_string_lossy()` (o próprio `librqbit` já rejeita
  path traversal/`..`).
- `is_padding` = `file.attrs().padding` (BEP 47, `attr='p'`).
- Quando `only_files` (CLI) está presente, filtra os índices fora do conjunto (o índice é a posição de
  `iter_file_details`).

## 3. `file_url(base, config, file) -> url::Url`

- multi-file: `base + config.name + '/' + relative_path`.
- single-file (sem entries em `files`): `base + config.name`.
- Componentes de path são URL-encoded por `url::Url` (join encoda espaços/acentos).
- Execução: `base.parse()?.join(&encoded_path)`.

## 4. `async fn download_webseed(...)` (core)

- Tenta as bases **em ordem** (FR-003): para cada arquivo, `reqwest.get(url)` com `Range: bytes=0-<len-1>`
  e `User-Agent: opentorrent`; redirects seguidos (default).
- Aceita `206` (valida `content-range`) ou `200` (Range ignorado); falha se `content-length` != `length`
  esperado do arquivo (dados divergentes não batem o SHA1 — edge case).
- **Graceful parcial (FR-009)**: um arquivo com 404/timeout em todas as bases → registra erro parcial,
  continua os demais; o resultado (mesmo incompleto) vai para o motor.
- **Padding/size-0**: cria o arquivo (`create` + `set_len` para padding; vazio para size-0) **sem** HTTP
  (FR-008).
- **Resumo/re-download**: arquivo já no disco com o tamanho exato é pulado (dados parciais reaproveitados);
  nunca apaga arquivos existentes.
- **Progresso**: por callback (`Fn(&WebseedProgress)`), chamado sem locks held (o chamador decide o
  destino: `MultiProgress` no CLI, registro no daemon).
- Concorrência entre arquivos limitada por semáforo (default 4, configurável).

## 5. Integração daemon — `dispatch(Add)`

- Após `resolve_source` (US-048): se `ResolvedSource::TorrentBytes(bytes)` e `extract_url_list` devolve
  bases → registra `WebseedProgress` no registro compartilhado, **spawna** a task background e responde
  `Ok { message: "adicionando via webseed" }` (não bloqueia a conexão IPC).
- A task: calcula `output_folder` (`default_output_folder()` + `get_default_subfolder_for_torrent` layout —
  `info.name` multi-file), baixa, e **ao concluir chama `session.add_torrent(bytes → TorrentFileBytes,
  None)`** — mesmo destino de um `Add` normal (dedup/infohash no motor). Remove a tarefa do registro.
- `build_state` inclui `webseed` ativo no `DaemonState` (progresso p/ TUI).
- Falha de resolução/extração → `DaemonResponse::Error` (comportamento atual).

## 6. Integração CLI — `run_add`

- No fluxo por-torrent: após o probe `list_only` (que devolve `info`, `only_files`, `output_folder`) e
  **antes** do re-add: se `extract_url_list(bytes)` → roda `download_webseed` com barra `MultiProgress`
  (formato das barras existentes), gravando no `output_folder` do probe; depois segue o fluxo normal
  (`existing_state` decide completo/parcial/missing e o re-add valida por `initial_check`).
- Sem `url-list` → comportamento inalterado (FR-006).

## 7. Integração TUI (modo daemon)

- O snapshot agora carrega `DaemonState.webseed`; a TUI renderiza tarefas webseed como linhas de progresso
  (nome + barra) abaixo dos torrents, idênticas às pendentes (`PendingStatus::Resolving`), sem "erro vermelho".
- Quando a tarefa conclui e o torrent entra na sessão, a linha webseed some e o torrent aparece na lista.
- Falha da tarefa → linha vira `Error` com a mensagem de domínio.

## Regressão canônica

```bash
# Sem daemon rodando:
cargo run -- add "https://archive.org/download/linuxmint-20-20.1-20.2-20.3-iso-collection/linuxmint-20-20.1-20.2-20.3-iso-collection_archive.torrent" -o /tmp/dl-mint --exit-on-finish
# Deve baixar via HTTP (barra cresce) e completar sem depender de seeds.
# Fluxo preservado (peers):
cargo run -- add "https://isos.artixlinux.org/artix-base-s6-20260810-x86_64.iso.torrent" -o /tmp/dl-artix
```