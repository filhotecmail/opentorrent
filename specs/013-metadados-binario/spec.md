# US-021 — Injeção de metadados e propriedades de projeto no binário de Release

## Contexto

O executável de release precisa carregar, de forma embutida e auditável, as propriedades do
projeto (nome, URL do repositório e objetivo/descrição) para permitir rastrear a procedência sem
depender de arquivos externos. A US-018 já assina o binário; esta US embute os metadados na
própria estrutura ELF, de forma transparente ao laço de desenvolvimento.

## Critérios de aceitação

- **AC-1 — Extração das configurações:** o `build.rs` extrai nome, repositório e descrição das
  configurações do projeto (Cargo.toml, via `CARGO_PKG_*`).
- **AC-2 — Seção dedicada:** as propriedades são embutidas em uma seção somente leitura do ELF,
  `.note.opentorrent`, no formato GNU note (reconhecido por `readelf -n`).
- **AC-3 — Leitura nativa:** o binário em `target/release/` permite ler as propriedades com
  ferramentas nativas do Linux (`strings` e `readelf`).
- **AC-4 — Transparência:** a injeção é automática (build script), não afeta a execução nem a
  assinatura digital (a assinatura da US-018 ocorre após a compilação, sobre o binário completo).

## Cenários de teste

1. **Verificação dos metadados:** `cargo build --release` + `strings`/`readelf` exibem nome, link
   do repositório e objetivo.
2. **Atualização dinâmica:** alterar `repository`/`description` no Cargo.toml reexecuta o build
   script (`rerun-if-changed=Cargo.toml`) e os novos valores aparecem no binário.

## Decisões de implementação

- `build.rs` monta a nota ELF GNU: `namesz | descsz | type=0 | "opentorrent\0" | desc`, com
  `desc = "OpenTorrent v{versão}\nRepositorio: {url}\nObjetivo: {descrição}\n"` e padding a 4 bytes.
  Grava `OUT_DIR/opentorrent-note.bin` e a constante de tamanho `opentorrent_note_size.rs`
  (o `_` não é permitido em static).
- `src/metadata.rs` injeta os bytes na seção com `#[used] #[link_section = ".note.opentorrent"]`
  (`#[used]` impede dead-stripping mesmo com LTO fat + `strip` do release).
- Validação: `readelf -S` (seção), `readelf -n` (nota com owner `opentorrent`), `strings`
  (texto legível); testes unitários do formato da nota (42 no total).
- A assinatura digital (US-018) é gerada após o build e continua válida.
