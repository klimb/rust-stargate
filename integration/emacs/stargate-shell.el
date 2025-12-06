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
(defvar stargate-shell--capturing nil
  "Whether we're currently capturing list-variables output.")
(defvar stargate-shell--capture-buffer ""
  "Buffer for capturing output.")

(defun stargate-shell--output-filter (proc string)
  "Filter function to capture list-variables output.
PROC is the process, STRING is the output."
  (let ((vars-buf (get-buffer stargate-shell--variables-buffer-name)))
    (when (and vars-buf (buffer-live-p vars-buf))
      ;; Check if we see "Name Type Value" header - start capturing
      (when (string-match "Name[ \t]+Type[ \t]+Value" string)
        (setq stargate-shell--capturing t)
        (setq stargate-shell--capture-buffer ""))
      
      ;; If we're capturing, accumulate output
      (when stargate-shell--capturing
        (setq stargate-shell--capture-buffer 
              (concat stargate-shell--capture-buffer string))
        
        ;; Check for prompt to end capture
        (when (string-match "stargate>" stargate-shell--capture-buffer)
          (setq stargate-shell--capturing nil)
          ;; Extract everything from "Name" to "stargate>"
          (when (string-match "Name[ \t]+Type[ \t]+Value\\(.*?\\)stargate>" 
                             stargate-shell--capture-buffer)
            (let ((full-match (match-string 0 stargate-shell--capture-buffer)))
              ;; Remove the trailing "stargate>" prompt
              (when (string-match "\\(.*\\)stargate>" full-match)
                (let ((table (match-string 1 full-match)))
                  (with-current-buffer vars-buf
                    (let ((inhibit-read-only t))
                      (erase-buffer)
                      (insert table)
                      (goto-char (point-min))
                      (read-only-mode 1)))))))
          (setq stargate-shell--capture-buffer ""))))))

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
