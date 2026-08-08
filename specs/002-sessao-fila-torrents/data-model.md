# Data Model: US-009 + US-010

**Date**: 2026-08-08

## Entidades de domínio

### Torrent de sessão (observado da sessão librqbit)

Exposto ao TUI via `session.with_torrents`; cada item é um
`ManagedTorrentHandle`. Campos relevantes:

| Campo | Fonte | Tipo | Descrição |
|-------|-------|------|-----------|
| `id` | `handle.id()` | `TorrentId` (usize) | Identificador na fila |
| `name` | `handle.name()` | `Option<String>` | Nome do torrent |
| `info_hash` | `handle.info_hash()` | `Id20` | Hash BTv1 |
| `state` | `handle.stats().state` | `TorrentStatsState` | `Initializing`/`Live`/`Paused`/`Error` |
| `is_paused` | `handle.is_paused()` | `bool` | Se pausado |
| `percent` | `handle.stats()` | `f64` | `progress_bytes/total_bytes*100` |
| `finished` | `handle.stats().finished` | `bool` | Se 100% concluído |
| `error` | `handle.stats().error` | `Option<String>` | Mensagem de erro, se houver |

> A persistência desses campos é **gerenciada pelo librqbit**
> (`JsonSessionPersistenceStore`), não pela nossa camada. A nossa camada apenas
> lê o estado para renderizar e aciona pause/unpause/delete.

### Download completo (listagem US-010)

Item da varredura do diretório padrão `~/downloads/torrent-downloads/`:

| Campo | Tipo | Descrição |
|-------|------|-----------|
| `path` | `PathBuf` | Caminho da pasta do arquivo |
| `name` | `String` | Nome do arquivo |
| `size` | `u64` | Tamanho em disco (bytes) |
| `created` | `SystemTime` | Data de criação (metadata modified/created) |

### Comandos do menu (US-010)

Enum interno que representa as quatro opções do menu `/`:

| Variante | Atalho | Ação |
|----------|--------|------|
| `Adicionar` | A | Abre menu de inclusão da US-009 |
| `Acompanhar` | C | Abre listagem de sessão da US-009 |
| `ListarCompletos` | L | Varre `~/downloads/torrent-downloads/` e lista |
| `Sair` | S | Encerra a aplicação |

## Estados e transições (sessão)

- `Initializing → Live` (metainfo resolvida, download iniciado)
- `Live → Paused` (tecla P / pausa)
- `Paused → Live` (tecla R / retomada)
- `Live|Paused → removido da fila` (tecla X / `session.delete(id, false)`)
- `Error` (falha no torrent; exibido com mensagem)

## Persistência

| Aspecto | Decisão |
|---------|---------|
| Mecanismo | `SessionPersistenceConfig::Json { folder }` nativo do librqbit |
| Local | `~/.config/opentorrent/session` |
| Conteúdo | por torrent: info_hash, torrent_bytes, trackers, output_folder, only_files, is_paused |
| Quando | a cada mudança de estado (add/pause/unpause/delete) — gerenciado pelo store |
| Restauração | automática no `Session::new_with_opts` |

## Formato da listagem de downloads completos

Linha por item:

```
[#n] <nome do arquivo>
     Pasta: <path>
     Tamanho: <size formatado> | Criado em: <data>
```
