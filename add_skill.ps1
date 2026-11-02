# Copy a skill into a project so Claude Code (.claude\skills) and Codex and Pi
# (.agents\skills) can all use it. Windows version of add_skill.sh.
#
# Usage: powershell -ExecutionPolicy Bypass -File .\add_skill.ps1 SKILL [PROJECT]
#   SKILL    a folder that contains SKILL.md, or a single SKILL.md file
#   PROJECT  the project folder (default: the current folder)
param(
    [Parameter(Mandatory = $true, Position = 0)][string]$Skill,
    [Parameter(Position = 1)][string]$Project = "."
)
$ErrorActionPreference = "Stop"

function Fail([string]$Message) {
    Write-Host "Error: $Message" -ForegroundColor Red
    exit 1
}

if (Test-Path -LiteralPath $Skill -PathType Container) {
    $skillMd = Join-Path $Skill "SKILL.md"
    if (-not (Test-Path -LiteralPath $skillMd -PathType Leaf)) { Fail "$Skill has no SKILL.md" }
    $isFolder = $true
} elseif (Test-Path -LiteralPath $Skill -PathType Leaf) {
    $skillMd = $Skill
    $isFolder = $false
} else {
    Fail "$Skill not found"
}
if (-not (Test-Path -LiteralPath $Project -PathType Container)) { Fail "project folder $Project not found" }

# Read the fields from the frontmatter between the two --- lines at the top of SKILL.md.
$fields = @{}
$lines = @(Get-Content -LiteralPath $skillMd -Encoding UTF8)
if ($lines.Count -gt 0 -and $lines[0].Trim() -eq "---") {
    for ($i = 1; $i -lt $lines.Count -and $lines[$i].Trim() -ne "---"; $i++) {
        if ($lines[$i] -match '^(name|description):\s*(.*)$') {
            $fields[$Matches[1]] = $Matches[2].Trim().Trim('"', "'")
        }
    }
}
$name = $fields["name"]
if (-not $name -or -not $fields["description"]) {
    Fail "SKILL.md must start with:`n---`nname: my-skill`ndescription: What the skill does and when to use it.`n---"
}
if ($name -cnotmatch '^[a-z0-9]+(-[a-z0-9]+)*$' -or $name.Length -gt 64) {
    Fail "name '$name' must use lowercase letters, numbers and single hyphens (at most 64 characters)"
}

foreach ($dir in @(".claude\skills", ".agents\skills")) {
    $dest = Join-Path (Join-Path $Project $dir) $name
    # Running the script on a skill that is already in place must not delete it.
    if ($isFolder -and (Test-Path -LiteralPath $dest) -and
        ((Resolve-Path -LiteralPath $Skill).Path -eq (Resolve-Path -LiteralPath $dest).Path)) {
        Write-Host "Already in place: $dest"
        continue
    }
    if (Test-Path -LiteralPath $dest) {
        Remove-Item -LiteralPath $dest -Recurse -Force
        $action = "Replaced"
    } else {
        $action = "Added"
    }
    New-Item -ItemType Directory -Path $dest -Force | Out-Null
    if ($isFolder) {
        Get-ChildItem -LiteralPath $Skill -Force | Copy-Item -Destination $dest -Recurse -Force
    } else {
        Copy-Item -LiteralPath $skillMd -Destination (Join-Path $dest "SKILL.md") -Force
    }
    Write-Host "${action}: $dest"
}

Write-Host ""
Write-Host "Done. Commit it in your project:"
Write-Host "  git add .claude/skills .agents/skills"
Write-Host "  git commit -m `"Add $name skill`""
Write-Host "Then ask for it in a Task prompt: `"Use the $name skill to ...`""
