$ErrorActionPreference = "Stop"
$herdr = if ($env:HERDR_BIN_PATH) { $env:HERDR_BIN_PATH } else { "herdr" }
& $herdr plugin pane open --plugin lazy-herd --entrypoint menu-windows
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
