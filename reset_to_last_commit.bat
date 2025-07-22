@echo off
echo Reverting all changes to the last commit...

:: Ask for confirmation
choice /M "Are you sure you want to reset all changes?" /C YN
if errorlevel 2 goto cancel

:: Navigate to repo folder
cd /d E:\Dev\SolanaWalletAnalyzer\wallet-analyzer

:: Check if it's a Git repo
if not exist .git (
    echo Error: This directory is not a Git repository.
    pause
    exit /b 1
)

:: Restore all files to HEAD
git reset --hard

:: Remove untracked files and folders
git clean -fd

echo ✅ Repository reset to last commit.
pause
exit /b 0

:cancel
echo ❌ Reset cancelled.
pause
exit /b 1
