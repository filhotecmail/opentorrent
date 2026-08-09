# Data Model: US-035 — /delete com confirmação no prompt da Home

Sem persistência nova. Única entidade de estado (reutilizada):

## `Tui.confirming_delete: Option<DeleteTarget>` (existente)

- **Campo**: `Option<DeleteTarget>` — `None` no construtor; preenchido por `request_delete`.
- **Novo papel**: além de guardar o alvo, é a **flag de modo** da pergunta no prompt da Home (`Some` = prompt exibe `excluir '<nome>'? [Y] ou [N]`).
- **Transições**:
  - `None → Some(target)`: via `request_delete` (call-sites: `/delete`, tecla `Delete` na Session, botão Excluir do mouse), sempre com `view = View::Home`.
  - `Some → None`: `confirm_delete()` (efetiva a exclusão) ou cancelamento (`N`/`n`/`Esc`) — ambos limpam o alvo.
- **Invariantes**:
  - `confirming_delete.is_some()` ⇒ `view == View::Home` (a pergunta só é renderizada no prompt da Home).
  - Com a confirmação aberta, apenas `Y`/`y`/`S`/`s` (confirma), `N`/`n`/`Esc` (cancela) e Ctrl+C (sai) têm efeito na Home; demais teclas são ignoradas.

## `home_prompt_label(prompt_add_mode: bool) -> &'static str` (existente)

- Inalterado: `> ` normal / `insira o link: ` no modo add.

## `confirm_delete_prompt_label(name: &str) -> String` (novo, puro)

- **Entrada**: nome do alvo (`DeleteTarget::name()`).
- **Saída**: `excluir '<name>'? [Y] ou [N] ` — texto exibido como rótulo da pergunta no prompt da Home.

## `footer_hints(view, prompt_add_mode, confirming_delete: bool) -> &'static str` (novo parâmetro)

- `View::Home` com `confirming_delete` → `"Y exclui · N/Esc cancela"`.
- Demais combinações inalteradas.
