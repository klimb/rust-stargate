;;; stargate-shell.el --- Major mode for interacting with Stargate Shell -*- lexical-binding: t; -*-

;; Copyright (C) 2025
;; Author: Dmitry Kalashnikov
;; Keywords: processes, terminals, shells
;; Version: 0.1.0
;; Package-Requires: ((emacs "26.1"))

;;; Commentary:
;; Provides an interactive shell for stargate-shell with live variable monitoring.
;; Type 'list-variables' in the shell to update the variables panel.

;;; Code:

(require 'term)

(defgroup stargate-shell nil
  "Major mode for interacting with Stargate Shell."
  :group 'processes
  :prefix "stargate-shell-")

(defcustom stargate-shell-program "stargate-shell"
  "Program to run for stargate-shell."
  :type 'string
  :group 'stargate-shell)

(defvar stargate-shell--variables-buffer-name "*Stargate Variables*")
(defvar stargate-shell--last-output ""
  "Buffer for accumulating terminal output.")

(defun stargate-shell--output-filter (proc string)
  "Filter function to capture all command output.
PROC is the process, STRING is the output."
  (condition-case err
      (let ((vars-buf (get-buffer stargate-shell--variables-buffer-name)))
        (when (and vars-buf (buffer-live-p vars-buf))
          ;; Accumulate all output
          (setq stargate-shell--last-output 
                (concat stargate-shell--last-output string))
          
          ;; When we see a prompt, we have complete command output
          (when (string-match "stargate>" string)
            ;; Extract recent output (simple approach: last 5000 chars before this prompt)
            (let* ((recent-output (if (> (length stargate-shell--last-output) 5000)
                                     (substring stargate-shell--last-output -5000)
                                   stargate-shell--last-output))
                   ;; Find where the last command started (after previous prompt)
                   (last-newline (or (string-match "\n[^\n]*stargate>" recent-output)
                                    0))
                   (output-start (if last-newline (match-end 0) 0))
                   ;; Get everything from after that prompt until now
                   (output (substring recent-output output-start)))
              
              ;; Update variables panel
              (with-current-buffer vars-buf
                (let* ((inhibit-read-only t)
                       ;; Clean up escape sequences and control characters
                       (clean-output output))
                  ;; Remove carriage returns
                  (setq clean-output (replace-regexp-in-string "\r" "" clean-output))
                  ;; Remove ANSI escape sequences (ESC[...m, ESC[...K, etc)
                  (setq clean-output (replace-regexp-in-string "\033\\[[0-9;]*[a-zA-Z]" "" clean-output))
                  ;; Remove CSI sequences
                  (setq clean-output (replace-regexp-in-string "\\[\\?[0-9]+[lh]" "" clean-output))
                  ;; Remove other control sequences
                  (setq clean-output (replace-regexp-in-string "\\[[0-9]*[A-Z]" "" clean-output))
                  ;; Remove bare [ at start of lines (leftover from escape sequences)
                  (setq clean-output (replace-regexp-in-string "^\\[ *" "" clean-output))
                  (setq clean-output (replace-regexp-in-string "\n\\[ *" "\n" clean-output))
                  ;; Trim leading whitespace
                  (setq clean-output (replace-regexp-in-string "\\`[ \t\n]+" "" clean-output))
                  ;; Trim trailing whitespace
                  (setq clean-output (replace-regexp-in-string "[ \t\n]+\\'" "" clean-output))
                  
                  (erase-buffer)
                  (insert "=== Last Command Output ===\n")
                  (insert (format "[%d chars]\n" (length clean-output)))
                  (insert "===========================\n\n")
                  (insert clean-output)
                  (insert "\n")
                  ;; Scroll to bottom
                  (goto-char (point-max))
                  (read-only-mode 1)
                  ;; Scroll window to bottom if visible
                  (let ((win (get-buffer-window vars-buf)))
                    (when win
                      (with-selected-window win
                        (goto-char (point-max))))))))
            
            ;; Keep buffer manageable
            (when (> (length stargate-shell--last-output) 10000)
              (setq stargate-shell--last-output 
                    (substring stargate-shell--last-output -5000))))))
    (error
     (message "Error in stargate-shell filter: %S" err))))

;;;###autoload
(defun stargate-shell ()
  "Run stargate-shell with variables panel."
  (interactive)
  (let* ((program (or (and stargate-shell-program
                           (file-exists-p stargate-shell-program)
                           stargate-shell-program)
                      (executable-find "stargate-shell")
                      (let ((root (locate-dominating-file default-directory "Cargo.toml")))
                        (when root
                          (expand-file-name "target/debug/stargate-shell" root)))))
         (existing (get-buffer "*Stargate Shell*")))
    (unless program
      (error "Cannot find stargate-shell"))
    
    (when existing
      (kill-buffer existing))
    (when (get-buffer stargate-shell--variables-buffer-name)
      (kill-buffer stargate-shell--variables-buffer-name))
    
    (let ((term-buf (make-term "stargate-shell" program)))
      (switch-to-buffer term-buf)
      (term-char-mode)
      
      ;; Add output filter to capture list-variables
      (let ((proc (get-buffer-process term-buf)))
        (when proc
          (add-function :before (process-filter proc) 
                       #'stargate-shell--output-filter)))
      
      (let ((shell-win (selected-window))
            (vars-win (split-window-right -40)))
        (select-window vars-win)
        (let ((vars-buf (get-buffer-create stargate-shell--variables-buffer-name)))
          (switch-to-buffer vars-buf)
          (read-only-mode 1)
          (set-window-dedicated-p vars-win t)
          (select-window shell-win)
          (message "Stargate shell started. Type 'list-variables' to see variables."))))))

;;;###autoload
(defun stargate-shell-stop ()
  "Stop stargate-shell."
  (interactive)
  (when (get-buffer "*Stargate Shell*")
    (kill-buffer "*Stargate Shell*"))
  (when (get-buffer stargate-shell--variables-buffer-name)
    (kill-buffer stargate-shell--variables-buffer-name)))

(provide 'stargate-shell)
;;; stargate-shell.el ends here
