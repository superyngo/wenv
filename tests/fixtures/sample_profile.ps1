# Sample PowerShell profile for testing
# This is a test configuration file

# Git aliases
Set-Alias gs git-status
Set-Alias gd git-diff

# Navigation aliases
Set-Alias ll Get-ChildItem

# Environment variables
$env:EDITOR = "code"
$env:MY_PROJECT_DIR = "$HOME\projects"

# Load additional configuration
. .\aliases.ps1

# Greeting function
function Get-Greeting {
    param($Name)
    Write-Host "Hello, $Name!"
}

# Function to create and enter directory
function New-DirectoryAndEnter {
    param($Path)
    New-Item -ItemType Directory -Path $Path -Force
    Set-Location $Path
}
