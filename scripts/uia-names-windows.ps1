#Requires -Version 5.1
<#
.SYNOPSIS
Dump the UI Automation tree of a Sela window (M1-16 accessibility checks).

.DESCRIPTION
Attaches to the given window handle (or the main window of a process) with
the .NET UIAutomation client API and prints a JSON document describing every
node: AutomationId, ControlType, Name, IsEnabled, IsOffscreen,
HasKeyboardFocus, bounding rectangle (physical screen pixels) and, when
supported, ToggleState / SelectionItem.IsSelected / ExpandCollapseState.

    -Hwnd <int>        window handle to inspect (preferred)
    -ProcessId <int>   fall back to the process' MainWindowHandle
    -FocusedOnly       dump only the element that currently has keyboard focus
    -OutJson <path>    write the JSON to a file as well as stdout

The GPUI/AccessKit adapter activates lazily on the first UIA query, so no
application flag is needed. Read-only: sends no input, changes no state.
#>
param(
    [Int64]$Hwnd = 0,
    [int]$ProcessId = 0,
    [switch]$FocusedOnly,
    [string]$OutJson = "",
    [int]$MaxNodes = 8000
)

$ErrorActionPreference = "Stop"
# Emit UTF-8 so callers can decode names containing non-ASCII characters
# regardless of the console OEM codepage.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

if ($Hwnd -eq 0) {
    if ($ProcessId -eq 0) {
        throw "pass -Hwnd or -ProcessId"
    }
    $proc = Get-Process -Id $ProcessId -ErrorAction Stop
    $Hwnd = $proc.MainWindowHandle.ToInt64()
    if ($Hwnd -eq 0) {
        throw "process $ProcessId has no main window handle"
    }
}

function Convert-Element($el, [int]$depth, [string]$parent) {
    $c = $el.Current
    $rect = $c.BoundingRectangle
    $node = [ordered]@{
        depth            = $depth
        parent           = $parent
        automationId     = $c.AutomationId
        controlType      = ($c.ControlType.ProgrammaticName -replace "^ControlType\.", "")
        name             = $c.Name
        isEnabled        = $c.IsEnabled
        isOffscreen      = $c.IsOffscreen
        hasKeyboardFocus = $c.HasKeyboardFocus
        x                = [math]::Round($rect.X)
        y                = [math]::Round($rect.Y)
        width            = [math]::Round($rect.Width)
        height           = [math]::Round($rect.Height)
        toggle           = $null
        selected         = $null
        expanded         = $null
    }
    try {
        $p = $el.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
        $node.toggle = "$($p.Current.ToggleState)"
    } catch {}
    try {
        $p = $el.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern)
        $node.selected = [bool]$p.Current.IsSelected
    } catch {}
    try {
        $p = $el.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
        $node.expanded = "$($p.Current.ExpandCollapseState)"
    } catch {}
    return $node
}

$nodes = New-Object System.Collections.Generic.List[object]

if ($FocusedOnly) {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ($null -ne $focused) {
        $nodes.Add((Convert-Element $focused 0 ""))
    }
} else {
    $root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$Hwnd)
    if ($null -eq $root) {
        throw "no automation element for hwnd $Hwnd"
    }
    $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
    # Iterative walk; entries carry (element, depth, parentAutomationId).
    $stack = New-Object System.Collections.Generic.Stack[object]
    $stack.Push([pscustomobject]@{ El = $root; Depth = 0; Parent = "" })
    while ($stack.Count -gt 0 -and $nodes.Count -lt $MaxNodes) {
        $item = $stack.Pop()
        $node = Convert-Element $item.El $item.Depth $item.Parent
        $nodes.Add($node)
        $child = $walker.GetFirstChild($item.El)
        $siblings = New-Object System.Collections.Generic.List[object]
        while ($null -ne $child) {
            $siblings.Add($child)
            $child = $walker.GetNextSibling($child)
        }
        # Push reversed so children come out in visual order.
        for ($i = $siblings.Count - 1; $i -ge 0; $i--) {
            $stack.Push([pscustomobject]@{ El = $siblings[$i]; Depth = $item.Depth + 1; Parent = $node.automationId })
        }
    }
}

$doc = [ordered]@{
    hwnd  = $Hwnd
    count = $nodes.Count
    nodes = $nodes
}
$json = $doc | ConvertTo-Json -Depth 6 -Compress
if ($OutJson -ne "") {
    [System.IO.File]::WriteAllText($OutJson, $json)
}
Write-Output $json
