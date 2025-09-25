# set-debug-env.ps1

# --- DEBUG configuration ---
$Env:DEF_SLEEP_TIME = '1'
$Env:C2_HOST = 'http://127.0.0.1'
$Env:C2_URI = '/'
$Env:SECURITY_TOKEN = 'sfsdfdsfsdfwerwetweewryh1g'
$Env:C2_PORT = '8080'
$Env:AGENT_NAME = 'local_debug_test'


Write-Host " Environment variables set:"
Write-Host "  DEF_SLEEP_TIME   = $Env:DEF_SLEEP_TIME"
Write-Host "  C2_HOST          = $Env:C2_HOST"
Write-Host "  C2_URI           = $Env:C2_URI"
Write-Host "  SECURITY_TOKEN   = $Env:SECURITY_TOKEN"
Write-Host "  C2_PORT          = $Env:C2_PORT"
Write-Host "  AGENT_NAME       = $Env:AGENT_NAME"

Write-Host ""
Write-Host "NOTE: To now use this config, on the C2 stage a new 'agent' which has the same agent name and security token listed here."