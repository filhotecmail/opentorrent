# US-018 — Tarefas

## Concluídas

- [x] Criar `scripts/sign-release.sh` com `init`, `sign` e `verify`.
- [x] `init`: gera CA local e certificado X.509 com `extendedKeyUsage=codeSigning`
      quando não existem (AC-1).
- [x] `sign`: assina `target/release/opentorrent` com RSA-SHA256 gerando
      `opentorrent.sig` (AC-2).
- [x] `verify`: valida cadeia com a CA raiz, extensão `Code Signing` e integridade
      da assinatura; falha retorna erro (AC-3).
- [x] Integrar assinatura no `build.sh` para builds release (falha aborta a esteira).
- [x] Armazenamento seguro das chaves: `$SIGN_DIR` com permissões restritas, fora
      do versionamento (`.signing/` e `*.sig` no `.gitignore`) (AC-4).
- [x] Testes manuais: cenário 1 (sign + verify OK) e cenário 2 (binário adulterado
      → verificação falha com exit 1).
