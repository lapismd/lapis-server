# Server-owned interactive Bash defaults. This file never changes real paths.
__lapis_terminal_root=${LAPIS_SERVER_TERMINAL_ROOT-}
__lapis_terminal_vault=${LAPIS_SERVER_TERMINAL_VAULT_ID-}
unset LAPIS_SERVER_TERMINAL_ROOT LAPIS_SERVER_TERMINAL_VAULT_ID

# The managed shell remains a login-interactive shell. Load normal startup
# files once before composing the application prompt with the user's hooks.
PROMPT_COMMAND=${LAPIS_SERVER_INHERITED_PROMPT_COMMAND-}
unset LAPIS_SERVER_INHERITED_PROMPT_COMMAND
[[ -r /etc/profile ]] && . /etc/profile
if [[ -n ${HOME-} ]]; then
    if [[ -r $HOME/.bash_profile ]]; then
        . "$HOME/.bash_profile"
    elif [[ -r $HOME/.bash_login ]]; then
        . "$HOME/.bash_login"
    elif [[ -r $HOME/.profile ]]; then
        . "$HOME/.profile"
    elif [[ -r $HOME/.bashrc ]]; then
        . "$HOME/.bashrc"
    fi
fi

# Shell cwd is physical (for example /private/var on macOS); resolve only the
# display comparison, keeping the user's HOME environment and real paths intact.
__lapis_terminal_home=${HOME-}
if [[ -n $__lapis_terminal_home && -d $__lapis_terminal_home ]]; then
    __lapis_terminal_home=$(builtin cd -P -- "$__lapis_terminal_home" && builtin pwd -P)
fi

__lapis_terminal_capture_status() {
    __lapis_terminal_previous_status=$?
    return "$__lapis_terminal_previous_status"
}

__lapis_terminal_render_prompt() {
    __lapis_terminal_prefix=$PWD
    __lapis_terminal_id=
    __lapis_terminal_suffix=
    if [[ -n $__lapis_terminal_vault &&
          $__lapis_terminal_root == */vaults/"$__lapis_terminal_vault"/content &&
          ( $PWD == "$__lapis_terminal_root" || $PWD == "$__lapis_terminal_root/"* ) ]]; then
        __lapis_terminal_prefix=${__lapis_terminal_root%"/$__lapis_terminal_vault/content"}/
        __lapis_terminal_id=${__lapis_terminal_vault:0:8}
        __lapis_terminal_suffix=/content${PWD#"$__lapis_terminal_root"}
    fi
    if [[ -n $__lapis_terminal_home && $__lapis_terminal_prefix == "$__lapis_terminal_home" ]]; then
        __lapis_terminal_prefix='~'
    elif [[ -n $__lapis_terminal_home && $__lapis_terminal_prefix == "$__lapis_terminal_home/"* ]]; then
        __lapis_terminal_prefix="~/${__lapis_terminal_prefix#"$__lapis_terminal_home/"}"
    fi
    __lapis_terminal_prefix=${__lapis_terminal_prefix//[[:cntrl:]]/?}
    __lapis_terminal_suffix=${__lapis_terminal_suffix//[[:cntrl:]]/?}
    # Expand variables once as data. Never put cwd bytes into executable PS1 code.
    shopt -s promptvars
    PS1='\[\e[36m\]${__lapis_terminal_prefix}\[\e[35m\]${__lapis_terminal_id}\[\e[36m\]${__lapis_terminal_suffix} '
    if [[ ${__lapis_terminal_previous_status:-0} == 0 ]]; then
        PS1+='\[\e[32m\]❯\[\e[0m\] '
    else
        PS1+='\[\e[31m\]❯\[\e[0m\] '
    fi
}

# Preserve startup hooks and their order, and colour status using the user command.
if (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )); then
    PROMPT_COMMAND=(__lapis_terminal_capture_status "${PROMPT_COMMAND[@]-}" __lapis_terminal_render_prompt)
else
    PROMPT_COMMAND=$'__lapis_terminal_capture_status\n'"${PROMPT_COMMAND:-:}"$'\n__lapis_terminal_render_prompt'
fi
# The bootstrap arrived through the environment, but its replacement references
# private shell functions. Nested shells must load their own prompt normally.
export -n PROMPT_COMMAND

# GNU ls is present on Server. Keep native development shells usable on macOS.
if command ls --color=auto -d . >/dev/null 2>&1; then
    alias ls='ls --color=auto'
elif command -v gls >/dev/null 2>&1; then
    alias ls='gls --color=auto'
else
    export CLICOLOR=1
    alias ls='ls -G'
fi

__lapis_terminal_previous_status=0
# The bootstrap is itself the first prompt command. Run the composed hooks now
# as Bash will on later prompts. These are trusted startup commands, never cwd data.
if (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )); then
    __lapis_terminal_previous_status=0
    for __lapis_terminal_initial_hook in "${PROMPT_COMMAND[@]}"; do
        builtin eval -- "$__lapis_terminal_initial_hook"
    done
    unset __lapis_terminal_initial_hook
else
    __lapis_terminal_previous_status=0
    builtin eval -- "$PROMPT_COMMAND"
fi
