#Requires -RunAsAdministrator
#Requires -Modules Hyper-V
<#
.SYNOPSIS
Automates the repeatable Hyper-V host scaffolding for the Cowork WP10 full gate.

.DESCRIPTION
AUTOMATED: This harness automates VM creation with nested virtualization, baseline and ready
checkpoints, checkpoint revert, installer copy into the guest, induced failure
conditions, and post-run guest verification.

MANUAL: The Windows 11 installation, the Cowork GUI wizard, and agent OAuth stay manual.
The harness pauses for those manual parts and prints the next action to run.

.NOTES
Run this script elevated on the Hyper-V host. Guest-side commands use
PowerShell Direct through Invoke-Command -VMName, and file copy uses Copy-VMFile.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('create-vm', 'baseline-checkpoint', 'prepare', 'revert', 'induce', 'verify-guest', 'status')]
    [string]$Action,

    [string]$VmName = 'Cowork-FullGate',
    [string]$IsoPath,
    [string]$CoworkSetup,
    [int]$VhdSizeGB = 80,
    [int]$MemoryGB = 8,
    [int]$Cpu = 4,
    [string]$Snapshot = 'ready-to-install',

    [ValidateSet('virt-off', 'virt-on', 'low-disk', 'restore-disk', 'net-drop', 'net-restore', 'existing-ubuntu')]
    [string]$Failure,

    [string]$SwitchName = 'Default Switch',
    [int]$TargetFreeGB = 14,
    [string]$GuestDest = 'C:\Cowork'
)

$ErrorActionPreference = 'Stop'
$BaselineSnap = 'clean-wsl-never-enabled'
$script:GuestCred = $null

function Get-GuestCred {
    if (-not $script:GuestCred) {
        $script:GuestCred = Get-Credential -Message 'the guest local Windows account for PowerShell Direct.'
    }

    return $script:GuestCred
}

function Invoke-Guest {
    param(
        [Parameter(Mandatory)]
        [scriptblock]$Script,

        [object[]]$ArgList = @()
    )

    Invoke-Command -VMName $VmName -Credential (Get-GuestCred) -ScriptBlock $Script -ArgumentList $ArgList
}

function Assert-VmExists {
    if (-not (Get-VM -Name $VmName -ErrorAction SilentlyContinue)) {
        throw "VM '$VmName' does not exist."
    }
}

