;;; world-txt.el --- Wander world.txt islands from Emacs -*- lexical-binding: t -*-

;; Version: 0.1.0
;; Package-Requires: ((emacs "29.1") (websocket "1.15"))
;; Keywords: games
;; URL: https://github.com/markryall/world.txt

;;; Commentary:

;; A world.txt client in Emacs: one buffer per world, as many worlds as you
;; like, each connected only while you want it to be.
;;
;;   M-x world-txt-join        paste a magic link (or a world's address)
;;   M-x world-txt             pick a world: connect, or jump back into it
;;   M-x world-txt-disconnect  wander off (the buffer stays, to read)
;;
;; Like the terminal client (`world.txt play'), it's thin: the world decides
;; what you see and this just draws it.  It speaks the same JSON over a
;; websocket (src/protocol.rs) and keeps its device tokens in the same place
;; (~/.config/world.txt/tokens), so a world you've joined from the terminal
;; is already joined here.
;;
;; In a world's buffer:
;;
;;   RET        say or do what's typed (or tap the words under point)
;;   TAB        step through what you could do here (S-TAB goes back)
;;   M-p / M-n  earlier and later things you typed
;;   C-c C-p    show or hide the side panel (map, weather, who's about)
;;   C-c C-w    another world
;;   C-c C-r    reconnect
;;   C-c C-d    disconnect
;;
;; Sound comes from `world.txt sound', the terminal client's own speakers
;; in a process of their own, so it's just as in the terminal: `sound on'
;; (and `sound off'), hums and humming circles, radios and gramophones
;; that get louder as you walk up, the island's rain and birdsong, and
;; `play music ~/Music/Rumours' at a gramophone.  It needs world.txt itself
;; installed, and mpv (and ffmpeg, for your own songs).

;;; Code:

(require 'cl-lib)
(require 'subr-x)
(require 'url-parse)
(require 'websocket)

(defgroup world-txt nil
  "Wander world.txt islands."
  :group 'games
  :prefix "world-txt-")

(defcustom world-txt-worlds nil
  "Worlds you can visit, as (NAME . ADDRESS).
ADDRESS is the world's websocket address, e.g. \"wss://world.example.com\".
Worlds you join with `world-txt-join' are remembered in
`world-txt-worlds-file' as well, so you needn't list them here."
  :type '(alist :key-type string :value-type string))

(defcustom world-txt-worlds-file
  (expand-file-name "~/.config/world.txt/emacs-worlds.eld")
  "Where worlds joined with `world-txt-join' are remembered."
  :type 'file)

(defcustom world-txt-max-lines 3000
  "How many lines of a world's log to keep."
  :type 'integer)

(defcustom world-txt-show-panel t
  "Show the side panel when you go into a world."
  :type 'boolean)

(defcustom world-txt-panel-width 36
  "How wide the side panel is."
  :type 'integer)

(defcustom world-txt-program nil
  "The world.txt program, for sound (`world.txt sound').
Nil finds world.txt (or world-txt) on `exec-path'."
  :type '(choice (const :tag "Find it" nil) file))

(defcustom world-txt-sound-on-start nil
  "Start each world with sound on, rather than waiting for `sound on'."
  :type 'boolean)

(defcustom world-txt-prompt "✨ "
  "What sits in front of what you type."
  :type 'string)

;;;; Colours

;; The world names its colours rather than sending codes (src/protocol.rs),
;; so each one is a face you can change.  These match the terminal client.

(defmacro world-txt--colour (name dark light doc)
  `(defface ,(intern (format "world-txt-%s" name))
     '((((background light)) :foreground ,light)
       (t :foreground ,dark))
     ,doc))

(world-txt--colour dim       "#8d849e" "#6b6278" "Quiet words.")
(world-txt--colour white     "#ffffff" "#1e1e28" "Bright words.")
(world-txt--colour blue      "#6aa8ff" "#2f6fc4" "Blue: the sea, the sky.")
(world-txt--colour deep-blue "#3b5bdb" "#2a46b0" "Deep water.")
(world-txt--colour cyan      "#5fe0e0" "#14868a" "People's names.")
(world-txt--colour green     "#7ed87e" "#2e8b3e" "Grass and growing things.")
(world-txt--colour dark-green "#3f9a4f" "#2b6e37" "Forest.")
(world-txt--colour yellow    "#ffd866" "#9a7400" "Things worth noticing.")
(world-txt--colour sand      "#e8cf9a" "#8a6d33" "Sand.")
(world-txt--colour magenta   "#ff79c6" "#b0307e" "Magenta.")
(world-txt--colour pink      "#ffa7d6" "#b04a86" "Pink: music, hints.")
(world-txt--colour red       "#ff6b6b" "#c03030" "Red.")
(world-txt--colour brown     "#c08a5a" "#7d5430" "Wood and earth.")
(world-txt--colour grey      "#a0a0a8" "#686870" "Stone.")

(defface world-txt-tap '((t :underline t))
  "Words you can tap.")

(defface world-txt-prompt '((t :inherit world-txt-magenta :weight bold))
  "The prompt.")

(defun world-txt--face (col bold)
  "The face for the world's colour COL (a string), in BOLD or not."
  (let ((base (unless (member col '(nil "plain"))
                (intern (format "world-txt-%s" (string-replace "_" "-" col))))))
    (cond ((and base bold) (list base 'bold))
          (base base)
          (bold 'bold))))

;;;; Worlds and tokens

(defvar world-txt--remembered 'unread
  "Worlds from `world-txt-worlds-file', read when first needed.")

(defun world-txt--remembered ()
  (when (eq world-txt--remembered 'unread)
    (setq world-txt--remembered
          (when (file-readable-p world-txt-worlds-file)
            (with-temp-buffer
              (insert-file-contents world-txt-worlds-file)
              (ignore-errors (read (current-buffer)))))))
  world-txt--remembered)

(defun world-txt--remember (name address)
  "Keep NAME at ADDRESS in `world-txt-worlds-file'."
  (let ((worlds (cons (cons name address)
                      (cl-remove-if (lambda (w) (equal (car w) name)) (world-txt--remembered)))))
    (setq world-txt--remembered worlds)
    (make-directory (file-name-directory world-txt-worlds-file) t)
    (with-temp-file world-txt-worlds-file
      (insert ";; Worlds joined from Emacs (world-txt.el): (NAME . ADDRESS)\n")
      (pp worlds (current-buffer)))))

(defun world-txt--worlds ()
  "Every world we know of, configured ones first."
  (let ((seen (mapcar #'car world-txt-worlds)))
    (append world-txt-worlds
            (cl-remove-if (lambda (w) (member (car w) seen)) (world-txt--remembered)))))

(defun world-txt--token-file (address)
  "Where the device token for ADDRESS lives: the terminal client's place."
  (expand-file-name
   (concat "~/.config/world.txt/tokens/"
           (replace-regexp-in-string "[^a-z0-9@-]" "_" (downcase (concat "player@" address))))))

(defun world-txt--load-token (address)
  (let ((f (world-txt--token-file address)))
    (when (file-readable-p f)
      (let ((tok (string-trim (with-temp-buffer (insert-file-contents f) (buffer-string)))))
        (unless (string-empty-p tok) tok)))))

(defun world-txt--save-token (address token)
  (let ((f (world-txt--token-file address)))
    (make-directory (file-name-directory f) t)
    (with-temp-file f (insert token))
    (set-file-modes f #o600)))

(defun world-txt--forget-token (address)
  (let ((f (world-txt--token-file address)))
    (when (file-exists-p f) (delete-file f))))

(defun world-txt--parse-link (link)
  "Split a magic LINK (or a bare address) into (ADDRESS . INVITE).
https://… becomes wss://…, http://… becomes ws://…, and INVITE is nil
when there isn't one."
  (let* ((link (string-trim link))
         (invite (when (string-match "invite=\\([^&#[:space:]]+\\)" link)
                   (match-string 1 link)))
         (base (replace-regexp-in-string "/*\\(#.*\\)?\\'" "" link))
         (base (replace-regexp-in-string "\\`https://" "wss://" base))
         (base (replace-regexp-in-string "\\`http://" "ws://" base)))
    (unless (string-match-p "\\`wss?://" base)
      (setq base (concat "wss://" base)))
    (cons base invite)))

(defun world-txt--local-zone ()
  "This computer's timezone, as an IANA name, like the terminal client finds it."
  (let ((tz (string-remove-prefix ":" (or (getenv "TZ") ""))))
    (if (not (string-empty-p tz))
        tz
      (let ((target (file-symlink-p "/etc/localtime")))
        (when (and target (string-match "zoneinfo/\\(.+\\)\\'" target))
          (match-string 1 target))))))

;;;; Per-world state

(cl-defstruct (world-txt-world (:constructor world-txt--make-world))
  name address buffer panel ws
  invite             ; a magic link's code, until it's used
  user nickname
  status hint
  (actions nil) (picked nil)
  (history nil) (hist-pos nil)
  quitting           ; we said goodbye, so don't come back by ourselves
  (retries 0) retry-timer
  told-sound
  helper helper-out  ; `world.txt sound', and what it's half-said
  no-helper          ; why there's no sound, when there isn't
  (unread 0))

(defvar world-txt--all (make-hash-table :test #'equal)
  "Every world we've been to this session, by name.")

(defvar-local world-txt--world nil
  "The world this buffer belongs to.")

(defvar-local world-txt--log-end nil)
(defvar-local world-txt--input-start nil)

(defun world-txt--connected-p (w)
  (and w (world-txt-world-ws w) (websocket-openp (world-txt-world-ws w))))

;;;; Drawing

(defun world-txt--linkify (text face)
  "TEXT in FACE, with its links made tappable.
Links are `[label](https://…)' or a bare https link (src/links.rs)."
  (let ((out "") (start 0))
    (while (string-match "\\[\\([^]\n]+\\)\\](\\(https?://[^)[:space:]]+\\))\\|https?://[^][()<>\"[:space:]]+[^][()<>\"[:space:].,;:!?]"
                         text start)
      (let* ((label (or (match-string 1 text) (match-string 0 text)))
             (url (or (match-string 2 text) (match-string 0 text)))
             (before (substring text start (match-beginning 0))))
        (setq out (concat out (propertize before 'face face)
                          (world-txt--button label face (list :url url) "open in a browser"))
              start (match-end 0))))
    (concat out (propertize (substring text start) 'face face))))

(defun world-txt--button (label face what help)
  "LABEL in FACE as a button that does WHAT (:run, :fill or :url)."
  (make-text-button (copy-sequence label) nil
                    'face (if face (list 'world-txt-tap face) 'world-txt-tap)
                    'world-txt-what what
                    'help-echo help
                    'follow-link t
                    'action #'world-txt--tap))

(defun world-txt--span (span)
  "A span from the world as propertized text."
  (let-alist span
    (let ((face (world-txt--face .c .b)))
      (cond (.run (world-txt--button .t face (list :run .run) (concat "tap: " .run)))
            (.fill (world-txt--button .t face (list :fill .fill) (concat "fill in: " .fill)))
            (t (world-txt--linkify (or .t "") face))))))

(defun world-txt--line (line)
  (mapconcat #'world-txt--span line ""))

(defun world-txt--plain (col text &optional bold)
  "One span of TEXT in COL, as the world would send it."
  `((t . ,text) (c . ,col) ,@(when bold '((b . t)))))

(defun world-txt--say (w &rest spans)
  "Add a line of our own to W's log, made of SPANS."
  (world-txt--add-lines w (list spans)))

(defun world-txt--add-lines (w lines)
  "Add LINES (from the world) to the end of W's log."
  (let ((buf (world-txt-world-buffer w)))
    (when (buffer-live-p buf)
      (with-current-buffer buf
        (let ((inhibit-read-only t)
              (following (cl-loop for win in (get-buffer-window-list buf nil t)
                                  when (>= (window-point win) world-txt--input-start)
                                  collect win)))
          (save-excursion
            (goto-char world-txt--log-end)
            (dolist (l lines)
              (insert (propertize (concat (world-txt--line l) "\n")
                                  'read-only t 'front-sticky '(read-only))))
            (world-txt--trim))
          (dolist (win following)
            (with-selected-window win
              (unless (pos-visible-in-window-p (point-max))
                (recenter -1)))))
        (unless (get-buffer-window buf t)
          (cl-incf (world-txt-world-unread w) (length lines)))))))

(defun world-txt--trim ()
  "Let the oldest lines go once there are more than `world-txt-max-lines'."
  (let ((n (count-lines (point-min) world-txt--log-end)))
    (when (> n (+ world-txt-max-lines 200))
      (goto-char (point-min))
      (forward-line (- n world-txt-max-lines))
      (delete-region (point-min) (point)))))

;;;; The panel

(defun world-txt--panel-buffer (w)
  (let ((buf (world-txt-world-panel w)))
    (unless (buffer-live-p buf)
      (setq buf (get-buffer-create (format "*world.txt: %s panel*" (world-txt-world-name w))))
      (with-current-buffer buf
        (world-txt-panel-mode)
        (setq world-txt--world w))
      (setf (world-txt-world-panel w) buf))
    buf))

(defun world-txt--bar (b)
  (let-alist b
    (let* ((total (max 1 (or .total 1)))
           (filled (/ (* (min .done total) 10) total)))
      (concat (propertize (make-string filled ?▓) 'face 'world-txt-green)
              (propertize (make-string (- 10 filled) ?░) 'face 'world-txt-dim)
              " " .label (if (string-empty-p (or .place "")) "" (concat " · " .place)) " "
              (propertize (if (= .total 100) (format "%d%%" .done) (format "%d/%d" .done .total))
                          'face 'world-txt-dim)))))

(defun world-txt--light (l)
  (let-alist l
    (let ((face (if .up 'world-txt-green 'world-txt-red)))
      (concat (propertize "● " 'face face) .label " "
              (propertize (if .up "up" "down") 'face face)
              (if (and .note (not .up)) (propertize (concat " · " .note) 'face 'world-txt-dim) "")))))

(defun world-txt--draw-panel (w)
  (let ((buf (world-txt--panel-buffer w))
        (st (world-txt-world-status w)))
    (with-current-buffer buf
      (let ((inhibit-read-only t)
            (centre (lambda (s)
                      (let ((pad (/ (max 0 (- world-txt-panel-width 2 (string-width s))) 2)))
                        (concat (make-string pad ? ) s)))))
        (erase-buffer)
        (if (not st)
            (insert (propertize (if (world-txt--connected-p w) "Arriving…" "Not connected.")
                                'face 'world-txt-dim))
          (let-alist st
            (insert (propertize (or .place "") 'face '(world-txt-magenta bold)) "\n"
                    (propertize (or .weather "") 'face 'world-txt-blue) "\n"
                    (propertize (or .clock "") 'face 'world-txt-dim) "\n"
                    (propertize (format-time-string "🕰 %a %-d %b, %-I:%M%P · your time")
                                'face 'world-txt-dim)
                    "\n\n")
            (dolist (l .map) (insert (funcall centre (world-txt--line l)) "\n"))
            (insert "\n")
            (when .here
              (insert (propertize (concat "☺ " (string-join .here ", ")) 'face 'world-txt-cyan) "\n"))
            (dolist (b .progress) (insert (world-txt--bar b) "\n"))
            (dolist (l .lights) (insert (world-txt--light l) "\n"))
            (when (or .progress .lights) (insert "\n"))))
        (when-let* ((hint (world-txt-world-hint w)))
          (let-alist hint
            (when .text
              (insert (if .cmd
                          (world-txt--button .text 'world-txt-pink (list :run .cmd) (concat "try: " .cmd))
                        (propertize .text 'face 'world-txt-pink))
                      "\n\n"))))
        (when-let* ((actions (world-txt-world-actions w)))
          (insert (propertize "You could (TAB):" 'face 'world-txt-dim) "\n")
          (cl-loop for a in actions for i from 0
                   do (let-alist a
                        (insert "  "
                                (world-txt--button
                                 .label
                                 (if (eql i (world-txt-world-picked w))
                                     '(world-txt-magenta bold)
                                   'world-txt-pink)
                                 (if .run (list :run .run) (list :fill .fill))
                                 (or .run .fill))
                                "\n"))))
        (when st
          (let-alist st
            (when (> (or .humans 0) 0)
              (insert "\n" (propertize (format "%d %s about" .humans (if (= .humans 1) "person" "people"))
                                       'face 'world-txt-dim)))))
        (goto-char (point-min))))))

(defun world-txt--header (w)
  "What goes in W's header line."
  (let ((st (world-txt-world-status w)))
    (concat " "
            (cond ((world-txt--connected-p w)
                   (if st
                       (let-alist st
                         (concat (propertize (or .place "") 'face 'bold) "  "
                                 (or .weather "") "  "
                                 (propertize (or .clock "") 'face 'world-txt-dim)))
                     "arriving…"))
                  ((world-txt-world-retry-timer w) "🌙 coming back…")
                  (t (propertize "not connected · C-c C-r to come back" 'face 'world-txt-dim))))))

(defun world-txt-toggle-panel ()
  "Show or hide this world's side panel."
  (interactive)
  (let* ((w (or world-txt--world (user-error "Not in a world")))
         (buf (world-txt--panel-buffer w))
         (win (get-buffer-window buf)))
    (if win
        (delete-window win)
      (world-txt--show-panel w))))

(defun world-txt--show-panel (w)
  (world-txt--draw-panel w)
  (display-buffer-in-side-window
   (world-txt--panel-buffer w)
   `((side . right) (slot . 0) (window-width . ,world-txt-panel-width)
     (preserve-size . (t . nil))
     (window-parameters . ((no-delete-other-windows . t))))))

;;;; Talking to the world

(defun world-txt--send (w msg)
  "Send MSG (a plist) to W, as JSON."
  (if (world-txt--connected-p w)
      (websocket-send-text (world-txt-world-ws w)
                           (decode-coding-string (json-serialize msg) 'utf-8))
    (world-txt--say w (world-txt--plain "dim" "🌙 Not connected. C-c C-r to come back."))))

(defun world-txt--hello (w back)
  (let* ((invite (world-txt-world-invite w))
         (token (unless invite (world-txt--load-token (world-txt-world-address w)))))
    (list :type "hello" :name "player" :bot :false
          :invite (or invite :null) :token (or token :null)
          :passport :null :back (if back t :false)
          :tz (or (world-txt--local-zone) :null))))

(cl-defun world-txt--connect (w &optional back)
  (when (world-txt--connected-p w)
    (websocket-close (world-txt-world-ws w)))
  (setf (world-txt-world-quitting w) nil)
  (let ((address (world-txt-world-address w)))
    (unless (or (world-txt-world-invite w) (world-txt--load-token address))
      (setf (world-txt-world-quitting w) t)
      (world-txt--say w (world-txt--plain
                         "pink" "💌 world.txt is invite-only: ask an island keeper for a magic link, then M-x world-txt-join."))
      (cl-return-from world-txt--connect))
    (unless back
      (world-txt--say w (world-txt--plain "dim" (format "⛵ Sailing to %s…" address))))
    (world-txt--start-helper w)
    (condition-case err
        (setf (world-txt-world-ws w)
              (websocket-open
               address
               :on-open (lambda (_ws)
                          (setf (world-txt-world-retries w) 0)
                          (world-txt--send w (world-txt--hello w back)))
               :on-message (lambda (_ws frame)
                             (condition-case err
                                 (world-txt--receive w (websocket-frame-text frame))
                               (error (message "world.txt: %s" (error-message-string err)))))
               :on-close (lambda (_ws) (world-txt--closed w))
               :on-error (lambda (_ws _type err)
                           (message "world.txt: %s" (if (stringp err) err (format "%S" err))))))
      (error
       (world-txt--say w (world-txt--plain "red" (format "🌊 Couldn't reach %s: %s"
                                                         address (error-message-string err))))
       (world-txt--closed w)))
    (force-mode-line-update t)))

(defun world-txt--closed (w)
  "W's connection has gone: come back by ourselves unless we meant it."
  (setf (world-txt-world-ws w) nil)
  (if (world-txt-world-quitting w)
      (progn
        (world-txt--stop-helper w)
        (world-txt--say w (world-txt--plain "dim" "👋 Disconnected. C-c C-r to come back.")))
    (let ((delay (nth (min (world-txt-world-retries w) 4) '(2 5 10 30 60))))
      (when (= (world-txt-world-retries w) 0)
        (world-txt--say w (world-txt--plain "dim" "🌙 The connection dropped. Coming back in a moment…")))
      (cl-incf (world-txt-world-retries w))
      (when (timerp (world-txt-world-retry-timer w))
        (cancel-timer (world-txt-world-retry-timer w)))
      (setf (world-txt-world-retry-timer w)
            (run-at-time delay nil
                         (lambda ()
                           (setf (world-txt-world-retry-timer w) nil)
                           (when (and (buffer-live-p (world-txt-world-buffer w))
                                      (not (world-txt-world-quitting w))
                                      (not (world-txt--connected-p w)))
                             (world-txt--connect w t)))))))
  (world-txt--refresh w))

(defun world-txt--refresh (w)
  (when (buffer-live-p (world-txt-world-panel w))
    (world-txt--draw-panel w))
  (force-mode-line-update t))

;;;; Sound

;; The speakers are `world.txt sound' (src/speakers.rs), one for each world
;; you're in.  Everything the world says goes to them as it came, and so
;; does everything typed (`typed <text>').  They answer with `say <line>'
;; to show and `send <json>' for the world, so `sound on' and `play music'
;; never reach the world and everything else comes straight back to send.

(defun world-txt--program ()
  (or world-txt-program (executable-find "world.txt") (executable-find "world-txt")))

(defun world-txt--helper-live-p (w)
  (process-live-p (world-txt-world-helper w)))

(defun world-txt--start-helper (w)
  "Start W's speakers, unless they're going already or can't."
  (unless (or (world-txt--helper-live-p w) (world-txt-world-no-helper w))
    (if-let* ((program (world-txt--program)))
        (setf (world-txt-world-helper-out w) ""
              (world-txt-world-helper w)
              (make-process
               :name (format "world.txt sound: %s" (world-txt-world-name w))
               :command (append (list program "sound") (when world-txt-sound-on-start '("--on")))
               :connection-type 'pipe
               :coding 'utf-8
               :noquery t
               :stderr (get-buffer-create " *world.txt sound*")
               :filter (lambda (_p out) (world-txt--from-helper w out))
               :sentinel (lambda (p _event) (world-txt--helper-gone w p))))
      (setf (world-txt-world-no-helper w)
            "Sound needs world.txt itself (brew install markryall/tap/world-txt), then C-c C-r."))))

(defun world-txt--stop-helper (w)
  (when (world-txt--helper-live-p w)
    (delete-process (world-txt-world-helper w)))
  (setf (world-txt-world-helper w) nil))

(defun world-txt--helper-gone (w p)
  "The speakers stopped: if they fell over, say so rather than keep trying."
  (when (and (eq (process-status p) 'exit) (/= (process-exit-status p) 0)
             (eq p (world-txt-world-helper w)))
    (setf (world-txt-world-helper w) nil
          (world-txt-world-no-helper w)
          "Sound in Emacs needs a newer world.txt, one with world.txt sound (brew upgrade world-txt), then C-c C-r.")))

(defun world-txt--from-helper (w out)
  "Lines from W's speakers: `say' them or `send' them."
  (let* ((all (concat (world-txt-world-helper-out w) out))
         (lines (split-string all "\n")))
    (setf (world-txt-world-helper-out w) (car (last lines)))
    (dolist (line (butlast lines))
      (cond ((string-prefix-p "say " line)
             (world-txt--add-lines
              w (list (json-parse-string (substring line 4) :object-type 'alist :array-type 'list
                                         :null-object nil :false-object nil))))
            ((and (string-prefix-p "send " line) (world-txt--connected-p w))
             (websocket-send-text (world-txt-world-ws w) (substring line 5)))))))

(defun world-txt--note-sound (w text)
  "Say once per visit that there's sound here we can't play, and why."
  (unless (world-txt-world-told-sound w)
    (setf (world-txt-world-told-sound w) t)
    (world-txt--say w (world-txt--plain "pink" (concat text " " (or (world-txt-world-no-helper w) "")))))) 

(defun world-txt--receive (w text)
  "Handle one message, TEXT, from W."
  (when (world-txt--helper-live-p w)
    (process-send-string (world-txt-world-helper w) (concat text "\n")))
  ;; Gramophone music is big, and only the speakers want it.
  (if (string-prefix-p "{\"type\":\"audio\"," text)
      (unless (world-txt--helper-live-p w)
        (world-txt--note-sound w "🎶 There's music playing nearby."))
    (let* ((msg (json-parse-string text :object-type 'alist :array-type 'list
                                   :null-object nil :false-object nil))
           (type (alist-get 'type msg)))
      (let-alist msg
        (pcase type
          ("lines" (world-txt--add-lines w .lines))
          ("status"
           (setf (world-txt-world-status w) msg)
           (world-txt--set-actions w .actions)
           (world-txt--refresh w))
          ("hint"
           (setf (world-txt-world-hint w) (when .text msg))
           (world-txt--refresh w))
          ("board" (world-txt--board w msg))
          ("circle"
           (when .parts
             (world-txt--add-lines w (list (world-txt--circle-line .parts)))))
          ("welcome"
           (when .token (world-txt--save-token (world-txt-world-address w) .token))
           (setf (world-txt-world-invite w) nil
                 (world-txt-world-user w) .user
                 (world-txt-world-nickname w) .nickname)
           (world-txt--refresh w))
          ("bye"
           (setf (world-txt-world-quitting w) t)
           (when (string-match-p "isn't signed in" (or .reason ""))
             (world-txt--forget-token (world-txt-world-address w)))
           (world-txt--say w (world-txt--plain "pink" (or .reason "👋"))))
          ("restarting"
           (setf (world-txt-world-retries w) 1)
           (world-txt--say w (world-txt--plain "dim" "🌙 The world is restarting, just for a moment. Coming back by ourselves…")))
          ("travel"
           (world-txt--add-lines
            w (list (list (world-txt--plain "blue" "⛵ Visiting other worlds happens in a browser: open ")
                          (world-txt--plain "yellow" .url t)))))
          ;; With no speakers, music nearby says why it's quiet, and the
          ;; jukebox gets our turn straight back rather than waiting on it.
          ((guard (world-txt--helper-live-p w)) nil)
          ("radio" (when .url (world-txt--note-sound w "📻 There's a radio playing nearby.")))
          ((or "spin_start" "juke_add")
           (world-txt--say w (world-txt--plain "pink" (concat "🎶 Playing your own songs: " (world-txt-world-no-helper w)))))
          ("juke_turn"
           (world-txt--send w (list :type "track_done" :thing .thing :key .key)))
          (_ nil))))))

(defun world-txt--circle-line (parts)
  "Who's humming in this turn of the circle (protocol::circle_line)."
  (cons (world-txt--plain "pink" "🎶")
        (cl-loop for p in parts for i from 0
                 append (let-alist p
                          (append
                           (list (world-txt--plain "dim" (if (= i 0) " " " · "))
                                 (if .you (world-txt--plain "pink" "you" t) (world-txt--plain "cyan" .name)))
                           (when (> .of 1)
                             (list (world-txt--plain "dim" (format " %d/%d" .section .of)))))))))

(defun world-txt--board (w msg)
  "A noticeboard's notes, drawn into the log (protocol::board_lines)."
  (let-alist msg
    (unless (equal .mode "new")
      (let ((out nil))
        (when (equal .mode "open")
          (push (list (world-txt--plain "yellow" .title t)) out)
          (when .page (push (list (world-txt--plain "dim" (concat "  " .page))) out)))
        (unless .notes
          (push (list (world-txt--plain "dim" "  Nothing here yet. Be the first: post hello!")) out))
        (dolist (n .notes)
          (let-alist n
            (push (list (world-txt--plain nil "  ")
                        (world-txt--plain "cyan" .author t)
                        (world-txt--plain "dim" (format " (%s): " .when))
                        (world-txt--plain "white" .text))
                  out)))
        (push (list (world-txt--plain
                     "dim" (format "  (%s)"
                                   (string-join
                                    (append (when .more '("read more for older notes"))
                                            (when .weeks '("archive for past weeks"))
                                            '("post <message> to add one"))
                                    " · "))))
              out)
        (world-txt--add-lines w (nreverse out))))))

;;;; Typing and tapping

(defun world-txt--input ()
  (buffer-substring-no-properties world-txt--input-start (point-max)))

(defun world-txt--set-input (text)
  (let ((inhibit-read-only t))
    (delete-region world-txt--input-start (point-max))
    (goto-char (point-max))
    (insert text)))

(defun world-txt--do (w text)
  "Say or do TEXT in W: through the speakers, which keep `sound on' and
`play music' for themselves and send the rest on."
  (if (and (world-txt--helper-live-p w) (world-txt--connected-p w))
      (process-send-string (world-txt-world-helper w)
                           (concat "typed " (replace-regexp-in-string "\n" " " text) "\n"))
    (let ((trimmed (downcase (string-trim text))))
      (if (or (member trimmed '("sound" "sound on" "sound off"))
              (string-match-p "\\`\\(?:play\\|wind\\|add\\|queue\\) +\\(?:music\\|songs\\|records\\) +[^ ]" trimmed))
          (world-txt--say w (world-txt--plain "pink" (concat "🔇 " (or (world-txt-world-no-helper w) "No sound just now."))))
        (world-txt--send w (list :type "cmd" :text text))))))

(defun world-txt-send ()
  "Say or do what's typed."
  (interactive)
  (let ((w world-txt--world))
    (if (< (point) world-txt--input-start)
        (if-let* ((b (button-at (point)))) (push-button) (goto-char (point-max)))
      (let ((text (world-txt--input)))
        (unless (string-blank-p text)
          (push text (world-txt-world-history w))
          (setf (world-txt-world-hist-pos w) nil
                (world-txt-world-picked w) nil)
          (world-txt--set-input "")
          (world-txt--do w text))))))

(defun world-txt--tap (button)
  "Tapping some words: run their command, fill it in, or open their link."
  (let* ((what (button-get button 'world-txt-what))
         (w world-txt--world)
         (main (world-txt-world-buffer w)))
    (cond ((plist-get what :url) (browse-url (plist-get what :url)))
          ((plist-get what :run) (world-txt--do w (plist-get what :run)))
          ((plist-get what :fill)
           (pop-to-buffer main)
           (world-txt--set-input (plist-get what :fill))))))

(defun world-txt--set-actions (w actions)
  "New things W could do: keep TAB's place if what it picked is still there."
  (when (world-txt-world-picked w)
    (let ((current (with-current-buffer (world-txt-world-buffer w) (world-txt--input))))
      (setf (world-txt-world-picked w)
            (cl-position-if (lambda (a) (equal (or (alist-get 'run a) (alist-get 'fill a)) current))
                            actions))))
  (setf (world-txt-world-actions w) actions))

(defun world-txt-next-action (&optional back)
  "Put the next thing you could do here in the box, ready for RET.
Only over an empty box, or one TAB filled: never over what you've typed."
  (interactive)
  (let* ((w world-txt--world)
         (actions (world-txt-world-actions w))
         (n (length actions))
         (picked (world-txt-world-picked w)))
    (cond ((< (point) world-txt--input-start) (if back (backward-button 1 t) (forward-button 1 t)))
          ((or (= n 0) (and (null picked) (not (string-empty-p (world-txt--input))))) nil)
          (t (let* ((i (cond ((and (null picked) back) (1- n))
                             ((null picked) 0)
                             (back (mod (1- picked) n))
                             (t (mod (1+ picked) n))))
                    (a (nth i actions)))
               (setf (world-txt-world-picked w) i)
               (world-txt--set-input (or (alist-get 'run a) (alist-get 'fill a) ""))
               (world-txt--refresh w))))))

(defun world-txt-previous-action ()
  "Put the previous thing you could do here in the box."
  (interactive)
  (world-txt-next-action t))

(defun world-txt-history-back ()
  "Something you typed earlier."
  (interactive)
  (let* ((w world-txt--world) (h (world-txt-world-history w)))
    (when h
      (let ((i (min (1- (length h)) (1+ (or (world-txt-world-hist-pos w) -1)))))
        (setf (world-txt-world-hist-pos w) i)
        (world-txt--set-input (nth i h))))))

(defun world-txt-history-forward ()
  "Something you typed later."
  (interactive)
  (let* ((w world-txt--world) (i (world-txt-world-hist-pos w)))
    (when i
      (if (<= i 0)
          (progn (setf (world-txt-world-hist-pos w) nil) (world-txt--set-input ""))
        (setf (world-txt-world-hist-pos w) (1- i))
        (world-txt--set-input (nth (1- i) (world-txt-world-history w)))))))

(defun world-txt-beginning-of-line ()
  "Go to the start of what you're typing (or of the line, in the log)."
  (interactive)
  (if (>= (point) world-txt--input-start)
      (goto-char world-txt--input-start)
    (beginning-of-line)))

;;;; Modes

(defvar-keymap world-txt-mode-map
  "RET" #'world-txt-send
  "TAB" #'world-txt-next-action
  "<backtab>" #'world-txt-previous-action
  "M-p" #'world-txt-history-back
  "M-n" #'world-txt-history-forward
  "C-a" #'world-txt-beginning-of-line
  "C-c C-p" #'world-txt-toggle-panel
  "C-c C-w" #'world-txt
  "C-c C-r" #'world-txt-reconnect
  "C-c C-d" #'world-txt-disconnect)

(define-derived-mode world-txt-mode fundamental-mode "world.txt"
  "Wandering a world.txt island.

\\{world-txt-mode-map}"
  (let ((inhibit-read-only t))
    (insert (propertize world-txt-prompt 'face 'world-txt-prompt
                        'read-only t 'rear-nonsticky t 'front-sticky '(read-only)
                        'field 'prompt)))
  ;; The log grows in front of the prompt, and what's typed goes after it.
  (setq-local world-txt--log-end (copy-marker (point-min) t))
  (setq-local world-txt--input-start (point-max-marker))
  (setq-local header-line-format '(:eval (when world-txt--world (world-txt--header world-txt--world))))
  (setq-local scroll-conservatively 101)
  (setq-local truncate-lines nil)
  (setq-local word-wrap t)
  (add-hook 'kill-buffer-hook #'world-txt--killed nil t))

(define-derived-mode world-txt-panel-mode special-mode "world.txt panel"
  "The side panel for a world.txt island."
  (setq-local truncate-lines t)
  (setq-local mode-line-format nil)
  (setq-local cursor-type nil))

(defun world-txt--killed ()
  "Killing a world's buffer leaves it."
  (when-let* ((w world-txt--world))
    (setf (world-txt-world-quitting w) t)
    (when (timerp (world-txt-world-retry-timer w)) (cancel-timer (world-txt-world-retry-timer w)))
    (when (world-txt--connected-p w) (websocket-close (world-txt-world-ws w)))
    (world-txt--stop-helper w)
    (when (buffer-live-p (world-txt-world-panel w)) (kill-buffer (world-txt-world-panel w)))
    (remhash (world-txt-world-name w) world-txt--all)))

;;;; Commands

(defun world-txt--world-named (name address)
  "The world called NAME at ADDRESS, with its buffer, made if need be."
  (let ((w (gethash name world-txt--all)))
    (unless (and w (buffer-live-p (world-txt-world-buffer w)))
      (setq w (world-txt--make-world :name name :address address))
      (let ((buf (get-buffer-create (format "*world.txt: %s*" name))))
        (with-current-buffer buf
          (world-txt-mode)
          (setq world-txt--world w))
        (setf (world-txt-world-buffer w) buf))
      (puthash name w world-txt--all))
    (setf (world-txt-world-address w) address)
    w))

(defun world-txt--go (w)
  "Into W: its buffer, its panel, and a connection if it hasn't one."
  (setf (world-txt-world-unread w) 0)
  (pop-to-buffer-same-window (world-txt-world-buffer w))
  (goto-char (point-max))
  (when world-txt-show-panel (world-txt--show-panel w))
  (unless (or (world-txt--connected-p w) (world-txt-world-retry-timer w))
    (world-txt--connect w)))

(defun world-txt--annotate (name)
  (let ((w (gethash name world-txt--all)))
    (cond ((world-txt--connected-p w)
           (if (> (world-txt-world-unread w) 0)
               (format "  ● here · %d new" (world-txt-world-unread w))
             "  ● here"))
          (t "  ○"))))

;;;###autoload
(defun world-txt (name)
  "Go into the world called NAME: connect, or jump back into it."
  (interactive
   (let ((worlds (world-txt--worlds)))
     (unless worlds (user-error "No worlds yet: M-x world-txt-join with a magic link"))
     (list (completing-read
            "World: "
            (lambda (str pred action)
              (if (eq action 'metadata)
                  '(metadata (annotation-function . world-txt--annotate))
                (complete-with-action action (mapcar #'car worlds) str pred)))
            nil t nil nil
            ;; The most recently visited connected world that isn't this one.
            (cl-loop for b in (buffer-list)
                     for w = (buffer-local-value 'world-txt--world b)
                     when (and w (eq b (world-txt-world-buffer w)) (not (eq b (current-buffer))))
                     return (world-txt-world-name w))))))
  (let ((address (or (alist-get name (world-txt--worlds) nil nil #'equal)
                     (user-error "No world called %s" name))))
    (world-txt--go (world-txt--world-named name address))))

;;;###autoload
(defun world-txt-join (link name)
  "Join a world from a magic LINK, and call it NAME.
LINK can also be just a world's address, when this computer is already
signed in there (from `world.txt play', say)."
  (interactive
   (let* ((link (read-string "Magic link (or a world's address): "))
          (host (url-host (url-generic-parse-url (car (world-txt--parse-link link))))))
     (list link (read-string (format "Call it (%s): " host) nil nil host))))
  (pcase-let ((`(,address . ,invite) (world-txt--parse-link link)))
    (world-txt--remember name address)
    (let ((w (world-txt--world-named name address)))
      (setf (world-txt-world-invite w) invite)
      (when (and invite (world-txt--connected-p w))
        (world-txt--connect w))
      (world-txt--go w))))

(defun world-txt--pick-connected (prompt)
  (or (and world-txt--world (world-txt--connected-p world-txt--world) world-txt--world)
      (let ((names (cl-loop for w being the hash-values of world-txt--all
                            when (world-txt--connected-p w) collect (world-txt-world-name w))))
        (unless names (user-error "Not in any worlds just now"))
        (gethash (if (cdr names) (completing-read prompt names nil t) (car names)) world-txt--all))))

;;;###autoload
(defun world-txt-disconnect (w)
  "Leave the world W for now.  Its buffer stays, to read back through."
  (interactive (list (world-txt--pick-connected "Leave which world? ")))
  (setf (world-txt-world-quitting w) t)
  (when (timerp (world-txt-world-retry-timer w))
    (cancel-timer (world-txt-world-retry-timer w))
    (setf (world-txt-world-retry-timer w) nil))
  (if (world-txt--connected-p w)
      (websocket-close (world-txt-world-ws w))
    (world-txt--refresh w)))

;;;###autoload
(defun world-txt-disconnect-all ()
  "Leave every world."
  (interactive)
  (maphash (lambda (_ w) (when (world-txt--connected-p w) (world-txt-disconnect w))) world-txt--all))

(defun world-txt-reconnect ()
  "Come back to this world."
  (interactive)
  (let ((w (or world-txt--world (user-error "Not in a world"))))
    (setf (world-txt-world-retries w) 0 (world-txt-world-told-sound w) nil
          (world-txt-world-no-helper w) nil)
    (world-txt--connect w)))

;;;###autoload
(defun world-txt-forget (name)
  "Forget the world called NAME (joined with `world-txt-join').
Its device token stays, so joining again with just its address works."
  (interactive (list (completing-read "Forget which world? " (mapcar #'car (world-txt--remembered)) nil t)))
  (let ((worlds (cl-remove-if (lambda (w) (equal (car w) name)) (world-txt--remembered))))
    (setq world-txt--remembered worlds)
    (with-temp-file world-txt-worlds-file
      (insert ";; Worlds joined from Emacs (world-txt.el): (NAME . ADDRESS)\n")
      (pp worlds (current-buffer)))))

(provide 'world-txt)
;;; world-txt.el ends here
