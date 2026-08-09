# Data Model: US-034 — /add com prompt "insira o link:" na Home

Sem persistência nova. Única entidade de estado:

## `Tui` (campo novo)

- **Campo**: `prompt_add_mode: bool` — `false` no construtor.
- **Transições**:
  - `false → true`: `/add`/`/add ` sem origem no Enter da Home (ou via `execute_command("/add")`).
  - `true → false`: submissão bem-sucedida ou com origem vazia/erro **preservando** o modo; `Esc` cancela e volta ao modo normal.
- **Invariante**: com `prompt_add_mode == true`, `view == View::Home`.

## Invariantes de origem

- Origem vazia (`trim().is_empty()`) → notice "informe uma origem (magnet, .torrent ou URL)", modo permanece ativo.
- Origem inválida (`AddTorrent::from_cli_argument` Err) → notice de erro, texto preservado, modo permanece ativo.
- Origem válida → `PendingTorrent { status: Resolving }` registrado; spawn `add_torrent_background`; `row_index` = última entrada; modo desativado; view Home.
