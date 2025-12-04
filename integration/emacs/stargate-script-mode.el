;;; stargate-script-mode.el --- Major mode for Stargate script files (*.sg) -*- lexical-binding: t; -*-

;; Copyright (C) 2025

;; Author: Dmitry Kalashnikov
;; Keywords: languages, stargate
;; Version: 0.1.0
;; Package-Requires: ((emacs "26.1"))

;; This file is part of the stargate package.

;;; Commentary:

;; This package provides a major mode for editing Stargate script files (.sg).
;; It includes syntax highlighting, indentation, and integration with stargate-shell.
;;
;; Usage:
;;   Files with .sg extension will automatically use this mode.
;;   You can also manually enable it with: M-x stargate-script-mode

;;; Code:

(defgroup stargate-script nil
  "Major mode for editing Stargate script files."
  :group 'languages
  :prefix "stargate-script-")

(defvar stargate-script-mode-syntax-table
  (let ((table (make-syntax-table)))
    ;; Comments
    (modify-syntax-entry ?# "<" table)
    (modify-syntax-entry ?\n ">" table)
    
    ;; Strings
    (modify-syntax-entry ?\" "\"" table)
    (modify-syntax-entry ?' "\"" table)
    
    ;; Operators
    (modify-syntax-entry ?+ "." table)
    (modify-syntax-entry ?- "." table)
    (modify-syntax-entry ?* "." table)
    (modify-syntax-entry ?/ "." table)
    (modify-syntax-entry ?= "." table)
    (modify-syntax-entry ?< "." table)
    (modify-syntax-entry ?> "." table)
    (modify-syntax-entry ?& "." table)
    (modify-syntax-entry ?| "." table)
    (modify-syntax-entry ?! "." table)
    
    ;; Word constituents
    (modify-syntax-entry ?_ "w" table)
    (modify-syntax-entry ?- "w" table)
    
    table)
  "Syntax table for `stargate-script-mode'.")

;; Font-lock keywords
(defvar stargate-script-font-lock-keywords
  `(
    ;; Keywords
    (,(regexp-opt '("let" "if" "else" "while" "for" "fn" "return" 
                    "class" "new" "this" "print" "exec" "script")
                  'words)
     . font-lock-keyword-face)
    
    ;; Built-in constants
    (,(regexp-opt '("true" "false" "null") 'words)
     . font-lock-constant-face)
    
    ;; Function definitions
    ("\\bfn\\s-+\\([a-zA-Z_][a-zA-Z0-9_]*\\)" 1 font-lock-function-name-face)
    
    ;; Class definitions
    ("\\bclass\\s-+\\([A-Z][a-zA-Z0-9_]*\\)" 1 font-lock-type-face)
    
    ;; Variable assignments
    ("\\blet\\s-+\\([a-zA-Z_][a-zA-Z0-9_]*\\)" 1 font-lock-variable-name-face)
    
    ;; String literals
    ("\"[^\"]*\"" . font-lock-string-face)
    ("'[^']*'" . font-lock-string-face)
    
    ;; Numbers
    ("\\b[0-9]+\\(\\.[0-9]+\\)?\\b" . font-lock-constant-face)
    
    ;; Comments
    ("#.*$" . font-lock-comment-face)
    
    ;; Command flags
    ("--[a-z-]+" . font-lock-builtin-face)
    ("-[a-zA-Z]\\b" . font-lock-builtin-face)
    
    ;; Property access
    ("\\.[a-zA-Z_][a-zA-Z0-9_]*" . font-lock-variable-name-face)
    
    ;; Command substitution
    ("\\$([^)]*)" . font-lock-preprocessor-face))
  "Font-lock keywords for Stargate script mode.")

(defun stargate-script-calculate-indentation ()
  "Calculate the indentation level for the current line."
  (save-excursion
    (beginning-of-line)
    (let ((cur-indent 0)
          (opening-brace nil))
      
      ;; If this line starts with }, decrease indent
      (if (looking-at "^[ \t]*}")
          (progn
            ;; Find matching opening brace
            (condition-case nil
                (progn
                  (forward-char)
                  (backward-sexp)
                  (setq cur-indent (current-indentation)))
              (error (setq cur-indent 0))))
        
        ;; Otherwise, base indent on previous non-blank line
        (if (bobp)
            (setq cur-indent 0)
          (forward-line -1)
          (while (and (not (bobp)) (looking-at "^[ \t]*$"))
            (forward-line -1))
          
          ;; Get indentation from previous line
          (setq cur-indent (current-indentation))
          
          ;; If previous line ends with {, increase indent
          (end-of-line)
          (when (re-search-backward "{[ \t]*$" (line-beginning-position) t)
            (setq cur-indent (+ cur-indent 2)))))
      
      cur-indent)))

(defun stargate-script-indent-line ()
  "Indent current line as Stargate script code."
  (interactive)
  (let ((indent-col (stargate-script-calculate-indentation))
        (pos (point)))
    (beginning-of-line)
    (skip-chars-forward " \t")
    (let ((shift-amt (- indent-col (current-column))))
      (unless (zerop shift-amt)
        (delete-region (line-beginning-position) (point))
        (indent-to indent-col))
      ;; Keep cursor position relative to text
      (if (> pos (point))
          (goto-char pos)))))

(defun stargate-script-format-region (start end)
  "Format the region between START and END with proper indentation."
  (interactive "r")
  (save-excursion
    (goto-char start)
    (let ((end-marker (copy-marker end)))
      (while (< (point) end-marker)
        (stargate-script-indent-line)
        (forward-line 1))
      (set-marker end-marker nil))))

(defun stargate-script-format-buffer ()
  "Format the entire buffer with proper indentation."
  (interactive)
  (stargate-script-format-region (point-min) (point-max)))

(defun stargate-script-send-buffer ()
  "Send the entire buffer to stargate-shell."
  (interactive)
  (if (fboundp 'stargate-shell-send-buffer)
      (stargate-shell-send-buffer)
    (message "stargate-shell not loaded. Load stargate-shell.el first.")))

(defun stargate-script-send-region (start end)
  "Send the region between START and END to stargate-shell."
  (interactive "r")
  (if (fboundp 'stargate-shell-send-region)
      (stargate-shell-send-region start end)
    (message "stargate-shell not loaded. Load stargate-shell.el first.")))

(defun stargate-script-run-file ()
  "Run the current .sg file in stargate-shell."
  (interactive)
  (when (buffer-file-name)
    (save-buffer)
    (if (get-buffer "*Stargate Shell*")
        (with-current-buffer "*Stargate Shell*"
          (term-send-raw-string (concat "source " (buffer-file-name) "\n")))
      (message "No active stargate-shell session. Start one with M-x stargate-shell"))))

(defvar stargate-script-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "C-c C-c") 'stargate-script-send-buffer)
    (define-key map (kbd "C-c C-r") 'stargate-script-send-region)
    (define-key map (kbd "C-c C-l") 'stargate-script-run-file)
    (define-key map (kbd "C-c C-f") 'stargate-script-format-region)
    (define-key map (kbd "C-c C-b") 'stargate-script-format-buffer)
    map)
  "Keymap for `stargate-script-mode'.")

;;;###autoload
(define-derived-mode stargate-script-mode prog-mode "Stargate"
  "Major mode for editing Stargate script files.

\\{stargate-script-mode-map}"
  :syntax-table stargate-script-mode-syntax-table
  
  ;; Comments
  (setq-local comment-start "# ")
  (setq-local comment-end "")
  
  ;; Font-lock
  (setq font-lock-defaults '(stargate-script-font-lock-keywords))
  
  ;; Indentation
  (setq-local indent-line-function 'stargate-script-indent-line)
  (setq-local tab-width 2)
  (setq-local indent-tabs-mode nil))

;;;###autoload
(add-to-list 'auto-mode-alist '("\\.sg\\'" . stargate-script-mode))

(provide 'stargate-script-mode)

;;; stargate-script-mode.el ends here
