# US-021 — Tasks

## Implementação

- [x] `build.rs`: extrai `CARGO_PKG_NAME`/`REPOSITORY`/`DESCRIPTION`/`VERSION` e monta a nota ELF GNU (`.note.opentorrent`), com `rerun-if-changed=Cargo.toml`
- [x] `src/metadata.rs`: `#[used] #[link_section = ".note.opentorrent"]` embute a nota; constante de tamanho gerada pelo build script
- [x] Validação nativa: `readelf -S` (seção), `readelf -n` (nota owner `opentorrent`), `strings` (nome/repositório/objetivo)
- [x] Testes unitários do formato da nota (2 novos — 42 no total)
- [x] Pipeline local: fmt, clippy (-D warnings), test (42), machete — OK

## Publicação

- [ ] Commit + push da branch `feat/us-021-metadados-binario`
- [ ] PR para master com vinculação da US
- [ ] Merge squash após CI verde
- [ ] Master: build release assinado v0.2.0 (bump automático)
- [ ] Validar metadados no binário v0.2.0 (strings/readelf) e assinatura intacta
- [ ] Instalar binário em `/usr/local/bin` e publicar release v0.2.0 (binário + `.sig`)
