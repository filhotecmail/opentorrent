#!/usr/bin/env bash
# Pipeline de User Stories (US-017): automatiza o ciclo de vida de uma US.
#
# Fluxo:
#   1. `start`  — sincroniza a branch principal, cria a branch de trabalho e
#                 registra a issue correspondente no repositório.
#   2. (o desenvolvedor aplica as alterações na branch criada)
#   3. `finish` — valida o pipeline (fmt/clippy/test/machete), faz commit e
#                 push, abre o PR para a branch principal vinculando a issue
#                 ("Closes #N") — o GitHub fecha a issue ao mergear.
#
# Uso:
#   ./scripts/us-pipeline.sh start US-XXX "Título da US" ["descrição em markdown"]
#   ./scripts/us-pipeline.sh finish ["mensagem de commit"]
#   ./scripts/us-pipeline.sh state
#
# Requisitos: gh (GitHub CLI) autenticado, cargo na PATH.
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."

MAIN="master"
STATE_FILE=".us-pipeline.state"

die() { echo "erro: $*" >&2; exit 1; }

slug() {
  # "Melhoria na entrada de origem" -> "melhoria-na-entrada-de-origem"
  echo "$1" | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//' | cut -c1-40
}

read_state() {
  if [ -f "$STATE_FILE" ]; then
    # shellcheck disable=SC1090
    . "$STATE_FILE"
  fi
}

repo_name() {
  gh repo view --json nameWithOwner --jq '.nameWithOwner'
}

ensure_label() {
  local repo="$1"
  gh label create "US" --color "0E8A16" --description "User Story" \
    --force --repo "$repo" >/dev/null 2>&1 || true
}

ensure_milestone() {
  # O workflow issue-quality exige milestone; usa o primeiro aberto ou cria um.
  local repo="$1"
  local milestone
  milestone="$(gh api "repos/$repo/milestones?state=open" \
    --jq '.[0].title // empty' 2>/dev/null || true)"
  if [ -z "$milestone" ]; then
    gh api "repos/$repo/milestones" --method POST \
      -f title="v1.0" -f description="Milestone padrão do pipeline de US" \
      >/dev/null 2>&1 || true
    milestone="v1.0"
  fi
  echo "$milestone"
}

cmd_start() {
  local id="${1:-}" title="${2:-}" body="${3:-$title}"
  require_gh
  [ -n "$id" ] || die "informe o id da US (ex.: US-018)"
  [ -n "$title" ] || die "informe o título da US"

  local repo branch issue_number milestone
  repo="$(repo_name)"

  # AC-1: identifica a nova US e cria a branch a partir da principal.
  git fetch origin "$MAIN" >/dev/null
  git checkout "$MAIN" >/dev/null 2>&1
  git pull --ff-only origin "$MAIN" >/dev/null
  branch="feat/$(echo "$id" | tr '[:upper:]' '[:lower:]')-$(slug "$title")"
  git checkout -b "$branch"

  # AC-2: registra a issue com título e descrição da US.
  ensure_label "$repo"
  milestone="$(ensure_milestone "$repo")"
  issue_number="$(gh issue create --repo "$repo" \
    --title "[$id] $title" \
    --label "US" \
    --milestone "$milestone" \
    --body "$body" | grep -oE '[0-9]+$')"
  [ -n "$issue_number" ] || die "falha ao criar a issue"

  printf 'BRANCH=%s\nISSUE=%s\nUS_ID=%s\n' "$branch" "$issue_number" "$id" > "$STATE_FILE"
  echo "issue #$issue_number criada (milestone: $milestone)"
  echo "branch de trabalho: $branch"
  echo "aplicando as alterações e, ao concluir, rode: ./scripts/us-pipeline.sh finish"
}

cmd_finish() {
  local msg="${1:-}"
  require_gh
  read_state
  [ -n "${BRANCH:-}" ] || die "execute 'start' antes de 'finish'"
  [ -n "${ISSUE:-}" ] || die "estado da US incompleto (sem ISSUE)"

  # AC-3: as alterações são aplicadas exclusivamente na branch da US.
  local current
  current="$(git branch --show-current)"
  [ "$current" = "$BRANCH" ] || die "você está em '$current'; mude para '$BRANCH'"

  # Pipeline de validação antes do commit.
  echo "==> cargo fmt"
  cargo fmt --all -- --check
  echo "==> cargo clippy (-D warnings)"
  cargo clippy --all-targets --all-features -- -D warnings
  echo "==> cargo test"
  cargo test --locked --all-targets
  echo "==> cargo machete"
  cargo machete

  # AC-4: commit com as alterações e push para o remoto.
  if [ -z "$msg" ]; then
    msg="feat(${US_ID}): $(gh issue view "$ISSUE" --json title --jq '.title' 2>/dev/null \
      || echo 'implementação da US')"
  fi
  git add -A
  git commit -m "$msg"
  git push -u origin "$BRANCH"

  # AC-5/AC-6: PR para a principal referenciando a issue — o GitHub fecha a
  # issue automaticamente quando o PR é integrado ("Closes #N").
  gh pr create --base "$MAIN" --head "$BRANCH" \
    --title "$msg" \
    --body "Implementa ${US_ID} — closes #${ISSUE}" \
    --repo "$(repo_name)"
}

cmd_state() {
  read_state
  if [ -z "${BRANCH:-}" ]; then
    echo "nenhuma US em andamento"
  else
    echo "branch: ${BRANCH}"
    echo "issue:  #${ISSUE:-?}"
    echo "us:     ${US_ID:-?}"
  fi
}

require_gh() {
  command -v gh >/dev/null 2>&1 || die "gh (GitHub CLI) não está instalado"
  gh auth status >/dev/null 2>&1 || die "gh não está autenticado"
}

case "${1:-}" in
  start) cmd_start "${2:-}" "${3:-}" "${4:-}" ;;
  finish) cmd_finish "${2:-}" ;;
  state) cmd_state ;;
  *)
    echo "Uso: $0 {start US-XXX \"título\" [descrição] | finish [mensagem] | state}"
    exit 1
    ;;
esac
