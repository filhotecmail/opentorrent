# OpenTorrent

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
- Mostra progresso, velocidade de download/upload e ETA em tempo real.

O motor de BitTorrent é o **librqbit** — uma implementação 100% em Rust — o que
significa que o projeto compila e roda apenas com o ecossistema Rust, sem
dependências externas de runtime.

## Estrutura do projeto

```text
opentorrent/
├── Cargo.toml      # Manifesto do projeto: nome, versão e dependências
├── Cargo.lock      # Versões exatas das dependências travadas (binário → commitado)
├── src/
│   └── main.rs     # Ponto de entrada: CLI (clap) + sessão de download (librqbit)
├── .gitignore      # Arquivos/pastas que não entram no repositório
└── README.md       # Este arquivo
```

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
  -o, --output-folder <PASTA>   Pasta de destino. Padrão: pasta atual
  -s, --sub-folder <PASTA>      Subpasta dentro da pasta de destino
  -r, --filename-re <REGEX>     Baixar apenas arquivos cujo nome combine com o regex
  -l, --list                    Apenas listar o conteúdo, sem baixar
      --overwrite               Sobrescrever arquivos existentes
  -e, --exit-on-finish          Encerrar o programa ao terminar os downloads
      --initial-peers <PEERS>   Lista de peers iniciais separados por vírgula (host:porta)
  -h, --help                    Mostra a ajuda
```

### Exemplos

```bash
# Listar o conteúdo de um magnet link sem baixar
opentorrent add "magnet:?xt=urn:btih:..." --list

# Baixar apenas arquivos .mp4 em ./videos e encerrar ao concluir
opentorrent add ./arquivo.torrent -o videos -r '\.mp4$' -e

# Baixar para uma pasta específica
opentorrent add "magnet:?xt=urn:btih:..." --output-folder ~/Downloads
```

## Desenvolvimento

```bash
# Executar em modo debug
cargo run -- add "magnet:?xt=urn:btih:..."

# Formatação e lint
cargo fmt
cargo clippy -- -D warnings

# Testes
cargo test
```

## Licença

MIT
