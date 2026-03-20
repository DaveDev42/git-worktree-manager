// Shell completion generation using clap_complete.
//
// Supports bash, zsh, fish, powershell, and elvish.
// Static completions are generated via `gw --generate-completion <shell>`.
//
// Dynamic completions (branch names, preset names, terminal options)
// are provided through the shell function's tab completion hooks.
//
// Usage:
//   # Generate and install completions
//   gw --generate-completion bash > ~/.local/share/bash-completion/completions/gw
//   gw --generate-completion zsh > ~/.zfunc/_gw
//   gw --generate-completion fish > ~/.config/fish/completions/gw.fish
