# OpenTorrent

<!-- BADGES_START -->
[![CI](https://github.com/filhotecmail/opentorrent/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/filhotecmail/opentorrent/actions/workflows/ci.yml)
[![Cobertura](https://codecov.io/gh/filhotecmail/opentorrent/branch/master/graph/badge.svg)](https://codecov.io/gh/filhotecmail/opentorrent)
[![Release](https://img.shields.io/badge/release-—-blue)](https://github.com/filhotecmail/opentorrent/releases)
[![Issues abertas](https://img.shields.io/github/issues/filhotecmail/opentorrent)](https://github.com/filhotecmail/opentorrent/issues)
<!-- BADGES_END -->

Um cliente BitTorrent de **linha de comando (CLI)** para **Ubuntu/Linux** que
baixa **torrents** e **magnet links** diretamente do terminal, sem interface
gráfica.

## O que é este projeto?

O **OpenTorrent** é um programa em Rust que:

- Baixa arquivos a partir de **magnet links** (`magnet:?xt=urn:btih:...`);
- Baixa arquivos a partir de arquivos `.torrent` locais;
- Baixa arquivos a partir de URLs `http(s)` que apontam para `.torrent`;
- Lista o conteúdo de um torrent sem baixá-lo;
- Filtra quais arquivos baixar por expressão regular;
- Mostra progresso, velocidade de download/upload e ETA em tempo real;
- Interage por **mouse** com as barras de progresso (pausar, retomar, parar e
  excluir torrents com um clique);
- Usa **diretório padrão** `~/downloads/torrent-downloads/` quando `-o` não é
  informado;
- Faz **retomada inteligente**: torrents já 100% baixados são reportados e
  pulados; downloads parciais são retomados do ponto onde pararam.

O motor de BitTorrent é o **librqbit** — uma implementação 100% em Rust — o que
significa que o projeto compila e roda apenas com o ecossistema Rust, sem
dependências externas de runtime.

## Estado do projeto

> Esta seção é **atualizada automaticamente** pelo workflow `README vivo`
> (`.github/workflows/readme-live.yml`) a cada push, CI e semanalmente.

<!-- ESTADO_START -->
| Estado | Valor |
| --- | --- |
| Branch principal | `master` |
| Última release | `—` |
| Milestone atual | v1.0 |
| Issues abertas | 0 |
| Labels do projeto | 18 |
<!-- ESTADO_END -->

<!-- CARGO_START -->
| Metadado | Valor |
| --- | --- |
| Pacote | `opentorrent` |
| Versão | `0.1.0` |
| Edição Rust | 2021 |
<!-- CARGO_END -->

## Estrutura do projeto

```text
opentorrent/
├── Cargo.toml      # Manifesto do projeto: nome, versão, dependências e perfis
├── Cargo.lock      # Versões exatas das dependências travadas (binário → commitado)
├── src/
│   └── main.rs     # CLI (clap) + sessão de download (librqbit) + UI (indicatif/crossterm)
├── .gitignore      # Arquivos/pastas que não entram no repositório
└── README.md       # Este arquivo
```

> **Nota:** arquivos de governança/automação local como `AGENTS.md` e `.specify/`
> não são versionados (ver [Diretrizes de desenvolvimento](#diretrizes-de-desenvolvimento)).

## Dependências

### Dependências de runtime (crates Rust)

Todas as dependências são resolvidas automaticamente pelo Cargo no build.

| Crate | Versão | Finalidade |
| --- | --- | --- |
| `librqbit` | 8.1.1 | Motor BitTorrent: conexão a peers, trackers e DHT, download e upload de dados |
| `tokio` | 1.x | Runtime assíncrono (async/await) para rede e I/O concorrente |
| `clap` | 4.x | Parser da interface de linha de comando (subcomandos e opções) |
| `anyhow` | 1.x | Tratamento contextual de erros |
| `futures` | 0.3 | Utilitários de futuro assíncrono (ex: `join_all` para múltiplos downloads) |
| `indicatif` | 0.18 | Barras de progresso e MultiProgress na UI |
| `crossterm` | 0.28 | Captura de eventos de mouse e controle do terminal |
| `size_format` | 1.x | Formatação de tamanhos de bytes (ex: 1.24 GiB) |
| `tracing` | 0.1 | Log estruturado (macros `info!`, `error!`, `warn!`) |
| `tracing-subscriber` | 0.3 | Configuração dos logs no console (filtro por nível/`RUST_LOG`) |

> **Nota:** o `librqbit` é usado com `default-features = false` e o recurso
> `rust-tls`, evitando a necessidade de OpenSSL no runtime (TLS 100% Rust).

### Dependências de sistema (Ubuntu)

Necessárias apenas para **compilar** (gerar o binário):

| Pacote | Finalidade |
| --- | --- |
| `build-essential` | Compilador C (`cc`) e ferramentas de linkagem usadas nos build scripts |
| `pkg-config` | Localização de bibliotecas nativas durante o build |
| `libssl-dev` | Headers de OpenSSL (necessários por algumas dependências transitivas) |

## Instalação (Ubuntu)

### Opção 1 — Instalar a partir do Release (recomendado)

Cada release do GitHub publica o binário compilado para **Linux x86_64**. Para
instalar, basta baixar o binário e colocá-lo no `PATH`:

```bash
# Baixa o binário da última release (troque v0.1.5 pela versão desejada)
curl -L -o opentorrent https://github.com/filhotecmail/opentorrent/releases/download/v0.1.5/opentorrent-v0.1.5-linux-x86_64

# Torna o binário executável
chmod +x opentorrent

# Instala no PATH do sistema (todos os usuários)
sudo mv opentorrent /usr/local/bin/

# Verifica a instalação
opentorrent --version
```

> **Alternativa sem `sudo`:** mova o binário para `~/.local/bin` (garanta que
> esse diretório esteja no seu `PATH`):
>
> ```bash
> mkdir -p ~/.local/bin
> mv opentorrent ~/.local/bin/
> export PATH="$HOME/.local/bin:$PATH"
> ```

**Para atualizar:** basta repetir o download com a versão mais recente —
consulte a página de [Releases](https://github.com/filhotecmail/opentorrent/releases)
para ver as versões disponíveis.

### Opção 2 — Compilar a partir do código-fonte

```bash
# 1. Dependências de sistema (apenas para compilar)
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev

# 2. Toolchain Rust (se ainda não tiver)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# 3. Build do projeto
cd opentorrent
cargo build --release
```

O binário estará em `target/release/opentorrent`.

## Uso

Baixar um torrent a partir de um magnet link:

```bash
opentorrent add "magnet:?xt=urn:btih:..."
```

Baixar a partir de um arquivo `.torrent` local ou de uma URL:

```bash
opentorrent add ./meu-arquivo.torrent
opentorrent add https://exemplo.com/arquivo.torrent
```

### Opções do subcomando `add`

```text
Uso: opentorrent add [OPÇÕES] <TORRENT>...

Argumentos:
  <TORRENT>...  O magnet link, arquivo .torrent local ou URL http(s) para um .torrent

Opções:
  -o, --output-folder <PASTA>   Pasta de destino. Padrão: ~/downloads/torrent-downloads/
  -s, --sub-folder <PASTA>      Subpasta dentro da pasta de destino
  -r, --filename-re <REGEX>     Baixar apenas arquivos cujo nome combine com o regex
  -l, --list                    Apenas listar o conteúdo, sem baixar
      --overwrite               Forçar sobrescrita/re-download de arquivos existentes
  -e, --exit-on-finish          Encerrar o programa ao terminar os downloads
      --initial-peers <PEERS>   Lista de peers iniciais separados por vírgula (host:porta)
  -h, --help                    Mostra a ajuda
```

### Exemplos

```bash
# Listar o conteúdo de um magnet link sem baixar
opentorrent add "magnet:?xt=urn:btih:..." --list

# Baixar apenas arquivos .mp4 e encerrar ao concluir (diretório padrão)
opentorrent add ./arquivo.torrent -r '\.mp4$' -e

# Baixar para uma pasta específica
opentorrent add "magnet:?xt=urn:btih:..." --output-folder ~/Downloads

# Reexecutar o mesmo comando depois de interrompido: retoma onde parou
opentorrent add ./arquivo.torrent -o videos
# - arquivos completos → "torrent already fully downloaded", nada a fazer
# - arquivos parciais   → "partial files found, resuming download"
```

### Interação por mouse

Quando executado em um terminal interativo (TTY), cada barra de progresso exibe
botões clicáveis à direita:

```text
[0] debian.iso [======>-----------] [Pausar ] [Parar  ] [Excluir]
```

| Botão | Ação |
| --- | --- |
| `[Pausar ]` / `[Retomar]` | Alterna entre pausar e retomar o torrent |
| `[Parar  ]` | Para o torrent, mantendo os arquivos em disco |
| `[Excluir]` | Para o torrent e exclui os arquivos baixados |

## Desenvolvimento

### Ferramentas de verificação (obrigatórias)

Antes de submeter qualquer alteração, rode:

```bash
cargo fmt                      # formatação
cargo clippy -- -D warnings    # lint sem nenhum warning
cargo test                     # testes
```

### Loop de desenvolvimento rápido

O projeto usa perfis de compilação otimizados para velocidade de dev (ver
[Perfis de compilação](#perfis-de-compilação)). No loop de edição, prefira
`cargo check` em vez de `cargo build` — faz type/borrow check 2-3x mais rápido:

```bash
cargo check                    # verificação rápida (sem gerar binário)
cargo run -- add <torrent>     # executar em modo debug
cargo watch -c                 # (opcional) re-executa a cada mudança de arquivo
```

> **Dica:** se `cargo` não estiver no `PATH` do seu shell, adicione
> `export PATH="$HOME/.cargo/bin:$PATH"`.

### Perfis de compilação

Configurados em `Cargo.toml` para equilibrar velocidade de dev e binários de
produção otimizados:

```toml
[profile.dev]
debug = 0                      # sem debuginfo: builds mais rápidos
strip = "debuginfo"
incremental = true

[profile.dev.build-override]
opt-level = 3                  # acelera proc-macros e build scripts

[profile.dev.package."*"]
opt-level = 3                  # dependências compiladas com O3 uma vez, cacheadas

[profile.release]
opt-level = 3
lto = "fat"                    # otimização em todo o programa
codegen-units = 1              # máximo de otimização
panic = "abort"                # elimina overhead de unwinding
strip = true                   # binário menor
```

> **Referência:** <https://corrode.dev/blog/tips-for-faster-rust-compile-times>

### Manutenção de dependências

```bash
cargo update                   # atualiza versões semver-compatíveis
cargo audit                    # verifica vulnerabilidades
cargo machete                  # detecta dependências não usadas
cargo tree --duplicate         # consolida duplicações de versão
```

### Integração contínua (CI)

O repositório usa **GitHub Actions** em `.github/workflows/`, executado a cada
`push` para `master` e em cada **Pull Request**:

| Workflow | Arquivo | Verificações |
| --- | --- | --- |
| **CI** | `ci.yml` | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo machete` |
| **Cobertura** | `coverage.yml` | `cargo tarpaulin` → relatório XML (artefato + Codecov) |
| **Qualidade de issues** | `issue-quality.yml` | comenta e sinaliza issues sem label/milestone |
| **Notificações** | `notify.yml` | emails para `filhotecmail@gmail.com` (commits, issues, PRs, discussões, CI, releases) |
| **README vivo** | `readme-live.yml` | regenera badges e estado do projeto no README |
| **Dependabot** | `dependabot.yml` | atualizações semanais de deps Cargo e Actions |

Benefícios: PRs são bloqueados se qualquer verificação falhar; métricas de
cobertura ficam visíveis no badge do README e no Codecov. Para ativar o badge
de cobertura, configure o token do Codecov em `Settings → Secrets →
CODECOV_TOKEN` (o passo de CI não falha sem ele).

## Diretrizes de desenvolvimento

### Padrões e convenções

- **Idioma:** código, mensagens de usuário, documentação e commits em
  **português do Brasil**.
- **Código:** Rust com foco em clareza, sem comentários desnecessários e sem
  `.unwrap()` em caminhos de produção (use `?`/`context()` do `anyhow`).
- **Erros:** mensagens em letra minúscula, sem pontuação final
  (`err-10` das convenções Rust).
- **Async:** nunca segurar `Mutex`/`RwLock` guard atravessando um `.await`;
  escopos de lock curtos e soltos antes do `.await` (ver `stats_printer`).
- **UI:** usar `indicatif` para progresso (renderizado em **stderr**) e
  `crossterm` para eventos de mouse. Linhas impressas fora das barras usam
  `multi.suspend(|| ...)` via `print_line`.
- **Build limpo:** `cargo clippy -- -D warnings` deve passar sem warnings.

### Fluxo de trabalho com issues (US)

O projeto usa issues rotuladas como **US (User Stories)** numeradas. Para cada
US:

1. Criar branch descritiva: `feat/us-NNN-descricao-curta`;
2. Criar issue com critérios de aceite e testes;
3. Implementar seguindo os padrões acima e validar com as ferramentas de
   verificação;
4. Commit com mensagem descritiva e `Closes #N`;
5. Push e abrir **Pull Request** com base em `master`;
6. Merge com squash, mantendo `master` sempre estável.

### API do librqbit (referência rápida)

- `Session::new_with_opts(output_folder, options)` — cria a sessão;
- `session.add_torrent(AddTorrent, opts)` — adiciona torrent; retorna
  `AddTorrentResponse::{Added, AlreadyManaged, ListOnly}`;
- `AddTorrent::from_cli_argument(path)` — aceita magnet, `.torrent` local ou URL;
- `AddTorrentOptions` — campos chave: `overwrite`, `list_only`, `only_files_regex`,
  `sub_folder`, `initial_peers`;
- `ListOnlyResponse` — retorna `info` (metainfo), `only_files`, `output_folder`
  final e `torrent_bytes` (permite re-adicionar sem re-resolver magnet);
- `session.with_torrents(|it| ...)` — itera os torrents gerenciados (closure `Fn`);
- `ManagedTorrent::{id, name, stats, info_hash}` — metadados e estatísticas;
- `session.pause(&handle)`, `session.clone().unpause(&handle)`,
  `session.delete(TorrentIdOrHash::Id(id), delete_files)` — controle do torrent.

### Funcionalidades recentes

- **US-006** — Interação por mouse na barra de progresso (`crossterm`).
- **US-008** — Diretório padrão `~/downloads/torrent-downloads/` e retomada
  inteligente (completo → reporta; parcial → retoma).
- **US-009/US-010** — Interface interativa com sessão persistente (menu, fila de
  downloads e listagem de completos).
- **US-011** — Entrada de origem com cursor, colar (bracketed paste) e quebra de
  linha automática.
- **US-012** — Cliques do mouse nos botões de ação das linhas da sessão
  (pausar/retomar, parar e excluir) com mapeamento dinâmico de coordenadas.
- **US-013** — Destaque de seleção sincronizado com o cursor nos menus (teclas,
  atalhos e mouse).
- **US-014** — Adição assíncrona de torrents: transição imediata para a fila com
  resolução em background e status de erro sem bloquear a interface.
- **US-015** — Telas delimitadas por quadro de bordas duplas com área limitada
  na equivalência de 1024x768 (128×48 células) e quebra de linha interna.

## Licença

MIT
