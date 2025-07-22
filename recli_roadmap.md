# Recli Roadmap

---

### **Phase 0: Setup**

* [x] Install Rust and `cargo`
* [x] Finish Rustlings (in progress)
* [x] Create new binary crate: `recli`
* [x] Add `clap` with subcommands for better control

### **Phase 1: MVP**

* [x] Run basic shell passthrough via PTY
* [x] Intercept `Ctrl+X` via `crossterm`
* [x] Print "Hotkey detected!" as test
* [x] Add CLI subcommands (`start`, `stop`, `status`, `recent`, `clear`)

### **Phase 2: Terminal Buffer & Command Log**

* [x] Create CommandEntry and CommandLog data structures
* [x] Implement command logging methods (`start_command`, `append_output`, `finish_command`)
* [ ] Integrate command detection with PTY output parsing
* [ ] Include cwd and exit code per command (structures ready)
* [ ] Save history to structured log (methods ready)

### **Phase 3: Context Summarizer**

* [ ] On hotkey, scan recent commands and extract:
  * [ ] All `error:` lines
  * [ ] All non-zero exit codes
* [ ] Display interactive summary (e.g., with `ratatui`)

### **Phase 4: Command Impact Tracker**

* [ ] Detect canceling commands
* [ ] Mark files added/removed (via snapshots or `inotify`)
* [ ] Show final list of commands that changed system state

### **Phase 5: Regex Assistant**

* [ ] Add CLI to pass sample lines
* [ ] Suggest regex statically
* [ ] Test regex on scrollback buffer

### **Phase 6: AI Integration**

* [ ] Add optional LLM support (OpenAI, local)
* [ ] Format prompt with context summary
* [ ] Show AI-suggested fix
* [ ] Optional: inject fix into shell stream

### **Phase 7: `journalctl` Log Watcher**

* [ ] Spawn thread running `journalctl -f`
* [ ] Parse key failures, tag timestamps
* [ ] Display relevant logs with command timeline

---