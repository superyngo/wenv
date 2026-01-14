# Test file for PowerShell Here-String environment variables

# Single-line env var (backward compatibility)
$env:EDITOR = "code"

# Multi-line PATH using Here-String
$env:PATH = @"
C:\Program Files\bin
D:\tools
E:\utilities
"@

# Another single-line
$env:SHELL = "pwsh"

# Multi-line with special characters
$env:NOTES = @"
Line with "quotes"
Line with $variable
Line with 'single quotes'
"@

# Multi-line with empty lines
$env:CONFIG = @"
line1

line3
"@

# Alias (should still work)
Set-Alias ll Get-ChildItem

# Function (should still work)
function Get-Greeting {
    Write-Host "Hello, World!"
}
