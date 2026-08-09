# Research: US-035 — /delete com confirmação no prompt da Home

## Desconhecidos do Technical Context

Não há NEEDS CLARIFICATION restantes (resolvidos com o usuário na especificação). Descobertas de código:

### Estado e fluxo atuais da exclusão (US-027)

- `confirming_delete: Option<DeleteTarget>` (session_ui.rs:411) é o estado da confirmação; `request_delete` (session_ui.rs:1218) monta `DeleteTarget::Torrent`/`Pending`; `confirm_delete` (session_ui.rs:1241) efetiva a exclusão com notice. **Decisão**: estado e lógica ficam intactos — só muda a apresentação.
- `render_confirm_delete` (session_ui.rs:2069) desenha um modal central com overlay (`THEME.overlay_bg`) sobre o Body e `confirm_delete_modal_lines` (session_ui.rs:2664) gera as linhas. **Decisão**: ambos são removidos — a confirmação passa para o prompt da Home.
- `render()` (session_ui.rs:1401-1403) desenha o modal quando `view ∈ {Session, Home}` e `confirming_delete.is_some()`. **Decisão**: bloco removido.

### Call-sites de exclusão

- `execute_command("/delete")` (session_ui.rs:743): `request_delete(self.row_index)`. **Decisão**: adicionar `view = View::Home`. Isso também corrige o caso `/delete` selecionado no popup de comandos, que deixava `view = View::Menu` (o modal atual nem aparecia nesse caso).
- `handle_session_key` `Delete` (session_ui.rs:780): `request_delete(self.row_index)`. **Decisão**: adicionar `view = View::Home`; remover o bloco `confirming_delete.is_some()` do Session (linhas 759-772) — inacessível, pois a confirmação só vive na Home.
- `mouse_session` botão Excluir (session_ui.rs:1046-1049): `request_delete(idx)`. **Decisão**: adicionar `view = View::Home`.
- `handle_mouse` (session_ui.rs:926-928): ignora cliques durante a confirmação. **Decisão**: mantido — o mouse não deve interromper a pergunta.

### Prompt da Home (US-031/US-034)

- `render_home_prompt` (session_ui.rs:1670): usa `home_prompt_label(self.prompt_add_mode)` + placeholder ou `self.input`. **Decisão**: tratar `confirming_delete` antes — rótulo `excluir '<nome>'? [Y] ou [N]`, sem input e sem cursor.
- `home_prompt_label` (session_ui.rs:2421): `&'static str` (`> ` / `insira o link: `). **Decisão**: mantém-se; a pergunta com nome é dinâmica e entra em `render_home_prompt` via nova função pura.
- `footer_hints(view, prompt_add_mode)` (session_ui.rs:2686): **Decisão**: ganha 3º parâmetro `confirming_delete: bool`; Home com confirmação → "Y exclui · N/Esc cancela".

### Teclas da confirmação (Home)

- `handle_home_key` (session_ui.rs:527-540): Y/y/S/s → `confirm_delete()`, N/n/Esc → cancela. **Decisão**: manter; adicionar Ctrl+C → sai (AC US1-5) — hoje Ctrl+C é engolido pelo `_ => {}`.

### Testabilidade

- `Tui::new` exige `Arc<Session>`, não construível em teste puro. **Decisão**: lógica de rótulo da pergunta em função pura `confirm_delete_prompt_label(name) -> String`; `footer_hints` testável diretamente.
- Testes atuais do modal (`confirm_delete_modal_*`, session_ui.rs:3418-3443) são removidos junto com a função.

### Alternativas consideradas

- **Manter o modal na Session e só o comando no prompt**: rejeitado — o usuário decidiu "substituir tudo" (todos os caminhos usam o prompt da Home).
- **Renderizar a pergunta na própria Session**: rejeitado — a Session não tem prompt fixo; o layout é de lista/atalhos. Transitar para a Home é coerente com o padrão do `/add`.
- **Novo campo `confirm_delete_mode: bool`**: rejeitado — `confirming_delete` já é a flag (YAGNI, `m03`).

## Decisões consolidadas

- **Decision**: reusar `confirming_delete: Option<DeleteTarget>` como modo da pergunta no prompt da Home.
- **Rationale**: sem estado novo; `request_delete`/`confirm_delete` intactos.
- **Alternatives considered**: campo novo (descartado).

- **Decision**: todos os call-sites de `request_delete` fixam `view = View::Home`.
- **Rationale**: a pergunta só existe no prompt da Home; corrige o caso Menu (popup).
- **Alternatives considered**: pergunta na Session (layout sem prompt; descartado).

- **Decision**: remover `render_confirm_delete`, `confirm_delete_modal_lines` e seus testes; adicionar `confirm_delete_prompt_label(name) -> String`.
- **Rationale**: elimina código morto do modal (US3); rótulo puro testável.
- **Alternatives considered**: deixar as funções sem uso (ruído; descartado).

- **Decision**: `footer_hints` ganha flag `confirming_delete`.
- **Rationale**: hint contextual no modo confirmação; testes ajustados.
- **Alternatives considered**: derivar do `view` (não reflete o modo; descartado).
