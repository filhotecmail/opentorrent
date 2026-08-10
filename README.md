# OpenTorrent

<!-- BADGES_START -->
[![MSRV](https://img.shields.io/badge/MSRV-1.85+-orange?logo=rust)](https://github.com/filhotecmail/opentorrent/blob/master/Cargo.toml)
[![Plataforma](https://img.shields.io/badge/plataforma-linux%20x86__64%20%7C%20windows%20x86__64-blue)](https://github.com/filhotecmail/opentorrent/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/filhotecmail/opentorrent/total?color=2ea44f&label=downloads)](https://github.com/filhotecmail/opentorrent/releases)
[![CI pull request](https://github.com/filhotecmail/opentorrent/actions/workflows/ci.yml/badge.svg?event=pull_request)](https://github.com/filhotecmail/opentorrent/actions/workflows/ci.yml)
[![CI schedule](https://github.com/filhotecmail/opentorrent/actions/workflows/ci.yml/badge.svg?event=schedule)](https://github.com/filhotecmail/opentorrent/actions/workflows/ci.yml)
[![CodeQL](https://github.com/filhotecmail/opentorrent/actions/workflows/codeql.yml/badge.svg?branch=master)](https://github.com/filhotecmail/opentorrent/security/code-scanning)
[![Cobertura](https://codecov.io/gh/filhotecmail/opentorrent/branch/master/graph/badge.svg)](https://codecov.io/gh/filhotecmail/opentorrent)
[![Último commit](https://img.shields.io/github/last-commit/filhotecmail/opentorrent/master)](https://github.com/filhotecmail/opentorrent/commits/master)
[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/filhotecmail/opentorrent)
[![Release](https://img.shields.io/badge/release-v0.1.31-blue)](https://github.com/filhotecmail/opentorrent/releases)
[![Issues abertas](https://img.shields.io/github/issues/filhotecmail/opentorrent)](https://github.com/filhotecmail/opentorrent/issues)
[![APT Package](https://img.shields.io/badge/Debian%2FAPT-.deb-A81D33?style=flat-square&logo=debian&logoColor=white)](https://github.com/filhotecmail/opentorrent/releases/latest)
<!-- BADGES_END -->

Um cliente BitTorrent de **linha de comando (CLI)** para **Linux/Ubuntu e
Windows** que baixa **torrents** e **magnet links** diretamente do terminal,
sem interface gráfica.

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
| Última release | `v0.1.31` |
| Milestone atual | v1.0 |
| Issues abertas | 1 |
| Labels do projeto | 19 |
<!-- ESTADO_END -->

<!-- CARGO_START -->
| Metadado | Valor |
| --- | --- |
| Pacote | `opentorrent` |
| Versão | `0.1.31` |
| Edição Rust | 2024 |
<!-- CARGO_END -->

## Estrutura do projeto

```text
opentorrent/
├── Cargo.toml              # Manifesto: nome, versão, edição (2024), MSRV e dependências
├── Cargo.lock              # Versões exatas das dependências travadas (commitado)
├── build.rs                # Gera a nota .note.opentorrent com metadados do projeto (US-021)
├── build.sh                # Helper de build: bump de versão, build verbose e assinatura
├── .devcontainer/          # Container de desenvolvimento para GitHub Codespaces (US-034)
├── src/
│   ├── main.rs             # CLI (clap), sessão de download (librqbit) e checagem de versão (US-029)
│   ├── session_ui.rs       # TUI interativa: Header/Body/Footer, tabela, modal, mouse
│   ├── downloads.rs        # Lógica de download e estado de arquivos no disco
│   └── metadata.rs         # Metadados embutidos na seção .note.opentorrent do ELF
├── scripts/
│   ├── sign-release.sh     # Assinatura digital do binário (CA local, RSA-SHA256) — US-018
│   ├── update-readme.sh    # Gerador do README vivo (badges e estado do projeto)
│   └── us-pipeline.sh      # Automação do pipeline de US (start/finish/state) — US-017
├── specs/                  # Especificações das user stories (specs/001 a specs/018)
├── .github/
│   └── workflows/          # CI, CodeQL, Release (US-030), README vivo, notificações
├── .cargo/config.toml      # Linker mold (linkagem rápida em dev)
├── .gitignore              # Arquivos/pastas que não entram no repositório
└── README.md               # Este arquivo
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
| `crossterm` | 0.29 | Captura de eventos de mouse e controle do terminal |
| `size_format` | 1.x | Formatação de tamanhos de bytes (ex: 1.24 GiB) |
| `reqwest` | 0.12 | Cliente HTTP para a checagem de versão/atualizações (US-029) |
| `semver` | 1.x | Comparação SemVer de versões (US-029) |
| `serde_json` | 1.x | Leitura do `tag_name` da API de releases (US-029) |
| `chrono` | 0.4 | Datas/horários nos registros e na sessão |
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

## Instalação

### Linux (Ubuntu)

#### Instalar via apt (repositório Gemfury)

O pipeline de release publica o pacote `.deb` no repositório apt gerenciado
pelo Gemfury a cada release do GitHub. Para instalar com o gerenciador de
pacotes:

```bash
# 1. Configura o repositório (uma única vez)
echo "deb [trusted=yes] https://apt.fury.io/cads2509/ /" | sudo tee /etc/apt/sources.list.d/opentorrent.list

# 2. Atualiza o índice de pacotes e instala
sudo apt update
sudo apt install opentorrent
```

> O repositório Gemfury não é assinado com GPG — por isso o `[trusted=yes]`.

**Para atualizar:** cada release do GitHub publica automaticamente uma nova
versão no repositório. Basta atualizar o índice e fazer o upgrade:

```bash
sudo apt update
sudo apt install --only-upgrade opentorrent
```

**Para desinstalar:**

```bash
sudo apt remove opentorrent
sudo rm /etc/apt/sources.list.d/opentorrent.list
sudo apt update
```

#### Instalar a partir do Release (recomendado)

Cada release do GitHub publica o binário compilado para **Linux x86_64**. Para
instalar, basta baixar o binário da última release (sem hardcode de versão) e
colocá-lo no `PATH`:

```bash
# Baixa o binário da última release automaticamente
curl -L -o opentorrent https://github.com/filhotecmail/opentorrent/releases/latest/download/opentorrent-linux-x86_64

# Torna o binário executável
chmod +x opentorrent

# Instala no PATH do sistema (todos os usuários)
sudo mv opentorrent /usr/local/bin/

# Verifica a instalação (informa também se há versão mais recente)
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

**Para atualizar:** basta repetir o download do comando acima — o endpoint
`/releases/latest` sempre aponta para a versão mais recente. O próprio
`opentorrent --version` já consulta o GitHub e informa se há uma release nova
disponível, com o comando de atualização sugerido (US-029).

> **Releases automáticas:** cada push na `master` que incremente a versão do
> `Cargo.toml` publica automaticamente uma nova release (binários Linux +
> Windows) pelo workflow `Release` — não é preciso criar a release manualmente.

> **Verificação da assinatura (US-018):** cada release publica o binário Linux
> com seu arquivo `.sig` (RSA-SHA256, assinado por uma CA local). O pipeline de
> release já valida a integridade antes de publicar; para conferir
> manualmente (desenvolvedores/mantenedores que possuem a chave pública da
> CA local em `~/.local/share/opentorrent/signing/`):
>
> ```bash
> # Baixa a assinatura e verifica o binário `opentorrent` já baixado
> curl -LO https://github.com/filhotecmail/opentorrent/releases/latest/download/opentorrent-linux-x86_64.sig
> openssl dgst -sha256 -verify ~/.local/share/opentorrent/signing/code-signing.pub -signature opentorrent-linux-x86_64.sig opentorrent
> ```

#### Compilar a partir do código-fonte

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

### Windows

#### Baixar e instalar (PowerShell)

Cada release do GitHub publica o executável compilado para **Windows x86_64**
(`opentorrent-windows-x86_64.exe`). No **PowerShell** (abra o menu Iniciar,
digite `powershell` e pressione Enter):

```powershell
# Baixa o executável da última release automaticamente
Invoke-WebRequest -Uri https://github.com/filhotecmail/opentorrent/releases/latest/download/opentorrent-windows-x86_64.exe -OutFile opentorrent.exe

# Verifica a instalação (informa também se há versão mais recente)
.\opentorrent.exe --version
```

Para usá-lo de qualquer diretório — com o comando `opentorrent` sem o prefixo
`.\` — coloque o `opentorrent.exe` em uma pasta do `PATH` (ou adicione a pasta
atual ao `PATH` da sessão):

```powershell
# Ex.: instala em uma pasta do PATH criada para ferramentas do usuário
New-Item -ItemType Directory -Force -Path "$HOME\bin" | Out-Null
Move-Item -Force .\opentorrent.exe "$HOME\bin\opentorrent.exe"
$env:Path += ";$HOME\bin"

# Agora o comando funciona em qualquer diretório, sem ".\":
opentorrent --version

# Em janelas futuras, adicione a linha acima ao perfil do PowerShell:
# notepad $PROFILE  →  $env:Path += ";$HOME\bin"
```

No **CMD**, o equivalente é:

```bat
:: Instala em %USERPROFILE%\bin e adiciona ao PATH da sessão
mkdir %USERPROFILE%\bin
move opentorrent.exe %USERPROFILE%\bin\
set PATH=%PATH%;%USERPROFILE%\bin
opentorrent --version
:: Para persistir em todas as sessões:
::   setx PATH "%PATH%;%USERPROFILE%\bin"
```

> **Alternativa sem downloads:** se o `cargo` estiver instalado, compile
> diretamente com `cargo install --git https://github.com/filhotecmail/opentorrent`
> ou `cargo build --release` a partir do código-fonte (Rust 1.85+).

**Para atualizar:** repita o `Invoke-WebRequest` acima — o endpoint
`/releases/latest` sempre aponta para a versão mais recente, e o
`opentorrent --version` avisa quando há release nova (US-029).

## Quickstart

Em 4 comandos:

```bash
# 1. Instala a última release (Linux x86_64)
curl -L -o opentorrent https://github.com/filhotecmail/opentorrent/releases/latest/download/opentorrent-linux-x86_64
chmod +x opentorrent
sudo mv opentorrent /usr/local/bin/

# 2. Confere a versão instalada (e se há atualização disponível)
opentorrent --version

# 3. Baixa um torrent/magnet link (diretório padrão ~/downloads/torrent-downloads/)
opentorrent add "magnet:?xt=urn:btih:..."

# 4. Acompanha a sessão interativa (menu, fila e barra de progresso com mouse)
opentorrent
```

> **No Windows (PowerShell):** troque o passo 1 por
> `Invoke-WebRequest -Uri https://github.com/filhotecmail/opentorrent/releases/latest/download/opentorrent-windows-x86_64.exe -OutFile opentorrent.exe`
> e use `.\opentorrent.exe` nos demais comandos.

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

### Interface interativa (TUI)

Ao executar `opentorrent` sem argumentos, a TUI abre com **Header** (título +
versão), **Body** (menu/área de conteúdo) e **Footer** (atalhos) — estrutura
padronizada com tema escuro e destaque de seleção com cor de fundo (US-019).
A sessão exibe uma tabela com colunas `ID`, `PROGRESSO`, `STATUS`, `NOME` e
`AÇÕES`, com barra de progresso em blocos contínuos coloridos por estado
(US-020) e separadores sutis entre os registros (US-028).

Em um terminal interativo (TTY), cada linha exibe botões clicáveis à direita:

```text
>  [0] ██████████░░░░░░░░░░ 45.2% baixando  debian.iso  [Pausar ] [Parar   ] [Excluir]
```

| Botão / Tecla | Ação |
| --- | --- |
| `[Pausar ]` / `[Retomar]` | Alterna entre pausar e retomar o torrent |
| `[Parar  ]` | Para o torrent, mantendo os arquivos em disco |
| `[Excluir]` | Para o torrent e exclui os arquivos baixados (com confirmação) |
| `Delete` | Exclui o torrent selecionado (com confirmação Y/N) e apaga os arquivos do disco (US-027) |

A barra de progresso usa blocos sólidos (`█`) para o percentual concluído e
neutros (`░`) para o restante, com cor por estado: **verde** em andamento/
concluído, **azul/amarelo** pausado e **vermelho** em erro (US-020).

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

### Scripts de build, assinatura e PRs

Automatizam o ciclo dev → release:

- **`./build.sh [release|debug] [build|verbose|clean|check|test|clippy|fmt]`** —
  compila com bump automático de versão patch em `Cargo.toml`. Em `release`, ao
  final do build, assina digitalmente o binário (US-018).
- **`./scripts/sign-release.sh [init|sign|verify]`** — assinatura digital do
  binário release: gera CA local + certificado com
  `extendedKeyUsage=codeSigning` em `~/.local/share/opentorrent/signing` (fora
  do git), assina com RSA-SHA256 (assinatura desanexada `<bin>.sig`) e valida a
  integridade contra a CA raiz.
- **`./scripts/create-new-us.sh`** — ciclo de vida de uma User Story:
  - `start US-038 "título" [descrição]` — sincroniza `master`, cria a branch
    local `feat/us-038-slug` e abre a Issue com **milestone + label**;
  - `execute-pipeline-gh [mensagem]` — valida (fmt/clippy/test/machete), faz
    commit, push e abre o **PR** para `master` com "Closes #N" (entra no CI/CD);
  - `state` — mostra a US em andamento.

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
| **CodeQL** | `codeql.yml` | análise estática de segurança (Code Scanning) para Rust (US-034) |
| **Cobertura** | `coverage.yml` | `cargo tarpaulin` → relatório XML (artefato + Codecov) |
| **Qualidade de issues** | `issue-quality.yml` | comenta e sinaliza issues sem label/milestone |
| **Notificações** | `notify.yml` | emails para `filhotecmail@gmail.com` (commits, issues, PRs, discussões, CI, releases) |
| **README vivo** | `readme-live.yml` | regenera badges e estado do projeto no README |
| **Release** | `release.yml` | publica release automática (Linux + Windows) no push de tag `v*` ou push na master com bump de versão (US-030) |
| **Dependabot** | `dependabot.yml` | atualizações semanais de deps Cargo e Actions |

Benefícios: PRs são bloqueados se qualquer verificação falhar; métricas de
cobertura ficam visíveis no badge do README e no Codecov; os relatórios de
segurança aparecem em **Security → Code scanning**. Para ativar o badge
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

O projeto usa issues rotuladas como **US (User Stories)** numeradas. O ciclo é
automatizado por `./scripts/create-new-us.sh`:

1. `./scripts/create-new-us.sh start US-038 "título"` — cria a branch local a
   partir de `master` e abre a Issue com milestone e label;
2. Implementar seguindo os padrões acima e validar com as ferramentas de
   verificação;
3. `./scripts/create-new-us.sh execute-pipeline-gh "mensagem"` — valida, faz
   commit, push e abre o **Pull Request** com base em `master` (referenciando a
   issue com "Closes #N");
4. Merge com squash, mantendo `master` sempre estável.

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
- **US-016** — Renderização estável com double buffering/diff-rendering e
  redraw sob demanda (sem cintilação).
- **US-017** — Pipeline de automação de User Stories (`scripts/us-pipeline.sh`:
  `start` → branch + issue; `finish` → commit, push e PR vinculado).
- **US-018** — Assinatura digital do binário de release (`scripts/sign-release.sh`,
  RSA-SHA256 com CA local) e verificação de integridade na esteira.
- **US-019** — Redesign arquitetural da TUI: Header/Body/Footer fixos, tabela
  estruturada e paleta de tema escuro profissional com highlight de fundo.
- **US-020** — Barra de progresso visual em blocos contínuos (`█`/`░`) com cor
  por estado e percentual centralizado.
- **US-021** — Injeção de metadados do projeto (nome, repositório e objetivo) na
  seção `.note.opentorrent` do ELF (`strings`/`readelf`).
- **US-026** — Espaçamento vertical/separadores na tabela para isolamento
  visual das barras de progresso.
- **US-027** — Ações completas (`[Pausar] [Parar] [Excluir]`) para torrents em
  inicialização e exclusão com tecla `Delete` + confirmação Y/N e remoção dos
  arquivos do disco.
- **US-028** — Divisores sutis em linha fina (`─`) entre os registros da tabela
  com densidade compacta.
- **US-029** — `--version`/`-V` consulta o GitHub (timeout 3s) e avisa se há
  versão mais recente, sugerindo o comando de atualização.
- **US-030** — Pipeline de CI/CD de release no GitHub Actions: publica o binário
  assinado automaticamente no push de tag `v*` ou push na master com bump de
  versão (instalação via `/releases/latest`).
- **US-034** — Badges de saúde no README (MSRV, deps.rs, downloads, plataformas,
  último commit, CodeQL/Code Scanning, Codespaces e Discussions), análise
  estática de segurança via CodeQL e publicação de release também para Windows
  (instalação via PowerShell).
- **US-038** — Suporte cross-platform na resolução do diretório Home: uso de
  `dirs::home_dir()`/`dirs::config_dir()` com fallback automático para
  `%USERPROFILE%`/`%APPDATA%` no Windows (o `opentorrent.exe` inicia sem depender
  da variável `$HOME`) e orientação de `PATH` para executar `opentorrent` sem o
  prefixo `.\`.

## Licença

MIT
