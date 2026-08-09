# Contracts: US-034 — /add com prompt "insira o link:" na Home

Contratos internos do módulo `session_ui` (funções puras/helpers — sem interface externa nova).

## `parse_add_command(input: &str) -> Option<String>`

- **Entrada**: texto trimmado digitado no prompt da Home (ex.: `/add magnet:...`, `/add`, `/add `, `/list`).
- **Saída**:
  - `Some(origem)` quando `input` inicia com `/add ` (barra + add + espaço) e há conteúdo não-vazio após o prefixo (trimmed).
  - `None` caso contrário (`/add` puro, `/add ` vazio, outros comandos, texto sem `/`).
- **Contrato de teste**:
  - `/add magnet:?xt=urn:btih:abc` → `Some("magnet:?xt=urn:btih:abc")`
  - `/add` → `None`
  - `/add ` → `None`
  - `/list` → `None`
  - `olá` → `None`

## `submit_add_from_prompt(&mut self, source: &str)`

- **Pré**: chamado com `source` já `trim()`.
- **Pós (válida)**: `input`/`prompt_cursor` limpos; `prompt_add_mode = false`; `PendingTorrent` registrado; spawn de `add_torrent_background`; `view = Home`; `row_index` = última entrada.
- **Pós (vazia)**: notice "informe uma origem..."; modo inalterado.
- **Pós (inválida)**: notice de erro; texto preservado; modo inalterado.

## `Tui::prompt_add_mode` (estado)

- Ver data-model.md para transições e invariante (`view == Home`).
