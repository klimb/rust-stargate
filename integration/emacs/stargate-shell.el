;;; stargate-shell.el --- Major mode for interacting with Stargate Shell -*- lexical-binding: t; -*-

;; Copyright (C) 2025

;; Author: Dmitry Kalashnikov
;; Keywords: processes, terminals, shells
;; Version: 0.1.0
;; Package-Requires: ((emacs "26.1"))

;; This file is part of the stargate package.

;;; Commentary:

;; This package provides a major mode for running and interacting with
;; Stargate Shell inside Emacs.  It's similar to M-x term or M-x shell
;; but specifically designed for stargate-shell with proper integration.
;;
;; Usage:
;;   M-x stargate-shell
;;
;; Key bindings:
;;   RET - Send input to stargate-shell
;;   C-c C-c - Send interrupt (Ctrl-C)
;;   C-c C-d - Send EOF (Ctrl-D)
;;   C-c C-z - Suspend process
;;   C-c C-l - Clear buffer
;;   TAB - Request completion from shell (if supported)

;;; Code:

(require 'term)
(require 'ansi-color)

(defgroup stargate-shell nil
  "Major mode for interacting with Stargate Shell."
  :group 'processes
  :prefix "stargate-shell-")

(defcustom stargate-shell-program "stargate-shell"
  "Program to run for stargate-shell.
This should be the path to the stargate-shell binary."
  :type 'string
  :group 'stargate-shell)

(defcustom stargate-shell-args nil
  "Arguments to pass to stargate-shell on startup."
  :type '(repeat string)
  :group 'stargate-shell)

(defcustom stargate-shell-prompt-regexp "^stargate > "
  "Regular expression to match the stargate-shell prompt."
  :type 'regexp
  :group 'stargate-shell)

(defcustom stargate-shell-buffer-name "*Stargate Shell*"
  "Name of the stargate-shell buffer."
  :type 'string
  :group 'stargate-shell)

(defvar stargate-shell-mode-map
  (let ((map (make-sparse-keymap)))
    (set-keymap-parent map comint-mode-map)
    (define-key map (kbd "C-c C-l") 'stargate-shell-clear-buffer)
    (define-key map (kbd "C-c C-c") 'comint-interrupt-subjob)
    (define-key map (kbd "C-c C-d") 'comint-send-eof)
    (define-key map (kbd "C-c C-z") 'comint-stop-subjob)
    (define-key map (kbd "TAB") 'completion-at-point)
    map)
  "Keymap for `stargate-shell-mode'.")

(defvar stargate-shell-mode-syntax-table
  (let ((table (make-syntax-table)))
    (modify-syntax-entry ?- "w" table)
    (modify-syntax-entry ?_ "w" table)
    table)
  "Syntax table for `stargate-shell-mode'.")

;; Font-lock keywords for stargate-shell
(defvar stargate-shell-font-lock-keywords
  '(("^stargate > " . font-lock-keyword-face)
    ("\\blet\\b" . font-lock-keyword-face)
    ("\\bif\\b" . font-lock-keyword-face)
    ("\\belse\\b" . font-lock-keyword-face)
    ("\\bwhile\\b" . font-lock-keyword-face)
    ("\\bfor\\b" . font-lock-keyword-face)
    ("\\bfn\\b" . font-lock-keyword-face)
    ("\\breturn\\b" . font-lock-keyword-face)
    ("\\bclass\\b" . font-lock-keyword-face)
    ("\\bprint\\b" . font-lock-keyword-face)
    ("\\bexec\\b" . font-lock-keyword-face)
    ("\\bscript\\b" . font-lock-keyword-face)
    ("\\b\\(true\\|false\\|null\\)\\b" . font-lock-constant-face)
    ("\"[^\"]*\"" . font-lock-string-face)
    ("'[^']*'" . font-lock-string-face)
    ("\\b[0-9]+\\(\\.[0-9]+\\)?\\b" . font-lock-constant-face)
    ("--[a-z-]+" . font-lock-builtin-face)
    ("-[a-zA-Z]" . font-lock-builtin-face))
  "Font-lock keywords for stargate-shell mode.")

(defun stargate-shell-clear-buffer ()
  "Clear the stargate-shell buffer."
  (interactive)
  (let ((comint-buffer-maximum-size 0))
    (comint-truncate-buffer)))

(defun stargate-shell-initialize ()
  "Initialize stargate-shell buffer."
  (setq comint-process-echoes nil)
  (setq comint-use-prompt-regexp t)
  (setq comint-prompt-regexp stargate-shell-prompt-regexp)
  (setq comint-prompt-read-only nil)
  
  ;; Enable ANSI colors
  (ansi-color-for-comint-mode-on)
  (add-hook 'comint-output-filter-functions 'ansi-color-process-output nil t)
  
  ;; Set up compilation error pattern matching (optional)
  (setq-local compilation-error-regexp-alist nil)
  
  ;; Enable font-lock
  (setq font-lock-defaults '(stargate-shell-font-lock-keywords t)))

(define-derived-mode stargate-shell-mode comint-mode "Stargate-Shell"
  "Major mode for interacting with Stargate Shell.

This mode provides an interactive shell environment for stargate-shell
within Emacs, with proper syntax highlighting and keybindings.

\\{stargate-shell-mode-map}"
  :syntax-table stargate-shell-mode-syntax-table
  (stargate-shell-initialize))

;;;###autoload
(defun stargate-shell ()
  "Run stargate-shell in a terminal buffer.

If a stargate-shell buffer already exists, switch to it.
Otherwise, create a new one."
  (interactive)
  (let* ((program (or (and stargate-shell-program
                           (file-exists-p stargate-shell-program)
                           stargate-shell-program)
                      (executable-find "stargate-shell")
                      (let ((cargo-root (locate-dominating-file default-directory "Cargo.toml")))
                        (when cargo-root
                          (expand-file-name "target/debug/stargate-shell" cargo-root)))))
         (buffer-name "*Stargate Shell*")
         (existing-buffer (get-buffer buffer-name)))
    (unless program
      (error "Cannot find stargate-shell program. Set `stargate-shell-program' or add to PATH"))
    (if (and existing-buffer
             (buffer-live-p existing-buffer)
             (get-buffer-process existing-buffer))
        (switch-to-buffer existing-buffer)
      (when existing-buffer
        (kill-buffer existing-buffer))
      (let ((term-buffer (make-term "stargate-shell" program)))
        (switch-to-buffer term-buffer)
        (term-char-mode)
        (message "Started stargate-shell")))))

;;;###autoload
(defun stargate-shell-new ()
  "Create a new stargate-shell buffer.

This creates a new stargate-shell session even if one already exists."
  (interactive)
  (let* ((program (or (and stargate-shell-program
                           (file-exists-p stargate-shell-program)
                           stargate-shell-program)
                      (executable-find "stargate-shell")
                      (let ((cargo-root (locate-dominating-file default-directory "Cargo.toml")))
                        (when cargo-root
                          (expand-file-name "target/debug/stargate-shell" cargo-root)))))
         (n 1)
         (buffer-name stargate-shell-buffer-name)
         buffer)
    (unless program
      (error "Cannot find stargate-shell program. Set `stargate-shell-program' or add to PATH"))
    ;; Find a unique buffer name
    (while (get-buffer buffer-name)
      (setq n (1+ n))
      (setq buffer-name (format "*Stargate Shell<%d>*" n)))
    
    (setq buffer (get-buffer-create buffer-name))
    (pop-to-buffer buffer)
    (stargate-shell-mode)
    (apply 'make-comint-in-buffer
           "stargate-shell"
           buffer
           program
           nil
           (or stargate-shell-args '()))
    (message "Started new stargate-shell session")))

;;;###autoload
(defun stargate-shell-send-region (start end)
  "Send the region between START and END to stargate-shell."
  (interactive "r")
  (let ((text (buffer-substring-no-properties start end))
        (proc (get-buffer-process stargate-shell-buffer-name)))
    (if (and proc (process-live-p proc))
        (progn
          (with-current-buffer stargate-shell-buffer-name
            (goto-char (process-mark proc))
            (insert text)
            (comint-send-input))
          (message "Sent region to stargate-shell"))
      (error "No active stargate-shell process"))))

;;;###autoload
(defun stargate-shell-send-buffer ()
  "Send the entire current buffer to stargate-shell."
  (interactive)
  (stargate-shell-send-region (point-min) (point-max)))

;;;###autoload
(defun stargate-shell-send-line ()
  "Send the current line to stargate-shell."
  (interactive)
  (save-excursion
    (let ((start (line-beginning-position))
          (end (line-end-position)))
      (stargate-shell-send-region start end))))

(provide 'stargate-shell)

;;; stargate-shell.el ends here
