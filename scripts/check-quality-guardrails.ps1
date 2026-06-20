param(
    [string]$RepositoryRoot = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = 'Stop'
$errors = [System.Collections.Generic.List[string]]::new()

$localeDir = Join-Path $RepositoryRoot 'crates/gui/src/i18n/locales'
$english = Get-Content -Raw (Join-Path $localeDir 'en.json') | ConvertFrom-Json -AsHashtable
$usedKeys = [System.Collections.Generic.HashSet[string]]::new()

Get-ChildItem (Join-Path $RepositoryRoot 'crates/gui/src') -Recurse -Filter '*.rs' | ForEach-Object {
    $content = Get-Content -Raw $_.FullName
    foreach ($match in [regex]::Matches($content, '\btrf?\(\s*"((?:[^"\\]|\\.)*)"')) {
        [void]$usedKeys.Add([regex]::Unescape($match.Groups[1].Value))
    }
}

foreach ($key in $usedKeys) {
    if (-not $english.ContainsKey($key)) {
        $errors.Add("i18n: key used by UI is missing from en.json: $key")
    }
}

Get-ChildItem $localeDir -Filter '*.json' | ForEach-Object {
    $locale = Get-Content -Raw $_.FullName | ConvertFrom-Json -AsHashtable
    foreach ($key in $locale.Keys) {
        if (-not $english.ContainsKey($key)) {
            $errors.Add("i18n: $($_.Name) contains a key absent from en.json: $key")
        }
    }
}

Get-ChildItem (Join-Path $RepositoryRoot 'crates') -Recurse -Filter '*.rs' |
    Where-Object { $_.Name -ne 'build.rs' } |
    ForEach-Object {
        $insideTests = $false
        $lineNumber = 0
        foreach ($line in Get-Content $_.FullName) {
            $lineNumber++
            if ($line -match '^\s*#\[cfg\(test\)\]') {
                $insideTests = $true
            }
            if (-not $insideTests -and $line -match '\b(?:unwrap|expect)\s*\(') {
                $relative = [System.IO.Path]::GetRelativePath($RepositoryRoot, $_.FullName)
                $errors.Add("panic boundary: $relative`:$lineNumber uses unwrap/expect in production code")
            }
        }
    }

$baselinePath = Join-Path $RepositoryRoot 'docs/quality-guardrails-baseline.json'
$baseline = Get-Content -Raw $baselinePath | ConvertFrom-Json
$undocumentedPublicItems = 0

Get-ChildItem (Join-Path $RepositoryRoot 'crates') -Recurse -Filter '*.rs' | ForEach-Object {
    $insideTests = $false
    $previousLine = ''
    foreach ($line in Get-Content $_.FullName) {
        if ($line -match '^\s*#\[cfg\(test\)\]') {
            $insideTests = $true
        }
        if (
            -not $insideTests -and
            $line -match '^\s*pub\s+(?:mod|struct|enum|trait|fn|type|const|static)\b' -and
            $previousLine -notmatch '^\s*///|^\s*//!'
        ) {
            $undocumentedPublicItems++
        }
        $previousLine = $line
    }
}

if ($undocumentedPublicItems -gt $baseline.maximumUndocumentedPublicItems) {
    $errors.Add(
        "public API docs: undocumented item count increased from baseline " +
        "$($baseline.maximumUndocumentedPublicItems) to $undocumentedPublicItems"
    )
}

if ($errors.Count -gt 0) {
    $errors | ForEach-Object { Write-Error $_ }
    exit 1
}

Write-Host "Quality guardrails passed: i18n=$($usedKeys.Count) undocumented_public=$undocumentedPublicItems panic_boundaries=0"
