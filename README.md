# Recli

**Recli** is a fully open-source, **Rust-based CLI tool** that works inside any terminal emulator (Kitty, Alacritty, tmux, etc.), without needing to replace the terminal itself (like Warp or Wave). It will enhance the terminal experience through:

* **Hotkey-activated context capture**
* **Parsing features** like error summarization and regex parsing
* **AI features** like command flow summarization and suggestions
* **Session impact tracking** to extract what "actually worked" from a messy debug session
* **Passive `journalctl` monitoring** for low-level error capture

This is a **lightweight**, **user-controlled**, **emulator-agnostic** tool that's both powerful for advanced users and helpful for those learning CLI workflows.

---


## Installation
Recli is built in Rust, so you can install it to path using `cargo`. The following drop-in script will clone the repo, build the binary, install it to `~/.cargo/bin`, and then run the help command to verify the installation:

```bash
git clone https://github.com/<your-username>/recli.git
cd recli
cargo install --path .
recli --help
```

## **Key Features**
### Infrastructure

* [x] CLI interface via `clap` or `argh`
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

### Contextual Summarization (AI)

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

