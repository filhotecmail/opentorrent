# Contracts: US-009 + US-010

**Date**: 2026-08-08

## session_ui.rs

### `pub async fn run_interactive() -> anyhow::Result<()>`

Executa o TUI: tela inicial → loop de eventos → menu. Retorna `Ok(())` ao sair.

- **Entrada**: nenhuma (usa `default_output_folder()` e dir de sessão).
- **Saída**: `Ok(())` no Exit; `Err` com mensagem em qualquer falha fatal.
- **Efeito colateral**: restaura e persiste a sessão (via `Session`).

### `fn render_welcome() -> String`

- **Entrada**: nada.
- **Saída**: arte ASCII "OPENTORRENT" + dicas centralizadas (String).

### `fn parse_menu_input(line: &str) -> Option<MenuCommand>`

- **Entrada**: texto digitado na caixa de entrada.
- **Saída**: `Some(MenuCommand)` se o texto começa com `/` e casa um comando
  conhecido; `None` caso contrário (comando desconhecido → ignorar).
- **Contrato**: `/a`/`a`, `/c`/`c`, `/l`/`l`, `/s`/`s` mapeiam às quatro opções.

## downloads.rs

### `pub fn list_completed_downloads(dir: &Path) -> anyhow::Result<Vec<CompletedItem>>`

- **Entrada**: diretório de downloads (padrão).
- **Saída**: lista de `CompletedItem { path, name, size, created }` dos
  arquivos no diretório (recursivo).
- **Contrato**: retorna lista vazia (sem erro) se diretório não existe.

### `pub fn format_completed(item: &CompletedItem) -> String`

- **Entrada**: item de download completo.
- **Saída**: linha formatada com nome, pasta, tamanho (size_format) e data.

## Persistência (librqbit)

- `SessionOptions { persistence: Some(SessionPersistenceConfig::Json { folder:
  Some(<~/.config/opentorrent/session>) }) }`.
- Restauração e gravação automáticas; nenhum contrato extra.
