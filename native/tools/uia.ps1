<#
.SYNOPSIS
  Drives the native launcher through Windows UI Automation (the same tree a
  screen reader uses). Actions: list, invoke <name>, toggle <name>, focus <name>.
  Only the window of the given exact process id is touched.
#>
param(
    [Parameter(Mandatory)] [int] $ProcessId,
    [Parameter(Mandatory)] [ValidateSet('list', 'invoke', 'toggle', 'focus', 'value')] [string] $Action,
    [string] $Name,
    [string] $Value,
    [int] $Index = 0
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$cond = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $ProcessId)
$window = $A::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
if (-not $window) { throw "no UIA window for pid $ProcessId" }
$all = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)

if ($Action -eq 'list') {
    foreach ($e in $all) {
        $c = $e.Current
        if ($c.Name -or $c.ControlType.ProgrammaticName -ne 'ControlType.Pane') {
            "{0,-28} {1,-40} enabled={2} focus={3}" -f $c.ControlType.ProgrammaticName.Replace('ControlType.', ''), $c.Name, $c.IsEnabled, $c.HasKeyboardFocus
        }
    }
    return
}

$matches = @($all | Where-Object { $_.Current.Name -eq $Name })
if ($matches.Count -le $Index) { throw "no element named '$Name' (found $($matches.Count))" }
$target = $matches[$Index]
switch ($Action) {
    'invoke' {
        $pattern = $target.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $pattern.Invoke(); "invoked '$Name'"
    }
    'toggle' {
        $pattern = $target.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
        $pattern.Toggle(); "toggled '$Name' -> $($pattern.Current.ToggleState)"
    }
    'focus' { $target.SetFocus(); "focused '$Name'" }
    'value' {
        $pattern = $target.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        $pattern.SetValue($Value); "set '$Name' = '$Value'"
    }
}
