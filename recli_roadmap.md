# Recli Roadmap

---

### **Phase 0: Setup**

* [x] Install Rust and `cargo`
* [x] Finish Rustlings (in progress)
* [x] Create new binary crate: `recli`
* [x] Add `clap` or `argh` for command-line args

### **Phase 1: MVP**

* [x] Run basic shell passthrough via PTY
* [ ] Intercept `Ctrl+X` via `crossterm`
* [ ] Print "Hotkey detected!" as test

### **Phase 2: Terminal Buffer & Command Log**

* [ ] Start storing all typed commands and outputs
* [ ] Include cwd and exit code per command
* [ ] Save history to structured log

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