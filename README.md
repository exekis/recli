# Recli

**Recli** is a fully open-source, **Rust-based CLI tool** that works inside any terminal emulator (Kitty, Alacritty, tmux, etc.), without needing to replace the terminal itself (like Warp or Wave). It will enhance the terminal experience through:

* **Hotkey-activated full context capture**
* **Parsing features** like error summarization and regex parsing
* **AI features** like command flow summarization and suggestions
* **Session impact tracking** to extract what "actually worked" from a messy debug session
* **Passive `journalctl` monitoring** for low-level error capture

This is a **lightweight**, **user-controlled**, **emulator-agnostic** tool that's both powerful for advanced users and helpful for those learning CLI workflows.

---


## Installation
Recli is built in Rust, so you can install it to path using `cargo`. The following drop-in script will clone the repo, build the binary, install it to `~/.cargo/bin`, and then run the help command to verify the installation:

```bash
git clone https://github.com/exekis/recli.git
cd recli
cargo install --path .
recli --help
```

## Cloud Storage Setup (Optional)

Want your command sessions backed up to the cloud? Recli can automatically upload your session logs to Azure Cosmos DB. This is completely optional. Recli works perfectly fine storing everything locally.

### Setting up Azure Cosmos DB

1. **Create a Cosmos DB account** in the Azure portal (free tier works great)
2. **Create a database** called `recli` 
3. **Create a container** called `logs` with partition key `/session_id`
4. **Get your connection string** from the Azure portal (under Keys section)

### Configure your `.env` file

Create a `.env` file in your project directory with:

```bash
RECLI_AZURE__COSMOS__CONNSTR=AccountEndpoint=https://your-account.documents.azure.com:443/;AccountKey=your-key-here==;
RECLI_AZURE__COSMOS__DB=recli
RECLI_AZURE__COSMOS__CONTAINER=logs
```

Just replace `your-account` and `your-key-here` with your actual values from Azure.

### Test the connection

Run `recli cosmos_doctor` to verify everything's working. You should see green checkmarks if it's all set up correctly.

### How it works in practice

Here's what a typical session looks like:

```bash
# start a recli session
$ recli 
[recli] Starting shell session...

# do your normal work
$ echo "hello world"
hello world
$ ls -la
total 48
drwxr-xr-x  6 user user  4096 Sep  8 16:24 .
# ... more commands

# when you're done, just exit
$ exit
Session saved to: /home/user/.recli/logs/20250908_162446/commands.json
✓ Session uploaded to Cosmos DB
```

That's it! Your session gets saved locally (as always) and automatically synced to the cloud. No extra steps and no manual uploads. You can browse your uploaded sessions in the Azure portal or query them programmatically later.

When you exit a recli session, you'll see "✓ Session uploaded to Cosmos DB" and your command history will be safely stored in the cloud.

## **Key Features**

> **Note:** For the latest development progress and implementation status, see [`recli_roadmap.md`](recli_roadmap.md).
<!-- 
### Infrastructure

* CLI interface via `clap` with subcommands (`start`, `stop`, `status`, `recent`, `clear`)
* Start/stop wrapper around a real shell (bash/zsh) using PTY
* Stream stdin/stdout to user

### Hotkey Activation

* Raw mode input handling (`Ctrl+X` etc.)
* On hotkey, pause stream and snapshot recent terminal buffer
* Resume stream after user confirmation

### Contextual Summarization (Non-AI)

* Parse command log to detect:
  * Errors (`error:`, `fatal:`, `panic`)
  * Warnings (`warning:`, `deprecated`, etc.)
  * Exit codes
* Remove noise commands (`ls`, `pwd`, etc.)
* Group related commands into "blocks"
* Display error/warning summary in terminal overlay

### Contextual Summarization (AI)

* Format cleaned context into structured prompt
* Send to model (OpenAI API, Ollama, etc.)
* Display AI-suggested explanation + command flow
* Add "Inject" button to run suggested fix

### Command Impact Tracker

* Log each command + cwd + timestamp + exit code
* Detect net-neutral sequences (e.g., enable + disable)
* Track actual file changes, service state changes
* Export only effective commands to `.sh` or `.md`

### Regex Tooling

* Let user select sample filenames or text
* Generate regex pattern using static rules or ML
* Test pattern locally on buffer
* Show match/highlight results

### Reproducible Script Exporter

* Export command flow into `.sh` script
* Add optional inline comments via AI
* Export markdown version for docs/wiki

### Journalctl Integration

* Background thread monitors `journalctl -f --priority=3`
* Tag log events with timestamps
* Align log messages with command timeline
* Show relevant kernel/service logs in summary
 -->
