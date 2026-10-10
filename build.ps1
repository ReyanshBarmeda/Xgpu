param(
    [Parameter(Position = 0)]
    [ValidateSet("host", "codegen", "new-app", "sync-figma", "kernel", "all")]
    [string]$Command = "all",
    [string]$Name = "",
    [string]$FigmaExport = ""
)

$Root = $PSScriptRoot
$HostProj = Join-Path $Root "userland\host\Xdos.Host\Xdos.Host.csproj"

function Invoke-HostTool {
    param([string[]]$Args)
    dotnet run --project $HostProj -- @Args
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

switch ($Command) {
    "codegen" { Invoke-HostTool @("codegen") }
    "new-app" {
        if (-not $Name) { Write-Error "Use: .\build.ps1 new-app -Name MyApp"; exit 2 }
        Invoke-HostTool @("new-app", $Name)
    }
    "sync-figma" {
        if (-not $FigmaExport) { Write-Error "Use: .\build.ps1 sync-figma -FigmaExport path.json"; exit 2 }
        Invoke-HostTool @("sync-figma", $FigmaExport)
    }
    "host" {
        Invoke-HostTool @("codegen")
        dotnet build $HostProj
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
    "kernel" {
        $wsl = Get-Command wsl -ErrorAction SilentlyContinue
        if (-not $wsl) {
            Write-Error "WSL not found. Install WSL and run: wsl make -C /mnt/e/xdos run"
            exit 1
        }
        $drive = ($Root[0].ToString().ToLower())
        $pathInWsl = "/mnt/$drive" + ($Root.Substring(2) -replace '\\', '/')
        wsl make -C $pathInWsl run
    }
    "all" {
        & $PSCommandPath host
        Write-Host "[xdos] host tools ready. Kernel: .\build.ps1 kernel (needs WSL + Linux toolchain)"
    }
}
