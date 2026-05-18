# Start scsynth with UDP listening on all interfaces
# Usage: powershell.exe -File start-scsynth.ps1 [-Port 57110]

param(
    [int]$Port = 58888
)

$scsynth = "C:\Program Files\SuperCollider-3.14.1\scsynth.exe"

# Kill existing scsynth
Get-Process scsynth -ErrorAction SilentlyContinue | Stop-Process -Force 2>$null
Start-Sleep -Seconds 1

# Start scsynth with audio output enabled
# -H <device> selects audio API. Use PortAudio default device.
# -o 2 = stereo out, -i 0 = no input
# -B 0.0.0.0 = listen on all interfaces for WSL2
# WindowStyle Minimized (not Hidden) so audio device attaches
$args = @("-u", $Port, "-B", "0.0.0.0", "-a", "1024", "-o", "2", "-i", "0")
Write-Host "Starting scsynth on 0.0.0.0:$Port"
Start-Process -FilePath $scsynth -ArgumentList $args -WindowStyle Minimized
Start-Sleep -Seconds 3
Write-Host "scsynth started (PID: $((Get-Process scsynth).Id))"
