# Quickstart: US-048 — Suporte a links de download de .torrent e drag & drop no input

## Uso

### Modo CLI (non-interativo)

```bash
# URL de página que dispara download de .torrent (fosstorrents thankyou):
opentorrent add "https://fosstorrents.com/thankyou/?name=arcolinux&cat=Latest%20Edition&id=1&hybrid=0"

# URL que já responde bytes .torrent direto (sem extensão no caminho):
opentorrent add "https://host/download?id=1234"

# Fluxos existentes preservados:
opentorrent add "magnet:?xt=urn:btih:..."
opentorrent add "~/Downloads/file.torrent"
```

Flags seguem funcionando: `-o` output, `-s` sub_folder, `-r` filename-re, `-l` list.

### Modo TUI

- `/add <link>` ou `/add` + colar a URL e Enter.
- **Drag & drop**: arraste o arquivo `.torrent` do gerenciador de arquivos para o campo de input do footer; o terminal cola o caminho absoluto; pressione Enter. O torrent entra na lista de processamento.
- Colar (Ctrl+Shift+V) um caminho de `.torrent` também funciona.

## Build / verificação

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test && cargo machete
```

Regressão manual do caso real:

```bash
cargo run -- add "https://fosstorrents.com/thankyou/?name=arcolinux&cat=Latest%20Edition&id=1&hybrid=0" --list
# deve listar os arquivos do torrent (sem "error decoding torrent")
```