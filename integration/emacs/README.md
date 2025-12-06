# Stargate Shell Emacs Mode

An Emacs major mode for running and interacting with Stargate Shell inside Emacs.

## Installation

### Method 1: Manual Installation

1. Copy `stargate-shell.el` to your Emacs load path (e.g., `~/.emacs.d/lisp/`)

2. Add to your `.emacs` or `~/.emacs.d/init.el`:

```elisp
;; Add the directory containing stargate-shell.el to load-path
(add-to-list 'load-path "~/.emacs.d/lisp/")

;; Load stargate-shell mode
(require 'stargate-shell)

;; Optional: Set custom path to stargate-shell binary
;; (setq stargate-shell-program "/path/to/stargate-shell")

;; Optional: Set custom buffer name
;; (setq stargate-shell-buffer-name "*Stargate*")

;; Optional: Add a global keybinding to launch stargate-shell
(global-set-key (kbd "C-c s") 'stargate-shell)
```

### Method 2: Using use-package (recommended)

```elisp
(use-package stargate-shell
  :load-path "~/.emacs.d/lisp/"
  :commands (stargate-shell stargate-shell-new)
  :bind (("C-c s" . stargate-shell))
  :config
  ;; Optional: customize the stargate-shell program path
  (setq stargate-shell-program "stargate-shell")
  ;; Or use absolute path:
  ;; (setq stargate-shell-program "/path/to/stargate-shell")
  )
```

### Method 3: Installing from source directory

If you have the stargate source code checked out:

```elisp
;; Add stargate source directory to load path
(add-to-list 'load-path "~/src/rust-stargate/")

;; Load stargate-shell mode
(require 'stargate-shell)

;; Set the binary path to your compiled binary
(setq stargate-shell-program "~/src/rust-stargate/target/debug/stargate-shell")

;; Optional keybinding
(global-set-key (kbd "C-c s") 'stargate-shell)
```

## Usage

### Basic Commands

- `M-x stargate-shell` - Start or switch to stargate-shell buffer
- `M-x stargate-shell-new` - Create a new stargate-shell session
- `M-x stargate-shell-send-region` - Send selected region to stargate-shell
- `M-x stargate-shell-send-line` - Send current line to stargate-shell
- `M-x stargate-shell-send-buffer` - Send entire buffer to stargate-shell

### Keybindings (in stargate-shell buffer)

| Key         | Command                      | Description                    |
|-------------|------------------------------|--------------------------------|
| `RET`       | `comint-send-input`          | Send input to shell            |
| `C-c C-c`   | `comint-interrupt-subjob`    | Interrupt (Ctrl-C)             |
| `C-c C-d`   | `comint-send-eof`            | Send EOF (Ctrl-D)              |
| `C-c C-z`   | `comint-stop-subjob`         | Suspend process                |
| `C-c C-l`   | `stargate-shell-clear-buffer`| Clear buffer                   |
| `TAB`       | `completion-at-point`        | Request completion             |
| `M-p`       | `comint-previous-input`      | Previous command in history    |
| `M-n`       | `comint-next-input`          | Next command in history        |
| `C-c C-r`   | `comint-history-isearch-backward-regexp` | Search history |

### Example Workflow

1. Launch stargate-shell:
   ```
   M-x stargate-shell
   ```

2. Type commands as you would in a terminal:
   ```
   stargate > list-directory --long
   stargate > get-hostname
   stargate > let x = 10; print x;
   ```

3. Use history navigation with `M-p` and `M-n`

4. Send code from other buffers:
   - Select region in a script file
   - Run `M-x stargate-shell-send-region`

## Features

- **Syntax Highlighting**: Keywords, strings, numbers, and flags are highlighted
- **ANSI Color Support**: Colors and formatting from stargate-shell are properly displayed
- **Command History**: Navigate through command history with `M-p`/`M-n`
- **Multiple Sessions**: Run multiple stargate-shell instances with `stargate-shell-new`
- **Integration**: Send code from other buffers to the shell
- **Standard Comint Features**: All standard Emacs comint features work (history search, etc.)

## Customization

### Available Custom Variables

```elisp
;; Path to stargate-shell binary
(setq stargate-shell-program "stargate-shell")

;; Arguments to pass on startup (list of strings)
(setq stargate-shell-args nil)

;; Prompt regexp (for navigation)
(setq stargate-shell-prompt-regexp "^stargate > ")

;; Buffer name
(setq stargate-shell-buffer-name "*Stargate Shell*")
```

### Example Custom Configuration

```elisp
(use-package stargate-shell
  :load-path "~/.emacs.d/lisp/"
  :commands (stargate-shell stargate-shell-new)
  :bind (("C-c s" . stargate-shell)
         ("C-c S" . stargate-shell-new))
  :config
  ;; Use release build
  (setq stargate-shell-program "~/.cargo/bin/stargate-shell")
  
  ;; Custom buffer name
  (setq stargate-shell-buffer-name "*Stargate*")
  
  ;; Add hook for additional customization
  (add-hook 'stargate-shell-mode-hook
            (lambda ()
              (setq comint-scroll-to-bottom-on-input t)
              (setq comint-scroll-to-bottom-on-output t)
              (setq comint-move-point-for-output t))))
```

## Troubleshooting

### Binary not found

If you get "stargate-shell: command not found":

```elisp
;; Use absolute path
(setq stargate-shell-program "/full/path/to/stargate-shell")
```

### Colors not showing

ANSI colors should work automatically. If not:

```elisp
(add-hook 'stargate-shell-mode-hook
          (lambda ()
            (ansi-color-for-comint-mode-on)))
```

### History not working

History navigation should work with `M-p` and `M-n`. If not, check:

```elisp
(setq comint-input-ring-size 1000)  ; Increase history size
```

## Comparison with Other Emacs Shell Modes

| Feature              | stargate-shell | shell   | term    | eshell  |
|----------------------|----------------|---------|---------|---------|
| Native integration   | ✓              | ✓       | ✓       | ✓       |
| ANSI colors          | ✓              | ✓       | ✓       | ✓       |
| Emacs keybindings    | ✓              | ✓       | -       | ✓       |
| Stargate-specific    | ✓              | -       | -       | -       |
| Syntax highlighting  | ✓              | -       | -       | ✓       |
| Command history      | ✓              | ✓       | ✓       | ✓       |

## License

This file is part of the stargate package and uses the same license.
