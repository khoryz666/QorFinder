# Prepares a BEIR dataset (https://github.com/beir-cellar/beir) for QorFinder:
#   data/<dataset>/corpus/<doc_id>.txt   one file per document
#   data/<dataset>/queries.tsv           qid <tab> query text
#   data/<dataset>/qrels.tsv             TREC/BEIR qrels
# Then: qorfinder index data/<dataset>/corpus --collection <dataset>
#       qorfinder eval --corpus data/<dataset>/corpus --queries data/<dataset>/queries.tsv --qrels data/<dataset>/qrels.tsv
param(
    [string]$Dataset = "scifact",
    [string]$OutDir = "data"
)
$ErrorActionPreference = "Stop"

$root = Join-Path $OutDir $Dataset
$zip = Join-Path $OutDir "$Dataset.zip"
New-Item -ItemType Directory -Force -Path $root | Out-Null

$url = "https://public.ukp.informatik.tu-darmstadt.de/thakur/BEIR/datasets/$Dataset.zip"
if (-not (Test-Path $zip)) {
    Write-Host "Downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $zip
} else {
    Write-Host "Using existing $zip"
}
Expand-Archive -Path $zip -DestinationPath $root -Force

$corpusJson = Get-ChildItem $root -Filter "corpus.jsonl" -Recurse | Select-Object -First 1
$queriesJson = Get-ChildItem $root -Filter "queries.jsonl" -Recurse | Select-Object -First 1
$qrelsFile = Get-ChildItem $root -Filter "*.tsv" -Recurse | Where-Object { $_.Name -eq "test.tsv" } | Select-Object -First 1
if (-not $corpusJson -or -not $queriesJson -or -not $qrelsFile) {
    throw "expected corpus.jsonl, queries.jsonl and qrels/test.tsv inside $zip"
}

$corpusDir = Join-Path $root "corpus"
New-Item -ItemType Directory -Force -Path $corpusDir | Out-Null

Write-Host "Writing corpus files from $($corpusJson.FullName) ..."
$count = 0
foreach ($line in Get-Content $corpusJson.FullName) {
    $doc = $line | ConvertFrom-Json
    $id = ($doc._id -replace '[\\/:*?"<>|]', '_')
    $body = if ($doc.title) { "$($doc.title)`n$($doc.text)" } else { $doc.text }
    Set-Content -Path (Join-Path $corpusDir "$id.txt") -Value $body -Encoding utf8
    $count++
}
Write-Host "Wrote $count corpus files to $corpusDir"

$queriesOut = Join-Path $root "queries.tsv"
Write-Host "Writing queries from $($queriesJson.FullName) ..."
$sb = New-Object System.Text.StringBuilder
foreach ($line in Get-Content $queriesJson.FullName) {
    $q = $line | ConvertFrom-Json
    [void]$sb.AppendLine("$($q._id)`t$($q.text)")
}
Set-Content -Path $queriesOut -Value $sb.ToString() -Encoding utf8

$qrelsOut = Join-Path $root "qrels.tsv"
Copy-Item $qrelsFile.FullName $qrelsOut -Force

Write-Host "Done."
Write-Host "  corpus:  $corpusDir"
Write-Host "  queries: $queriesOut"
Write-Host "  qrels:   $qrelsOut"
Write-Host "Next:"
Write-Host "  qorfinder.exe index `"$corpusDir`" --collection $Dataset"
Write-Host "  qorfinder.exe eval --corpus `"$corpusDir`" --queries `"$queriesOut`" --qrels `"$qrelsOut`" --collection $Dataset"
