#Requires -RunAsAdministrator
#Requires -Modules Hyper-V
<#
.SYNOPSIS
  WP10 full-gate harness for Cowork v0.1 - automates the repeatable Hyper-V
  clean-room scaffolding around the parts that must stay manual.

.DESCRIPTION
  AUTOMATED (host PowerShell + PowerShell Direct into the guest):
    - VM creation: nested virtualization, static memory, sized disk, ISO boot
    - baseline + ready checkpoints, and instant revert between matrix runs
    - copying the Cowork installer into the guest (Copy-VMFile)
    - inducing failure conditions: virtualization off, low disk, network drop,
      a pre-existing Ubuntu distro
    - post-run verification inside the guest: Cowork distro present, a
      pre-existing Ubuntu left present, ~/workspaces/default, agent binaries

  MANUAL by nature (the harness PAUSES for these):
    - installing Windows 11 in the guest (one-time; automate with unattend.xml
      if desired)
    - clicking through the Cowork.exe wizard (it is a GUI, not a CLI)
    - the agent OAuth browser login

  Typical flow:
    1) .\fullgate.ps1 -Action create-vm -IsoPath C:\iso\Win11.iso
       (install Windows 11 in the guest, finish OOBE, enable PS remoting if asked)
    2) .\fullgate.ps1 -Action baseline-checkpoint
    3) .\fullgate.ps1 -Action prepare -CoworkSetup C:\path\to\Cowork-setup.exe
    4) per matrix row:
         .\fullgate.ps1 -Action revert
         .\fullgate.ps1 -Action induce -Failure low-disk        # optional
         <run the Cowork.exe wizard + OAuth manually in the guest>
         .\fullgate.ps1 -Action verify-guest

.NOTES
  Run elevated on the Hyper-V HOST. The guest must have the "Guest Service
  Interface" integration service (enabled by create-vm) for file copy, and
  PowerShell Direct uses a guest local-admin credential (prompted once).
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
    [int]$TargetFreeGB = 14,          # induce low-disk: leave this much free (< 16 GiB hard bar)
    [string]$GuestDest = 'C:\Cowork'  # where the installer is copied inside the guest
)

$ErrorActionPreference = 'Stop'
$BaselineSnap = 'clean-wsl-never-enabled'

# --- guest credential (PowerShell Direct), prompted once -------------------
$script:GuestCred = $null
function Get-GuestCred {
    if (-not $script:GuestCred) {
        $script:GuestCred = Get-Credential -Message "Guest ($VmName) local Windows account (for PowerShell Direct)"
    }
    return $script:GuestCred
}

function Invoke-Guest {
    param([Parameter(Mandatory)][scriptblock]$Script, [object[]]$ArgList)
    Invoke-Command -VMName $VmName -Credential (Get-GuestCred) -ScriptBlock $Script -ArgumentList $ArgList
}

function Assert-VmExists {
    if (-not (Get-VM -Name $VmName -ErrorAction SilentlyContinue)) {
        throw "VM '$VmName' does not exist. Run -Action create-vm first."
    }
}

