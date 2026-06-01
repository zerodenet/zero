# Set Windows system proxy to zero's local SOCKS5 port.
# Usage: powershell -File scripts\set-proxy.ps1 <port>
#        powershell -File scripts\set-proxy.ps1 off

param([string]$action = "1080")

if ($action -eq "off") {
    Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings" `
        -Name ProxyEnable -Value 0 -Type DWord
    Write-Host "System proxy disabled" -ForegroundColor Green
} else {
    $port = [int]$action
    Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings" `
        -Name ProxyServer -Value "127.0.0.1:$port" -Type String
    Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings" `
        -Name ProxyEnable -Value 1 -Type DWord
    Write-Host "System proxy set to 127.0.0.1:$port" -ForegroundColor Green
}

# Notify Windows of the change
$signature = @'
[DllImport("wininet.dll")]
public static extern bool InternetSetOption(IntPtr hInternet, int dwOption, IntPtr lpBuffer, int dwBufferLength);
'@
$wininet = Add-Type -MemberDefinition $signature -Name WinInet -Namespace Proxy -PassThru
$wininet::InternetSetOption(0, 39, 0, 0) | Out-Null  # INTERNET_OPTION_SETTINGS_CHANGED
$wininet::InternetSetOption(0, 37, 0, 0) | Out-Null  # INTERNET_OPTION_REFRESH
