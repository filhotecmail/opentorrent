# US-029 — Tasks

## Implementação

- [x] Flag `--version`/`-V` customizado (removido o automático do clap).
- [x] `run_version_check()` imprime a versão local e consulta a release.
- [x] `fetch_latest_release()` com timeout de 3s e falha graciosa (None).
- [x] Comparação SemVer (`has_update`) com malformados → false.
- [x] Comando de atualização (`update_command`) com o asset da release.
- [x] Mensagem "versão mais recente" quando atualizado.
- [x] Deps reqwest/semver/serde_json declaradas (já no lock via librqbit).
- [x] Helper `build_runtime()` compartilhado.

## Testes

- [x] `has_update_detects_newer_release`
- [x] `has_update_false_when_equal_or_older`
- [x] `has_update_ignores_malformed_versions`
- [x] `update_command_targets_release_asset`
- [x] Suíte existente (53 testes) verde + clippy `-D warnings` + machete.
- [x] Validação prática: `--version`/`-V` com a release atual (0.28s, "mais recente").

## Publicação

- [ ] Specs em `specs/017-check-versao/`
- [ ] Commit + push da branch `feat/us-029-check-versao`
- [ ] PR para `master` com CI verde e merge
- [ ] Build release assinado v0.1.16 + instalação + release no GitHub
- [ ] Validação do `--version` no binário de release instalado
