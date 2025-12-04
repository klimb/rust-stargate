;;; Example .emacs configuration for Stargate Shell
;;; 
;;; This file shows how to set up stargate-shell mode in your Emacs configuration.
;;; You can copy this to your ~/.emacs or ~/.emacs.d/init.el

;; Add the directory containing stargate-shell.el to load-path
(add-to-list 'load-path "~/src/rust-stargate/integration/emacs/")

;; Load stargate-shell mode
(require 'stargate-shell)

;; Load stargate-script-mode for .sg files
(require 'stargate-script-mode)

;; Set the path to your stargate-shell binary (debug build)
(setq stargate-shell-program "~/src/rust-stargate/target/debug/stargate-shell")

;; Add a convenient keybinding
(global-set-key (kbd "C-c s") 'stargate-shell)

;;; End of stargate-shell configuration example

