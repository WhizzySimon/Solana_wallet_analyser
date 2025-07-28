@echo off
setlocal

:: Set the wallet address (change if needed)
set WALLET=8YPGtwcECVoxBrUNdP6AXvQAwDPQkuNwNnPYJBvLjbds

:: Send POST request using PowerShell's Invoke-RestMethod
powershell -Command ^
  "Invoke-RestMethod -Uri http://localhost:8080/api/pnl -Method POST -Body '{\"wallet_address\":\"%WALLET%\"}' -ContentType 'application/json' | ConvertTo-Json -Depth 10"

pause