# --- actions ---------------------------------------------------------------
switch ($Action) {

    'create-vm' {
        if (-not $IsoPath) { throw "-IsoPath <Win11.iso> is required for create-vm." }
        if (Get-VM -Name $VmName -ErrorAction SilentlyContinue) { throw "VM '$VmName' already exists." }

        $vhd = Join-Path (Split-Path (Get-VMHost).VirtualHardDiskPath) "$VmName.vhdx"
        Write-Host "Creating VM '$VmName' (Gen2, ${MemoryGB}GB static RAM, $Cpu vCPU, ${VhdSizeGB}GB disk)..."
        New-VM -Name $VmName -Generation 2 -MemoryStartupBytes ($MemoryGB * 1GB) `
            -NewVHDPath $vhd -NewVHDSizeBytes ($VhdSizeGB * 1GB) -SwitchName $SwitchName | Out-Null

        # Nested virtualization (WSL2 = a VM inside the guest) needs static memory
        # and the exposed virtualization extensions; both require the VM powered off.
        Set-VM -Name $VmName -StaticMemory -AutomaticCheckpointsEnabled $false
        Set-VMProcessor -VMName $VmName -Count $Cpu -ExposeVirtualizationExtensions $true
        Set-VMMemory  -VMName $VmName -StartupBytes ($MemoryGB * 1GB)

        # Boot from the install ISO; enable file copy into the guest.
        $dvd = Add-VMDvdDrive -VMName $VmName -Path $IsoPath -Passthru
        Set-VMFirmware -VMName $VmName -FirstBootDevice $dvd
        Enable-VMIntegrationService -VMName $VmName -Name 'Guest Service Interface'

        Start-VM -Name $VmName
        Write-Host "VM started. Now install Windows 11 in the guest and finish OOBE." -ForegroundColor Yellow
        Write-Host "Then verify WSL is ABSENT, and run: .\fullgate.ps1 -Action baseline-checkpoint" -ForegroundColor Yellow
    }

    'baseline-checkpoint' {
        Assert-VmExists
        Write-Host "Verifying the guest baseline (WSL must be ABSENT, not just disabled)..."
        Invoke-Guest {
            $wsl = (Get-Command wsl.exe -ErrorAction SilentlyContinue)
            $vmp = (Get-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform).State
            $lxs = (Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Windows-Subsystem-Linux).State
            [pscustomobject]@{ WslExe = [bool]$wsl; VirtualMachinePlatform = $vmp; WSLFeature = $lxs }
        } | Format-List
        Write-Host "If VirtualMachinePlatform/WSLFeature are 'Disabled' and no distro is registered, this is a clean baseline." -ForegroundColor Yellow
        Checkpoint-VM -Name $VmName -SnapshotName $BaselineSnap
        Write-Host "Checkpoint '$BaselineSnap' created." -ForegroundColor Green
    }

    'prepare' {
        Assert-VmExists
        if (-not $CoworkSetup) { throw "-CoworkSetup <path-to-installer.exe> is required for prepare." }
        if (-not (Test-Path $CoworkSetup)) { throw "Installer not found: $CoworkSetup" }

        Write-Host "Copying '$CoworkSetup' into the guest at '$GuestDest'..."
        $dest = Join-Path $GuestDest (Split-Path $CoworkSetup -Leaf)
        Copy-VMFile -VMName $VmName -SourcePath $CoworkSetup -DestinationPath $dest `
            -FileSource Host -CreateFullPath -Force
        Write-Host "Copied to guest: $dest" -ForegroundColor Green
        Write-Host "Reminder: sign the guest browser OUT of agent accounts so OAuth is a real login." -ForegroundColor Yellow
        Checkpoint-VM -Name $VmName -SnapshotName $Snapshot
        Write-Host "Checkpoint '$Snapshot' created (start each matrix run by reverting to it)." -ForegroundColor Green
    }

    'revert' {
        Assert-VmExists
        Write-Host "Reverting '$VmName' to checkpoint '$Snapshot'..."
        Restore-VMCheckpoint -VMName $VmName -Name $Snapshot -Confirm:$false
        Start-VM -Name $VmName -ErrorAction SilentlyContinue
        Write-Host "Reverted and started. Run the wizard manually now." -ForegroundColor Green
    }

    'induce' {
        Assert-VmExists
        if (-not $Failure) { throw "-Failure <virt-off|virt-on|low-disk|restore-disk|net-drop|net-restore|existing-ubuntu> is required." }
        switch ($Failure) {
            'virt-off' {
                # Expected: preflight.virtualization_disabled / virtualization_unsupported. VM must be Off.
                Stop-VM -Name $VmName -Force -ErrorAction SilentlyContinue
                Set-VMProcessor -VMName $VmName -ExposeVirtualizationExtensions $false
                Write-Host "Nested virtualization DISABLED. Start the VM and run the wizard." -ForegroundColor Green
            }
            'virt-on' {
                Stop-VM -Name $VmName -Force -ErrorAction SilentlyContinue
                Set-VMProcessor -VMName $VmName -ExposeVirtualizationExtensions $true
                Write-Host "Nested virtualization RE-ENABLED." -ForegroundColor Green
            }
            'low-disk' {
                # Expected: preflight.insufficient_disk. Fill C: so free < 16 GiB.
                Invoke-Guest {
                    param($targetFreeGB)
                    $free = (Get-PSDrive C).Free
                    $target = $targetFreeGB * 1GB
                    if ($free -le $target) { "Free already <= ${targetFreeGB}GB ($([math]::Round($free/1GB,1)) GB)"; return }
                    $fill = $free - $target
                    & fsutil file createnew C:\cowork-fill.bin $fill | Out-Null
                    "Created C:\cowork-fill.bin = $([math]::Round($fill/1GB,1)) GB; free now ~${targetFreeGB}GB"
                } -ArgList $TargetFreeGB
            }
            'restore-disk' {
                Invoke-Guest { Remove-Item C:\cowork-fill.bin -Force -ErrorAction SilentlyContinue; "removed fill file" }
            }
            'net-drop' {
                # Expected: common.network_failed (Transient) during install.
                Get-VMNetworkAdapter -VMName $VmName | Disconnect-VMNetworkAdapter
                Write-Host "Guest network DISCONNECTED." -ForegroundColor Green
            }
            'net-restore' {
                Get-VMNetworkAdapter -VMName $VmName | Connect-VMNetworkAdapter -SwitchName $SwitchName
                Write-Host "Guest network RECONNECTED to '$SwitchName'." -ForegroundColor Green
            }
            'existing-ubuntu' {
                # Isolation check: a stock Ubuntu must survive a Cowork install untouched.
                # Requires WSL already enabled in the guest (run after the WSL-enable step,
                # or on a checkpoint where WSL is on).
                Invoke-Guest { & wsl.exe --install -d Ubuntu --no-launch 2>&1; "requested 'Ubuntu' install (verify with: wsl -l -v)" }
            }
        }
    }

    'verify-guest' {
        Assert-VmExists
        Write-Host "Verifying guest state after a wizard run..."
        Invoke-Guest {
            "=== wsl -l -v (Cowork present? a pre-existing Ubuntu still present?) ==="
            & wsl.exe -l -v
            "=== inside the Cowork distro ==="
            & wsl.exe -d Cowork -- bash -lc 'set +e
              echo "workspace:"; test -d "$HOME/workspaces/default" && echo "  OK ~/workspaces/default" || echo "  MISSING"
              echo "toolchain:"; for b in /home/linuxbrew/.linuxbrew/bin/brew "$HOME/.local/bin/mise"; do [ -x "$b" ] && echo "  OK $b" || echo "  MISSING $b"; done
              echo "agents:"; for b in claude codex agy; do p="$HOME/.local/bin/$b"; [ -x "$p" ] && echo "  OK $p" || echo "  (absent) $p"; done
              echo "creds:"; ls -la "$HOME/.cowork/creds" 2>/dev/null || echo "  (none yet)"'
        }
        Write-Host "Review the output above against the runbook's pass criteria (docs/wp10-full-gate.md)." -ForegroundColor Yellow
    }

    'status' {
        $vm = Get-VM -Name $VmName -ErrorAction SilentlyContinue
        if (-not $vm) { Write-Host "VM '$VmName' does not exist."; break }
        $vm | Format-List Name, State, ProcessorCount, MemoryAssigned
        Write-Host "Nested virt:" (Get-VMProcessor -VMName $VmName).ExposeVirtualizationExtensions
        Write-Host "Checkpoints:"
        Get-VMCheckpoint -VMName $VmName | Select-Object Name, CreationTime | Format-Table -AutoSize
    }
}
