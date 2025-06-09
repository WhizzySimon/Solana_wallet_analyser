@echo off
:: Batch script to add, commit (with user message), and push changes in Git

setlocal
:: Prompt user for commit message
set /p message="Enter commit message: "

:: Stage all changes
git add .

:: Commit with the provided message
git commit -m "%message%"

:: Push to remote (origin/main by default)
git push

pause
