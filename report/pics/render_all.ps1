$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$out = Join-Path $here "rendered"
New-Item -ItemType Directory -Force -Path $out | Out-Null

Get-ChildItem -Path $here -Filter *.dot | ForEach-Object {
    $base = $_.BaseName
    dot -Tpng $_.FullName -o (Join-Path $out "$base.png")
    dot -Tpdf $_.FullName -o (Join-Path $out "$base.pdf")
}

Write-Output "Rendered files are in: $out"
