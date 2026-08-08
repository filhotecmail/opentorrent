# Research: US-009 + US-010 (Interface interativa com sessão persistente)

**Date**: 2026-08-08

## Objetivo

Implementar de forma integrada:
- **US-010**: tela inicial centralizada com arte ASCII "OPENTORRENT", campo de
  entrada interativo e menu `/` (Adicionar, Acompanhar, Listar completos, Sair).
- **US-009**: persistência de sessão, restauração automática, gerenciamento de
  fila (pausar/retomar/remover) e inclusão de novos torrents sem interromper a
  fila.

## Descobertas de pesquisa (API librqbit 8.1.1)

### Persistência de sessão é nativa (mais que suficiente)

`librqbit::SessionOptions.persistence: Option<SessionPersistenceConfig>` com
`SessionPersistenceConfig::Json { folder: Option<PathBuf> }`. Ao configurar:

```rust
let opts = SessionOptions {
    persistence: Some(SessionPersistenceConfig::Json {
        folder: Some(PathBuf::from("~/.config/opentorrent/session")),
    }),
    ..Default::default()
};
let session = Session::new_with_opts(default_output_folder()?, opts).await?;
```

- `Session::new_with_opts` **restaura automaticamente** todos os torrents
  persistidos no startup (loop `persistence.stream_all()` → `into_add_torrent()`
  → `add_torrent`), preservando `output_folder`, `only_files`, `is_paused` e o
  bitfield de peças (`BitVFactory`).
- `JsonSessionPersistenceStore` grava um arquivo por torrent + fastresume.
- O diretório default (sem `folder`) usa
  `SessionPersistenceConfig::default_json_persistence_folder()`.

### Controle e leitura de estado

- `session.with_torrents(|it| ...)` — itera `(TorrentId, &ManagedTorrentHandle)`.
- `handle.stats() -> TorrentStats` com `{ state: TorrentStatsState (Initializing/
  Live/Paused/Error), progress_bytes, total_bytes, finished, live, error }`.
- `handle.is_paused() -> bool`, `handle.name() -> Option<String>`,
  `handle.info_hash() -> Id20`, `handle.only_files()`.
- `session.pause(&handle)`, `session.unpause(&Arc<Self>, &handle)`,
  `session.delete(id, delete_files)`.
- `session.add_torrent(AddTorrent::TorrentFileBytes(bytes), opts)` retorna
  `AddTorrentResponse::{Added(id, handle) | AlreadyManaged(id, handle) |
  ListOnly(...)}`.

### Resolução de metainfo (adicionar via interface)

- Para magnet links, `add_torrent` com `list_only: true` resolve via peers e
  retorna `ListOnlyResponse { info, only_files, output_folder, torrent_bytes }`.
  Re-adicionar com `AddTorrent::TorrentFileBytes(torrent_bytes)` evita
  re-resolução.
- `AddTorrent::from_cli_argument(path)` aceita magnet / `.torrent` / URL http(s).

## Decisões de design derivadas

1. **Não criar persistência própria**: usar a persistência nativa JSON do
   librqbit (`SessionPersistenceConfig::Json { folder }`) com folder customizado
   em `~/.config/opentorrent/session`. Satisfaz FR-001/FR-001a/FR-001b da US-009
   (persistência por mudança de estado é feita internamente pelo store, que
   grava na adição, pausa e retomada) e FR-002/FR-002a (restauração automática).
2. **Modo interativo sem subcomando**: quando `opts.subcommand` é `None`
   (executado sem parâmetros), entrar em modo TUI: tela inicial (US-010) →
   menu `/` → opções delegam à interface de sessão (US-009) e à listagem de
   downloads completos (varredura do diretório padrão).
3. **Dois modos**: `add` (existente, fluxo CLI) e `interativo` (novo, sem
   parâmetros). O subcomando `Add` continua sendo o caminho clássico; sem
   subcomando → TUI interativo.
4. **Interface de sessão**: listagem de `session.with_torrents` com estado,
   percentual e ações por linha (P/R/X) + atalho A/+ para adicionar.
5. **Sem `serde` adicional**: persistência é gerenciada pelo librqbit; a
   listagem de completos usa `std::fs` (sem serialização).

## Riscos / mitigação

- **Librqbit não expõe "parar"**: apenas `pause`/`unpause`. "Parado" será
  mapeado para `pause` na interface (estado exibido "pausado").
- **Remover da fila**: `session.delete(id, delete_files)` — `delete_files=false`
  para não apagar dados; na US-009 "Remover" remove da fila (delete_files=false).
- **Múltiplas adições concorrentes**: `add_torrent` é `async` e serializado pela
  sessão; a interface processa uma origem por vez (fluxo linear), evitando
  corrida no store.
- **Terminal pequeno**: renderização usa `crossterm` com `terminal::size()`;
  fallback de rolagem/centralização se altura insuficiente.
- **Testabilidade**: funções puras (varredura de completos, formato de lista)
  testáveis com `cargo test`; fluxo TUI validado por teste funcional manual.

## Conclusão

A persistência nativa do librqbit elimina a necessidade de modelar armazenamento
próprio; o trabalho concentra-se na camada de interface (TUI com crossterm),
na orquestração do modo interativo e nas duas rotas de navegação do menu.