switch ($Action) {
    'create-vm' {
        if (-not $IsoPath) {
            throw "-IsoPath is required for create-vm."
        }

        if (Get-VM -Name $VmName -ErrorAction SilentlyContinue) {
            throw "VM '$VmName' already exists."
        }

        $vhd = Join-Path (Get-VMHost).VirtualHardDiskPath "$VmName.vhdx"

        New-VM -Name $VmName -Generation 2 -MemoryStartupBytes ($MemoryGB * 1GB) -NewVHDPath $vhd -NewVHDSizeBytes ($VhdSizeGB * 1GB) -SwitchName $SwitchName
        Set-VM -Name $VmName -StaticMemory -AutomaticCheckpointsEnabled $false
        Set-VMProcessor -VMName $VmName -Count $Cpu -ExposeVirtualizationExtensions $true
        Set-VMMemory -VMName $VmName -StartupBytes ($MemoryGB * 1GB)

        $dvd = Add-VMDvdDrive -VMName $VmName -Path $IsoPath -Passthru
        Set-VMFirmware -VMName $VmName -FirstBootDevice $dvd
        Enable-VMIntegrationService -VMName $VmName -Name 'Guest Service Interface'

        Start-VM -Name $VmName
        Write-Host "Install Windows 11, finish OOBE, then run: .\fullgate.ps1 -Action baseline-checkpoint" -ForegroundColor Yellow
    }

    'baseline-checkpoint' {
        Assert-VmExists

        Invoke-Guest -Script {
            $wsl = Get-Command wsl.exe -ErrorAction SilentlyContinue
            $vmp = Get-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform
            $wslFeature = Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Windows-Subsystem-Linux

            [pscustomobject]@{
                WslExe = [bool]$wsl
                VirtualMachinePlatform = $vmp.State
                MicrosoftWindowsSubsystemLinux = $wslFeature.State
            }
        } | Format-List

        Write-Host "Clean baseline means both optional features are Disabled and no distro is registered." -ForegroundColor Yellow
        Checkpoint-VM -Name $VmName -SnapshotName $BaselineSnap
    }

    'prepare' {
        Assert-VmExists

        if (-not $CoworkSetup) {
            throw "-CoworkSetup is required for prepare."
        }

        if (-not (Test-Path $CoworkSetup)) {
            throw "Cowork installer does not exist: $CoworkSetup"
        }

        $destination = Join-Path $GuestDest (Split-Path $CoworkSetup -Leaf)
        Copy-VMFile -VMName $VmName -SourcePath $CoworkSetup -DestinationPath $destination -FileSource Host -CreateFullPath -Force
        Write-Host "Installer copied to $destination."
        Write-Host "Sign the guest browser OUT of the agent accounts before running the GUI wizard." -ForegroundColor Yellow
        Checkpoint-VM -Name $VmName -SnapshotName $Snapshot
    }

    'revert' {
        Assert-VmExists

        Restore-VMCheckpoint -VMName $VmName -Name $Snapshot -Confirm:$false
        Start-VM -Name $VmName -ErrorAction SilentlyContinue
    }

    'induce' {
        Assert-VmExists

        if (-not $Failure) {
            throw "-Failure is required for induce."
        }

        switch ($Failure) {
            'virt-off' {
                Stop-VM -Name $VmName -Force -ErrorAction SilentlyContinue
                Set-VMProcessor -VMName $VmName -ExposeVirtualizationExtensions $false
                Write-Host "Expected wizard result: preflight.virtualization_disabled or preflight.virtualization_unsupported." -ForegroundColor Yellow
            }

            'virt-on' {
                Stop-VM -Name $VmName -Force -ErrorAction SilentlyContinue
                Set-VMProcessor -VMName $VmName -ExposeVirtualizationExtensions $true
            }

            'low-disk' {
                Invoke-Guest -Script {
                    param([int]$TargetFreeGB)

                    $free = (Get-PSDrive C).Free
                    $target = [int64]$TargetFreeGB * 1GB

                    if ($free -le $target) {
                        Write-Host "C: already has $([math]::Round($free / 1GB, 2)) GB free, at or below target $TargetFreeGB GB."
                        return
                    }

                    $bytes = [int64]($free - $target)
                    fsutil file createnew C:\cowork-fill.bin $bytes
                } -ArgList @($TargetFreeGB)

                Write-Host "Expected wizard result: preflight.insufficient_disk." -ForegroundColor Yellow
            }

            'restore-disk' {
                Invoke-Guest -Script {
                    Remove-Item C:\cowork-fill.bin -Force -ErrorAction SilentlyContinue
                }
            }

            'net-drop' {
                Get-VMNetworkAdapter -VMName $VmName | Disconnect-VMNetworkAdapter
                Write-Host "Expected wizard result: common.network_failed Transient." -ForegroundColor Yellow
            }

            'net-restore' {
                Get-VMNetworkAdapter -VMName $VmName | Connect-VMNetworkAdapter -SwitchName $SwitchName
            }

            'existing-ubuntu' {
                Invoke-Guest -Script {
                    # Requires WSL already enabled in the guest; used for the isolation check that a pre-existing Ubuntu survives a Cowork install.
                    wsl.exe --install -d Ubuntu --no-launch
                }
            }
        }
    }

    'verify-guest' {
        Assert-VmExists

        Invoke-Guest -Script {
            Write-Host 'wsl.exe -l -v'
            wsl.exe -l -v

            Write-Host 'Cowork distro checks'
            wsl.exe -d Cowork -- bash -lc 'set +e
echo "workspace"
test -d "$HOME/workspaces/default" && echo "OK $HOME/workspaces/default" || echo "MISSING $HOME/workspaces/default"
echo "toolchain"
test -x "/home/linuxbrew/.linuxbrew/bin/brew" && echo "OK /home/linuxbrew/.linuxbrew/bin/brew" || echo "MISSING /home/linuxbrew/.linuxbrew/bin/brew"
test -x "$HOME/.local/bin/mise" && echo "OK $HOME/.local/bin/mise" || echo "MISSING $HOME/.local/bin/mise"
echo "agents"
for agent in claude codex agy; do
  test -x "$HOME/.local/bin/$agent" && echo "OK $HOME/.local/bin/$agent" || echo "MISSING $HOME/.local/bin/$agent"
done
echo "creds"
ls "$HOME/.cowork/creds"'
        }

        Write-Host "Compare the guest output against docs/wp10-full-gate.md." -ForegroundColor Yellow
    }

    'status' {
        $vm = Get-VM -Name $VmName -ErrorAction SilentlyContinue

        if (-not $vm) {
            Write-Host "VM '$VmName' does not exist."
            break
        }

        $vm | Format-List Name, State, ProcessorCount, MemoryAssigned
        (Get-VMProcessor -VMName $VmName).ExposeVirtualizationExtensions
        Get-VMCheckpoint -VMName $VmName | Select-Object Name, CreationTime | Format-Table -AutoSize
    }
}
