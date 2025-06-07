@echo off
REM ====== init_wallet_analyzer_repo.bat ======
REM Initialize git repo and upload to GitHub

REM Set project folder (edit if needed)
set PROJECT_DIR=%CD%
cd "%PROJECT_DIR%"

REM Initialize git repo if not already
if not exist .git (
    git init
)

REM Add all files
git add .

REM Make sure there is at least one commit (otherwise main branch won't exist)
git diff --cached --quiet || git commit -m "Initial commit"

REM Rename branch to main (GitHub default)
git branch -M main

REM Ask for GitHub repo URL (or hardcode here)
set /p REMOTE_URL="Enter your GitHub repo URL (e.g. https://github.com/youruser/yourrepo.git): "

REM Add remote (ignore errors if remote already exists)
git remote add origin "%REMOTE_URL%" 2>nul

REM Push to GitHub (set upstream if first push)
git push -u origin main

echo.
echo Done! Check your repo at: %REMOTE_URL%
pause
