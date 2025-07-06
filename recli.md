# Recli Project Roadmap

## **Project Summary**

**Recli** is a fully open-source, **Rust-based CLI tool** that works inside any terminal emulator (Kitty, Alacritty, tmux, etc.), without needing to replace the terminal itself (like Warp or Wave). It will enhance the terminal experience through:

* **Hotkey-activated context capture**
* **Parsing features** like error summarization and regex parsing
* **AI features** like command flow summarization and suggestions
* **Session impact tracking** to extract what "actually worked" from a messy debug session
* **Passive `journalctl` monitoring** for low-level error capture

This is a **lightweight**, **user-controlled**, **emulator-agnostic** tool that's both powerful for advanced users and helpful for those learning CLI workflows.

---

## **Key Features**

### Infrastructure

* [ ] CLI interface via `clap` or `argh`
* [ ] Start/stop wrapper around a real shell (bash/zsh) using PTY
* [ ] Stream stdin/stdout to user

### Hotkey Activation

* [ ] Raw mode input handling (`Ctrl+X` etc.)
* [ ] On hotkey, pause stream and snapshot recent terminal buffer
* [ ] Resume stream after user confirmation

### Contextual Summarization (Non-AI)

* [ ] Parse command log to detect:
  * [ ] Errors (`error:`, `fatal:`, `panic`)
  * [ ] Warnings (`warning:`, `deprecated`, etc.)
  * [ ] Exit codes
* [ ] Remove noise commands (`ls`, `pwd`, etc.)
* [ ] Group related commands into "blocks"
* [ ] Display error/warning summary in terminal overlay

### Contextual Summarization (AI-)

* [ ] Format cleaned context into structured prompt
* [ ] Send to model (OpenAI API, Ollama, etc.)
* [ ] Display AI-suggested explanation + command flow
* [ ] Add "Inject" button to run suggested fix

### Command Impact Tracker

* [ ] Log each command + cwd + timestamp + exit code
* [ ] Detect net-neutral sequences (e.g., enable + disable)
* [ ] Track actual file changes, service state changes
* [ ] Export only effective commands to `.sh` or `.md`

### Regex Tooling

* [ ] Let user select sample filenames or text
* [ ] Generate regex pattern using static rules or ML
* [ ] Test pattern locally on buffer
* [ ] Show match/highlight results

### Reproducible Script Exporter

* [ ] Export command flow into `.sh` script
* [ ] Add optional inline comments via AI
* [ ] Export markdown version for docs/wiki

### Journalctl Integration

* [ ] Background thread monitors `journalctl -f --priority=3`
* [ ] Tag log events with timestamps
* [ ] Align log messages with command timeline
* [ ] Show relevant kernel/service logs in summary

---

## **Project Roadmap (Phased)**

### **Phase 0: Setup**

* [x] Install Rust and `cargo`
* [x] Finish Rustlings (in progress)
* [ ] Create new binary crate: `recli`
* [ ] Add `clap` or `argh` for command-line args

### **Phase 1: MVP**

* [ ] Run basic shell passthrough via PTY
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