/// Shell function generation for gw-cd.
///
/// Outputs shell-specific function definitions for bash/zsh/fish/powershell.
/// Generate shell function for the specified shell.
pub fn generate(shell: &str) -> Option<String> {
    match shell {
        "bash" | "zsh" => Some(BASH_ZSH_FUNCTION.to_string()),
        "fish" => Some(FISH_FUNCTION.to_string()),
        "powershell" | "pwsh" => Some(POWERSHELL_FUNCTION.to_string()),
        _ => None,
    }
}

const BASH_ZSH_FUNCTION: &str = r#"# git-worktree-manager shell functions for bash/zsh
# Source this file to enable shell functions:
#   source <(gw _shell-function bash)

# Navigate to a worktree by branch name or worktree name.
# If no argument is provided, show interactive worktree selector.
gw-cd() {
    local target=""

    # Parse arguments
    while [ $# -gt 0 ]; do
        case "$1" in
            -*)
                echo "Error: Unknown option '$1'" >&2
                echo "Usage: gw-cd [branch|worktree-name]" >&2
                return 1
                ;;
            *)
                target="$1"
                shift
                ;;
        esac
    done

    local worktree_path

    if [ -z "$target" ]; then
        # No argument — interactive selector
        worktree_path=$(gw _path --interactive)
        if [ $? -ne 0 ]; then return 1; fi
    else
        # Resolve via git worktree list (branch name or worktree name)
        worktree_path=$(git worktree list --porcelain 2>/dev/null | awk -v t="$target" '
            /^worktree / { path=$2; name=path; sub(".*/", "", name) }
            /^branch / && $2 == "refs/heads/"t { print path; exit }
            /^worktree / && name == t { found_path=path }
            END { if (found_path != "") print found_path }
        ')
    fi

    if [ -z "$worktree_path" ]; then
        echo "Error: No worktree found for '$target'" >&2
        return 1
    fi

    if [ -d "$worktree_path" ]; then
        cd "$worktree_path" || return 1
        echo "Switched to worktree: $worktree_path"
    else
        echo "Error: Worktree directory not found: $worktree_path" >&2
        return 1
    fi
}

# Tab completion for gw-cd (bash)
_gw_cd_completion() {
    local cur="${COMP_WORDS[COMP_CWORD]}"

    local targets
    targets=$(gw _complete-targets 2>/dev/null)
    COMPREPLY=($(compgen -W "$targets" -- "$cur"))
}

# Register completion for bash
if [ -n "$BASH_VERSION" ]; then
    complete -F _gw_cd_completion gw-cd
    eval "$(gw --generate-completion bash 2>/dev/null || true)"

    # Wrap _gw to add dynamic completion for positional-target subcommands
    _gw_with_config() {
        local cur="${COMP_WORDS[COMP_CWORD]}"
        local subcmd="${COMP_WORDS[1]}"

        if [[ "$cur" != -* ]]; then
            local pos_count=0
            local start_idx=2
            local max_pos=0

            case "$subcmd" in
                rm|resume|spawn|exec)
                    max_pos=1
                    ;;
            esac

            if [[ $max_pos -gt 0 ]]; then
                local i
                for ((i=start_idx; i<COMP_CWORD; i++)); do
                    [[ ${COMP_WORDS[i]} != -* ]] && ((pos_count++))
                done
                if [[ $pos_count -lt $max_pos ]]; then
                    local targets
                    targets=$(gw _complete-targets 2>/dev/null)
                    COMPREPLY=($(compgen -W "$targets" -- "$cur"))
                    return
                fi
            fi
        fi

        _gw "$@"
    }
    complete -F _gw_with_config -o bashdefault -o default gw
fi

# Tab completion for zsh
if [ -n "$ZSH_VERSION" ]; then
    # Register clap completion for the gw CLI inline
    eval "$(gw --generate-completion zsh 2>/dev/null)"

    # Wrap _gw to add dynamic completion (targets)
    _gw_with_config() {
        local subcmd="${words[2]}"

        if [[ "${words[CURRENT]}" != -* ]]; then
            local -i pos_count=0
            local -i start_idx=3
            local -i max_pos=0

            case "$subcmd" in
                rm|resume|spawn|exec)
                    max_pos=1
                    ;;
            esac

            if [[ $max_pos -gt 0 ]]; then
                local -i i
                for ((i=start_idx; i<CURRENT; i++)); do
                    [[ ${words[i]} != -* ]] && ((pos_count++))
                done
                if [[ $pos_count -lt $max_pos ]]; then
                    local -a targets
                    targets=(${(f)"$(gw _complete-targets 2>/dev/null)"})
                    compadd -a targets
                    return
                fi
            fi
        fi

        _gw "$@"
    }
    compdef _gw_with_config gw

    _gw_cd_zsh() {
        local -a targets
        targets=(${(f)"$(gw _complete-targets 2>/dev/null)"})
        compadd -a targets
    }
    compdef _gw_cd_zsh gw-cd
