<#
Adds Larik Text to "Open with" for common text and code files, for the current user.
Run:  powershell -ExecutionPolicy Bypass -File .\Register-LarikText.ps1
Remove: add -Unregister
Windows asks you to confirm a default app yourself: right-click a file,
choose Open with > Choose another app, pick Larik Text, then Always.
#>
param([switch]$Unregister)

$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'larik-text.exe'
$icon = Join-Path $PSScriptRoot 'larik-document.ico'
$classes = 'HKCU:\Software\Classes'
$progId = 'LarikText.Document'
$extensions = @(
    '.txt','.md','.markdown','.mdx','.log','.json','.jsonc','.yaml','.yml','.toml','.ini','.cfg','.conf','.env',
    '.xml','.html','.css','.scss','.csv','.tsv','.sql','.py','.js','.mjs','.ts','.tsx','.jsx','.rs','.go','.java',
    '.kt','.c','.h','.cpp','.hpp','.cs','.rb','.php','.lua','.sh','.bash','.zsh','.ps1','.bat','.mk','.diff','.patch'
)

if ($Unregister) {
    Remove-Item "$classes\$progId" -Recurse -ErrorAction SilentlyContinue
    Remove-Item "$classes\Applications\larik-text.exe" -Recurse -ErrorAction SilentlyContinue
    foreach ($ext in $extensions) {
        Remove-ItemProperty "$classes\$ext\OpenWithProgids" -Name $progId -ErrorAction SilentlyContinue
    }
    Write-Host 'Larik Text was removed from Open with.'
    exit 0
}

if (-not (Test-Path $exe)) { throw "larik-text.exe was not found next to this script." }

New-Item "$classes\$progId" -Force | Out-Null
Set-ItemProperty "$classes\$progId" -Name '(default)' -Value 'Text document'
New-Item "$classes\$progId\DefaultIcon" -Force | Out-Null
Set-ItemProperty "$classes\$progId\DefaultIcon" -Name '(default)' -Value "`"$icon`""
New-Item "$classes\$progId\shell\open\command" -Force | Out-Null
Set-ItemProperty "$classes\$progId\shell\open\command" -Name '(default)' -Value "`"$exe`" `"%1`""

$app = "$classes\Applications\larik-text.exe"
New-Item "$app\shell\open\command" -Force | Out-Null
Set-ItemProperty $app -Name 'FriendlyAppName' -Value 'Larik Text'
Set-ItemProperty "$app\shell\open\command" -Name '(default)' -Value "`"$exe`" `"%1`""
New-Item "$app\SupportedTypes" -Force | Out-Null
foreach ($ext in $extensions) {
    Set-ItemProperty "$app\SupportedTypes" -Name $ext -Value ''
    New-Item "$classes\$ext\OpenWithProgids" -Force | Out-Null
    New-ItemProperty "$classes\$ext\OpenWithProgids" -Name $progId -PropertyType String -Value '' -Force | Out-Null
}
Write-Host 'Larik Text now appears in Open with. Choose it once and pick Always to make it the default.'
