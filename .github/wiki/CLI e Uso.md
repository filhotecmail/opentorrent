# CLI e Uso

```text
opentorrent <comando> [opções]
```

## Comandos

| Comando | Descrição |
| --- | --- |
| `add <torrent>` | Baixa um torrent. Aceita arquivo `.torrent`, URL `http(s)` ou **magnet link**. |

## Opções principais

| Opção | Descrição |
| --- | --- |
| `-o <dir>` | Diretório de saída (padrão: `~/downloads/torrent-downloads/`). |
| `-s <sub>` | Subpasta dentro do diretório de saída. |
| `-r <regex>` | Filtra quais arquivos baixar por expressão regular. |
| `--list` | Lista o conteúdo do torrent sem baixar. |
| `--overwrite` | Força re-download mesmo se o torrent já estiver completo. |

## Exemplos

```bash
# Baixar por magnet link
opentorrent add "magnet:?xt=urn:btih:..."

# Baixar arquivo .torrent local
opentorrent add movie.torrent

# Baixar a partir de URL
opentorrent add https://example.com/movie.torrent

# Listar conteúdo sem baixar
opentorrent add movie.torrent --list

# Baixar apenas episódios que casam com regex
opentorrent add serie.torrent -r "S01E0[1-3]"

# Salvar em pasta específica
opentorrent add movie.torrent -o ~/Videos
```

## Interação

Durante o download, o progresso é mostrado em tempo real (indicatif). Com
**mouse** (crossterm) é possível pausar, retomar, parar e excluir torrents com
um clique nas barras de progresso.
