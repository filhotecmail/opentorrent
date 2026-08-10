#!/usr/bin/env bash
set -euo pipefail

# Atualiza as seções dinâmicas do README.md (badges e estado do projeto).
# Chamado pelo workflow .github/workflows/readme-live.yml após mudanças no repo.

REPO="${GITHUB_REPOSITORY:-filhotecmail/opentorrent}"
FILE="README.md"
[ -f "$FILE" ] || { echo "ERRO: $FILE não encontrado" >&2; exit 1; }

TMPDIR_C="${TMPDIR:-/tmp}/opencode"
mkdir -p "$TMPDIR_C"

gh api "repos/$REPO" > "$TMPDIR_C/repo.json" 2>/dev/null || exit 1
open_issues=$(jq -r '.open_issues_count' "$TMPDIR_C/repo.json")
default_branch=$(jq -r '.default_branch' "$TMPDIR_C/repo.json")

gh api "repos/$REPO/milestones?state=open" > "$TMPDIR_C/mils.json" 2>/dev/null || exit 1
milestone=$(jq -r '.[0].title // "—"' "$TMPDIR_C/mils.json")

gh api "repos/$REPO/labels" > "$TMPDIR_C/labels.json" 2>/dev/null || exit 1
nlabels=$(jq -r 'length' "$TMPDIR_C/labels.json")

latest_tag=$(gh release view --repo "$REPO" --json tagName -q .tagName 2>/dev/null || echo "—")

# --- 1) Badges dinâmicos ----------------------------------------------
# Mantém o mesmo conteúdo do bloco BADGES_START do README.md. As badges com
# valor dinâmico (branch/tag) usam as variáveis acima; as demais são estáveis.
# Layout plano (sem agrupamentos), aprovado para o README: MSRV, Plataforma,
# Downloads, CI PR, CI schedule, CodeQL, Cobertura, Último commit, Codespaces,
# Release, Issues abertas, Gemfury (apt) e APT package.
msrv_badge="[![MSRV](https://img.shields.io/badge/MSRV-1.85+-orange?logo=rust)](https://github.com/$REPO/blob/$default_branch/Cargo.toml)"
platform_badge="[![Plataforma](https://img.shields.io/badge/plataforma-linux%20x86__64%20%7C%20windows%20x86__64-blue)](https://github.com/$REPO/releases/latest)"
downloads_badge="[![Downloads](https://img.shields.io/github/downloads/$REPO/total?color=2ea44f&label=downloads)](https://github.com/$REPO/releases)"
ci_pr_badge="[![CI pull request](https://github.com/$REPO/actions/workflows/ci.yml/badge.svg?event=pull_request)](https://github.com/$REPO/actions/workflows/ci.yml)"
ci_sched_badge="[![CI schedule](https://github.com/$REPO/actions/workflows/ci.yml/badge.svg?event=schedule)](https://github.com/$REPO/actions/workflows/ci.yml)"
codeql_badge="[![CodeQL](https://github.com/$REPO/actions/workflows/codeql.yml/badge.svg?branch=$default_branch)](https://github.com/$REPO/security/code-scanning)"
cov_badge="[![Cobertura](https://codecov.io/gh/$REPO/branch/$default_branch/graph/badge.svg)](https://codecov.io/gh/$REPO)"
last_commit_badge="[![Último commit](https://img.shields.io/github/last-commit/$REPO/$default_branch)](https://github.com/$REPO/commits/$default_branch)"
codespaces_badge="[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/$REPO)"
release_badge="[![Release](https://img.shields.io/badge/release-$latest_tag-blue)](https://github.com/$REPO/releases)"
issues_badge="[![Issues abertas](https://img.shields.io/github/issues/$REPO)](https://github.com/$REPO/issues)"
gemfury_badge="[![Gemfury Badge](https://badge.fury.io/apt/opentorrent.svg)](https://badge.fury.io/apt/opentorrent)"
apt_badge="[![APT Package](https://img.shields.io/badge/Debian%2FAPT-.deb-A81D33?style=flat-square&logo=debian&logoColor=white)](https://github.com/$REPO/releases/latest)"

badges="
$msrv_badge
$platform_badge
$downloads_badge
$ci_pr_badge
$ci_sched_badge
$codeql_badge
$cov_badge
$last_commit_badge
$codespaces_badge
$release_badge
$issues_badge
$gemfury_badge
$apt_badge"

estado="| Estado | Valor |
| --- | --- |
| Branch principal | \`$default_branch\` |
| Última release | \`$latest_tag\` |
| Milestone atual | $milestone |
| Issues abertas | $open_issues |
| Labels do projeto | $nlabels |"

cargo_info="| Metadado | Valor |
| --- | --- |
| Pacote | — |
| Versão | — |
| Edição Rust | — |"
if [ -f Cargo.toml ]; then
  pkg_name=$(awk -F'=' '/^name/ {gsub(/[" ]/,"",$2); print $2; exit}' Cargo.toml)
  pkg_ver=$(awk -F'=' '/^version/ {gsub(/[" ]/,"",$2); print $2; exit}' Cargo.toml)
  rust_ed=$(awk -F'=' '/^edition/ {gsub(/[" ]/,"",$2); print $2; exit}' Cargo.toml)
  cargo_info="| Metadado | Valor |
| --- | --- |
| Pacote | \`$pkg_name\` |
| Versão | \`$pkg_ver\` |
| Edição Rust | $rust_ed |"
fi

# --- Aplicação (via Python para evitar conflito de delimitadores) -------
BADGES="$badges" ESTADO="$estado" CARGO="$cargo_info" python3 - "$FILE" <<'PY'
import os, sys

f = sys.argv[1]
with open(f, encoding="utf-8") as fh:
    content = fh.read()

def block(name):
    return os.environ[name].strip()

for marker, var in (("BADGES", "BADGES"), ("ESTADO", "ESTADO"), ("CARGO", "CARGO")):
    start = f"<!-- {marker}_START -->"
    end = f"<!-- {marker}_END -->"
    s = content.find(start)
    e = content.find(end)
    if s == -1 or e == -1:
        print(f"AVISO: marcadores {marker} não encontrados; pulando.")
        continue
    e += len(end)
    content = content[:s] + start + "\n" + block(var) + "\n" + end + content[e:]

with open(f, "w", encoding="utf-8") as fh:
    fh.write(content)

print("README.md atualizado.")
PY
