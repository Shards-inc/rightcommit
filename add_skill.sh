#!/bin/sh
# Copy a skill into a project so Claude Code (.claude/skills) and Codex and Pi
# (.agents/skills) can all use it.
#
# Usage: ./add_skill.sh SKILL [PROJECT]
#   SKILL    a folder that contains SKILL.md, or a single SKILL.md file
#   PROJECT  the project folder (default: the current folder)
set -eu

usage() {
  echo "Usage: $0 SKILL_FOLDER_OR_FILE [PROJECT_FOLDER]" >&2
  exit 2
}
fail() {
  echo "Error: $1" >&2
  exit 1
}

[ $# -ge 1 ] && [ $# -le 2 ] || usage
src=$1
project=${2:-.}

if [ -d "$src" ]; then
  skill_md="$src/SKILL.md"
  [ -f "$skill_md" ] || fail "$src has no SKILL.md"
elif [ -f "$src" ]; then
  skill_md=$src
else
  fail "$src not found"
fi
[ -d "$project" ] || fail "project folder $project not found"

# Read one field from the frontmatter between the two --- lines at the top of SKILL.md.
field() {
  awk -v key="$1" '
    NR == 1 { if ($0 != "---") exit; next }
    $0 == "---" { exit }
    index($0, key ":") == 1 {
      sub("^" key ":[ \t]*", "")
      gsub(/^["\047]|["\047]$/, "")
      print
      exit
    }
  ' "$skill_md"
}
name=$(field name)
description=$(field description)

[ -n "$name" ] && [ -n "$description" ] || fail "SKILL.md must start with:
---
name: my-skill
description: What the skill does and when to use it.
---"
printf '%s' "$name" | grep -Eq '^[a-z0-9]+(-[a-z0-9]+)*$' && [ ${#name} -le 64 ] ||
  fail "name '$name' must use lowercase letters, numbers and single hyphens (at most 64 characters)"

abs() { (cd "$1" && pwd -P); }
for dir in .claude/skills .agents/skills; do
  dest="$project/$dir/$name"
  # Running the script on a skill that is already in place must not delete it.
  if [ -d "$src" ] && [ -d "$dest" ] && [ "$(abs "$src")" = "$(abs "$dest")" ]; then
    echo "Already in place: $dest"
    continue
  fi
  [ -e "$dest" ] && action=Replaced || action=Added
  rm -rf "$dest"
  mkdir -p "$dest"
  if [ -d "$src" ]; then
    cp -R "$src"/. "$dest"/
  else
    cp "$skill_md" "$dest/SKILL.md"
  fi
  echo "$action: $dest"
done

cat <<EOF

Done. Commit it in your project:
  git add .claude/skills .agents/skills && git commit -m "Add $name skill"
Then ask for it in a Task prompt: "Use the $name skill to ..."
EOF
