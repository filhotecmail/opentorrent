#!/usr/bin/env bash
# create-new-us.sh — automatiza o ciclo de vida de uma User Story (US).
#
# Fluxo:
#   1. `start` — sincroniza `master`, cria a branch local a partir dela e
#                abre a Issue no repositório com milestone e label (exigência
#                do workflow `.github/workflows/issue-quality.yml`).
#   2. (o desenvolvedor/agente aplica as alterações na branch criada)
#   3. `execute-pipeline-gh` — valida o pipeline (fmt/clippy/test/machete),
#                faz commit, push da branch e abre o PR para `master`
#                vinculando a issue ("Closes #N") para entrar no ciclo CI/CD.
#
# Uso:
#   ./scripts/create-new-us.sh start US-038 "Título da US" ["descrição em markdown"]
#   ./scripts/create-new-us.sh execute-pipeline-gh ["mensagem de commit"]
#   ./scripts/create-new-us.sh state
#
# Requisitos: gh (GitHub CLI) autenticado, cargo na PATH, git.
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."

MAIN="master"
STATE_FILE=".us-pipeline.state"

die() { echo "erro: $*" >&2; exit 1; }

require_gh() {
  command -v gh >/dev/null 2>&1 || die "gh (GitHub CLI) não está instalado"
  gh auth status >/dev/null 2>&1 || die "gh não está autenticado"
}

repo_name() {
  gh repo view --json nameWithOwner --jq '.nameWithOwner'
}

slug() {
  # "Melhoria na entrada de origem" -> "melhoria-na-entrada-de-origem"
  echo "$1" | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//' | cut -c1-40
}

read_state() {
  if [ -f "$STATE_FILE" ]; then
    while IFS='=' read -r key value; do
      case "$key" in
        BRANCH) BRANCH="$value" ;;
        ISSUE) ISSUE="$value" ;;
        US_ID) US_ID="$value" ;;
      esac
    done < "$STATE_FILE"
  fi
}

ensure_label() {
  local repo="$1" label="${2:-US}"
  gh label create "$label" --color "0E8A16" --description "User Story" \
    --force --repo "$repo" >/dev/null 2>&1 || true
}

ensure_milestone() {
  # O workflow issue-quality exige milestone; usa o primeiro aberto ou cria.
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
  [ -n "$id" ] || die "informe o id da US (ex.: US-038)"
  [ -n "$title" ] || die "informe o título da US"

  local repo branch issue_number milestone
  repo="$(repo_name)"

  if [ -n "$(git status --porcelain)" ]; then
    die "há alterações não commitadas; commite ou faça stash antes de start"
  fi

  # Branch local criada a partir de master sincronizada.
  git fetch origin "$MAIN" >/dev/null
  git checkout "$MAIN" >/dev/null 2>&1
  git pull --ff-only origin "$MAIN" >/dev/null
  branch="feat/$(echo "$id" | tr '[:upper:]' '[:lower:]')-$(slug "$title")"
  git checkout -b "$branch"

  # Issue com milestone e label obrigatórios.
  ensure_label "$repo"
  milestone="$(ensure_milestone "$repo")"
  issue_number="$(gh issue create --repo "$repo" \
    --title "[$id] $title" \
    --label "US" \
    --milestone "$milestone" \
    --body "$body" | grep -oE '[0-9]+$')"
  [ -n "$issue_number" ] || die "falha ao criar a issue"

  printf 'BRANCH=%s\nISSUE=%s\nUS_ID=%s\n' "$branch" "$issue_number" "$id" > "$STATE_FILE"
  echo "issue #$issue_number criada (milestone: $milestone, label: US)"
  echo "branch de trabalho: $branch"
  echo "ao concluir, rode: ./scripts/create-new-us.sh execute-pipeline-gh"
}

cmd_execute_pipeline_gh() {
  local msg="${1:-}"
  require_gh
  read_state
  [ -n "${BRANCH:-}" ] || die "execute 'start' antes de 'execute-pipeline-gh'"
  [ -n "${ISSUE:-}" ] || die "estado da US incompleto (sem ISSUE)"

  local current
  current="$(git branch --show-current)"
  [ "$current" = "$BRANCH" ] || die "você está em '$current'; mude para '$BRANCH'"

  # Validação obrigatória antes de push/PR (mesmos checks do CI).
  echo "==> cargo fmt"
  cargo fmt --all -- --check
  echo "==> cargo clippy (-D warnings)"
  cargo clippy --all-targets --all-features -- -D warnings
  echo "==> cargo test"
  cargo test --locked --all-targets
  if command -v cargo machete >/dev/null 2>&1; then
    echo "==> cargo machete"
    cargo machete
  else
    echo "==> cargo machete (não instalado — pulado; o CI valida)"
  fi

  if [ -z "$msg" ]; then
    msg="feat(${US_ID}): $(gh issue view "$ISSUE" --json title --jq '.title' 2>/dev/null \
      || echo 'implementação da US')"
  fi
  git add -A
  git commit -m "$msg"
  git push -u origin "$BRANCH"

  # PR para master referenciando a issue — o GitHub fecha a issue ao mergear.
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

case "${1:-}" in
  start) cmd_start "${2:-}" "${3:-}" "${4:-}" ;;
  execute-pipeline-gh) cmd_execute_pipeline_gh "${2:-}" ;;
  state) cmd_state ;;
  *)
    echo "Uso: $0 {start US-XXX \"título\" [descrição] | execute-pipeline-gh [mensagem] | state}"
    exit 1
    ;;
esac
