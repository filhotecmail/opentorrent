# Contracts: US-035 — /delete com confirmação no prompt da Home

Contratos internos do módulo `session_ui` (funções puras/helpers — sem interface externa nova).

## `confirm_delete_prompt_label(name: &str) -> String`

- **Entrada**: nome do alvo da exclusão (`DeleteTarget::name()`).
- **Saída**: rótulo da pergunta exibido no prompt da Home — `excluir '<name>'? [Y] ou [N] `.
- **Contrato de teste**:
  - `confirm_delete_prompt_label("arquivo.iso")` → contém `excluir 'arquivo.iso'?` e `[Y] ou [N]`.
  - Nome longo não estoura (truncamento aplicado no render via `Tui::truncate`).

## `footer_hints(view: &View, prompt_add_mode: bool, confirming_delete: bool) -> &'static str`

- **Entrada**: view atual, modo add do prompt e flag de confirmação.
- **Saída**:
  - `View::Home` + `confirming_delete` → `"Y exclui · N/Esc cancela"`.
  - `View::Home` + `prompt_add_mode` → `"digite/cole a origem · Enter adiciona · Esc cancela"`.
  - `View::Home` → `"/ para comandos · Enter executa · ↑/↓ seleciona"`.
  - `View::Session` → contém `[Del]` (inalterado).

## `Tui::confirming_delete` (estado)

- Ver data-model.md para transições e invariante (`confirming_delete.is_some()` ⇒ `view == View::Home`).

## Removidos (código morto do modal US-027)

- `Tui::render_confirm_delete` — método.
- `confirm_delete_modal_lines(name, detail)` — função.
- Testes `confirm_delete_modal_asks_yes_no`, `confirm_delete_modal_truncates_long_names`, `confirm_modal_prompt_fits_within_write_boxed_limit`.