fi
"#;

const FISH_FUNCTION: &str = r#"# git-worktree-manager shell functions for fish
# Source this file to enable shell functions:
#   gw _shell-function fish | source

# Navigate to a worktree by branch name or worktree name.
# If no argument is provided, show interactive worktree selector.
function gw-cd
    set -l target ""

    # Parse arguments
    for arg in $argv
        switch $arg
            case '-*'
                echo "Error: Unknown option '$arg'" >&2
                echo "Usage: gw-cd [branch|worktree-name]" >&2
                return 1
            case '*'
                set target $arg
        end
    end

    set -l worktree_path

    if test -z "$target"
        # No argument — interactive selector
        set worktree_path (gw _path --interactive)
        if test $status -ne 0
            return 1
        end
    else
        # Resolve via git worktree list (branch name or worktree name)
        set worktree_path (git worktree list --porcelain 2>/dev/null | awk -v t="$target" '
            /^worktree / { path=$2; name=path; sub(".*/", "", name) }
            /^branch / && $2 == "refs/heads/"t { print path; exit }
            /^worktree / && name == t { found_path=path }
            END { if (found_path != "") print found_path }
        ')
    end

    if test -z "$worktree_path"
        echo "Error: No worktree found for '$target'" >&2
        return 1
    end

    if test -d "$worktree_path"
        cd "$worktree_path"; or return 1
        echo "Switched to worktree: $worktree_path"
    else
        echo "Error: Worktree directory not found: $worktree_path" >&2
        return 1
    end
end

# Tab completion for gw-cd
complete -c gw-cd -f -a '(gw _complete-targets 2>/dev/null)'

# Tab completion for the gw CLI (clap-generated)
gw --generate-completion fish 2>/dev/null | source

# Target completion for subcommands with positional target args
for cmd in rm resume spawn exec
    complete -c gw -f -n "__fish_seen_subcommand_from $cmd" -a '(gw _complete-targets 2>/dev/null)'
end

"#;

const POWERSHELL_FUNCTION: &str = r#"# git-worktree-manager shell functions for PowerShell
# Source this file to enable shell functions:
#   gw _shell-function powershell | Out-String | Invoke-Expression

# Navigate to a worktree by branch name or worktree name.
# If no argument is provided, show interactive worktree selector.
function gw-cd {
    param(
        [Parameter(Mandatory=$false, Position=0)]
        [string]$Target
    )

    $worktreePath = $null

    if (-not $Target) {
        # No argument — interactive selector
        $worktreePath = gw _path --interactive
        if ($LASTEXITCODE -ne 0) {
            return
        }
    } else {
        # Resolve via git worktree list (branch name or worktree name)
        $lines = git worktree list --porcelain 2>&1 | Where-Object { $_ -is [string] }
        $currentPath = $null
        foreach ($line in $lines) {
            if ($line -match '^worktree (.+)$') {
                $currentPath = $Matches[1]
            } elseif ($line -match "^branch refs/heads/$([regex]::Escape($Target))$") {
                $worktreePath = $currentPath
                break
            }
        }
        if (-not $worktreePath) {
            # Try matching by worktree directory name
            foreach ($line in $lines) {
                if ($line -match '^worktree (.+)$') {
                    $p = $Matches[1]
                    if ([System.IO.Path]::GetFileName($p) -eq $Target) {
                        $worktreePath = $p
                        break
                    }
                }
            }
        }
    }

    if (-not $worktreePath) {
        Write-Error "Error: No worktree found for '$Target'"
        return
    }

    if (Test-Path -Path $worktreePath -PathType Container) {
        Set-Location -Path $worktreePath
        Write-Host "Switched to worktree: $worktreePath"
    } else {
        Write-Error "Error: Worktree directory not found: $worktreePath"
        return
    }
}

# Tab completion for gw-cd
Register-ArgumentCompleter -CommandName gw-cd -ParameterName Target -ScriptBlock {
    param($commandName, $parameterName, $wordToComplete, $commandAst, $fakeBoundParameters)

    $targets = gw _complete-targets 2>&1 |
        Where-Object { $_ -is [string] -and $_.Trim() } |
        Sort-Object -Unique

    $targets | Where-Object { $_ -like "$wordToComplete*" } |
        ForEach-Object {
            [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
        }
}

# Native target completion for `gw` subcommands: rm, resume, spawn, exec
Register-ArgumentCompleter -CommandName gw -Native -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    # Find the subcommand (first non-flag element after the command name)
    $elements = $commandAst.CommandElements
    if ($elements.Count -lt 2) { return }
    $subcmd = $elements[1].Value

    # Only complete positional targets for these subcommands
    if ($subcmd -notin @('rm', 'resume', 'spawn', 'exec')) { return }

    # Get completion targets from gw
    $targets = & gw _complete-targets 2>$null

    $targets | Where-Object { $_ -like "$wordToComplete*" } |
        ForEach-Object {
            [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
        }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bash() {
        let result = generate("bash");
        assert!(result.is_some());
        let script = result.unwrap();
        assert!(script.contains("gw-cd()"));
        assert!(script.contains("_gw_cd_completion"));
        assert!(script.contains("BASH_VERSION"));
        assert!(script.contains("ZSH_VERSION"));
        assert!(script.contains("_gw_cd_zsh"));
        assert!(!script.contains("cw-cd"));
        assert!(!script.contains("complete -F _gw_with_config -o bashdefault -o default cw"));
    }

    #[test]
    fn test_generate_zsh() {
        let result = generate("zsh");
        assert!(result.is_some());
        let script = result.unwrap();
        assert!(script.contains("compdef _gw_cd_zsh gw-cd"));
        assert!(!script.contains("cw-cd"));
        assert!(!script.contains("compdef _gw_with_config cw"));
    }

    #[test]
    fn test_generate_fish() {
        let result = generate("fish");
        assert!(result.is_some());
        let script = result.unwrap();
        assert!(script.contains("function gw-cd"));
        assert!(script.contains("complete -c gw-cd"));
        assert!(!script.contains("cw-cd"));
        assert!(!script.contains("complete -c cw "));
    }

    #[test]
    fn test_generate_powershell() {
        let result = generate("powershell");
        assert!(result.is_some());
        let script = result.unwrap();
        assert!(script.contains("function gw-cd"));
        assert!(script.contains("Register-ArgumentCompleter"));
        assert!(!script.contains("cw-cd"));
        assert!(!script.contains("CommandName gw, cw"));
    }

    #[test]
    fn test_powershell_uses_complete_targets() {
        let script = generate("powershell").unwrap();
        assert!(
            script.contains("gw _complete-targets"),
            "PowerShell script should call gw _complete-targets"
        );
        assert!(
            script.contains("Register-ArgumentCompleter -CommandName gw"),
            "PowerShell script should register a completer for `gw`"
        );
    }

    #[test]
    fn test_generate_pwsh_alias() {
        let result = generate("pwsh");
        assert!(result.is_some());
        // pwsh should return the same as powershell
        assert_eq!(result, generate("powershell"));
    }

    #[test]
    fn test_generate_unknown() {
        assert!(generate("unknown").is_none());
        assert!(generate("").is_none());
    }

    /// Verify bash/zsh script uses _complete-targets as the completion source.
    #[test]
    fn test_bash_uses_complete_targets() {
        let script = generate("bash").unwrap();
        assert!(
            script.contains("gw _complete-targets"),
            "bash script should use gw _complete-targets for completions"
        );
        assert!(
            !script.contains("_path --list-branches"),
            "bash script should not reference the removed _path --list-branches"
        );
    }

    /// Verify bash/zsh script has valid syntax using `bash -n`.
    #[test]
    #[cfg(not(windows))]
    fn test_bash_script_syntax() {
        let script = generate("bash").unwrap();

        // bash -n: check syntax without executing
        let output = std::process::Command::new("bash")
            .arg("-n")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.take().unwrap().write_all(script.as_bytes())?;
                child.wait_with_output()
            });

        match output {
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                assert!(
                    out.status.success(),
                    "bash -n failed for generated bash/zsh script:\n{}",
                    stderr
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("bash not found, skipping syntax check");
            }
            Err(e) => panic!("failed to run bash -n: {}", e),
        }
    }

    /// Verify fish script has valid syntax using `fish --no-execute`.
    #[test]
    fn test_fish_script_syntax() {
        let script = generate("fish").unwrap();

        let output = std::process::Command::new("fish")
            .arg("--no-execute")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.take().unwrap().write_all(script.as_bytes())?;
                child.wait_with_output()
            });

        match output {
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                assert!(
                    out.status.success(),
                    "fish --no-execute failed for generated fish script:\n{}",
                    stderr
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("fish not found, skipping syntax check");
            }
            Err(e) => panic!("failed to run fish --no-execute: {}", e),
        }
    }
}
