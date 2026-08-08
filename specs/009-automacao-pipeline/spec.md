# US-017 — Automação do pipeline de desenvolvimento para execução de User Stories

## Descrição

A recepção de uma nova User Story deve acionar automaticamente o fluxo de
criação de branch, geração de issue, commit, push, Pull Request e fechamento
da issue, garantindo rastreabilidade e padronização.

## Critérios de aceite

- **AC-1** — Identificar a chegada de uma nova US e criar imediatamente uma
  branch a partir da ramificação principal.
- **AC-2** — Registrar uma issue correspondente no repositório com o título e a
  descrição da US cadastrada.
- **AC-3** — Aplicar as alterações do projeto exclusivamente na branch criada
  para a demanda.
- **AC-4** — Após a conclusão, registrar o commit com as alterações e efetuar o
  push para o repositório remoto.
- **AC-5** — Abrir um Pull Request direcionado à ramificação principal,
  vinculando a issue criada.
- **AC-6** — Realizar o fechamento automático da issue após a integração do
  Pull Request.

## Implementação

### `scripts/us-pipeline.sh`

Script de automação com três subcomandos:

- **`start US-XXX "título" [descrição]`**
  - Sincroniza `master` (fetch + pull --ff-only).
  - Cria a branch `feat/us-xxx-<slug-do-título>` a partir da principal (AC-1).
  - Registra a issue `[US-XXX] título` com label `US` e milestone (exigência do
    workflow `issue-quality.yml`) e corpo com a descrição da US (AC-2).
  - Salva o estado (branch/issue/us) em `.us-pipeline.state`.
- **`finish [mensagem]`**
  - Verifica que a branch atual é a branch da US (AC-3).
  - Roda o pipeline local: `fmt`, `clippy -D warnings`, `test`, `machete`.
  - Faz `git add -A`, commit (mensagem padrão com o título da issue) e push
    para o remoto (AC-4).
  - Abre o PR para `master` com corpo `Implementa US-XXX — closes #N` (AC-5).
    O GitHub fecha a issue automaticamente quando o PR é integrado (AC-6).
- **`state`** — exibe a US em andamento (branch, issue, id).

O fechamento automático da issue depende do corpo do PR conter `closes #N`,
recurso nativo do GitHub vinculado ao merge do PR.

## Cenários de teste

### Execução do pipeline completo a partir de uma nova US

Dado o repositório com a branch principal atualizada, quando uma nova US é
cadastrada no pipeline (`start` + alterações + `finish`), então a branch de
trabalho é criada, a issue registrada, o commit/push executados e o PR aberto
com a referência à issue.

### Rastreabilidade e vínculo entre PR e Issue

Dado que a branch da US foi atualizada e enviada ao remoto, quando o PR é
gerado pelo pipeline, então o corpo do PR contém `closes #<numero>` apontando
para a issue criada na etapa inicial.
