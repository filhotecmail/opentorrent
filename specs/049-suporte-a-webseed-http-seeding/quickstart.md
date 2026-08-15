# Quickstart: US-049 — Suporte a webseed (HTTP seeding) para torrents com url-list

## Uso

### Modo CLI (non-interativo)

```bash
# Torrent do archive.org com url-list e 0 seeds no tracker (webseed):
opentorrent add "https://archive.org/download/linuxmint-20-20.1-20.2-20.3-iso-collection/linuxmint-20-20.1-20.2-20.3-iso-collection_archive.torrent" -o /tmp/dl-mint

# Com filtro de arquivos (apenas o que o regex casa):
opentorrent add "<url do .torrent>" -r "*.iso" -o /tmp/dl

# Fluxos sem url-list continuam por peers (comportamento atual):
opentorrent add "https://isos.artixlinux.org/artix-base-s6-20260810-x86_64.iso.torrent" -o /tmp/dl-artix
opentorrent add "magnet:?xt=urn:btih:..."
```

- Durante o download via HTTP, aparece **barra de progresso** (bytes por arquivo e total) — não fica
  "vermelho/0%".
- Ao concluir o download HTTP, o torrent é entregue ao motor (`initial_check` valida as peças; peers
  completam o que faltar).
- Flags seguem funcionando: `-o`, `-s`, `-r`, `-l`, `--overwrite`, `-e`.

### Modo TUI / daemon

- `/add <url do .torrent>` — com o daemon em background, o download webseed roda em task separada; a lista
  mostra o progresso da tarefa. Quando termina, o torrent aparece na Biblioteca.
- Parar o daemon durante o webseed: dados parciais ficam no disco; re-adicionar retoma por piece
  (idempotente).

## Build / verificação

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test && cargo machete
```

Regressão manual do caso real (0 seeds, só webseed):

```bash
# Parar o daemon primeiro (porta DHT em uso pelo daemon impede sessão local):
opentorrent daemon stop

cargo run -- add "https://archive.org/download/linuxmint-20-20.1-20.2-20.3-iso-collection/linuxmint-20-20.1-20.2-20.3-iso-collection_archive.torrent" -o /tmp/dl-mint --exit-on-finish
# Deve mostrar barra de progresso HTTP crescendo e, no fim, download completo (sem 0.00%).
```