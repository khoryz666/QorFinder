# Prepares the Quran corpus (Tanzil Uthmani text + English translation via
# risan/quran-json) for QorFinder: one file per ayah (6,236 files total).
#   data/quran/corpus/surah-<s>-ayah-<a>.txt
param(
    [string]$OutDir = "data"
)
$ErrorActionPreference = "Stop"

$corpusDir = Join-Path (Join-Path $OutDir "quran") "corpus"
New-Item -ItemType Directory -Force -Path $corpusDir | Out-Null

$base = "https://raw.githubusercontent.com/risan/quran-json/main/dist/chapters/en"
for ($i = 1; $i -le 114; $i++) {
    $surah = Invoke-RestMethod -Uri "$base/$i.json"
    foreach ($ayah in $surah.verses) {
        $name = "surah-$($surah.id)-ayah-$($ayah.id)"
        $body = "$($ayah.text)`n$($ayah.translation)"
        Set-Content -Path (Join-Path $corpusDir "$name.txt") -Value $body -Encoding utf8
    }
    Write-Host "surah $i/114 done"
}
$total = (Get-ChildItem $corpusDir -Filter "*.txt").Count
Write-Host "Done. Wrote $total ayah files to $corpusDir"
