pub const RUFF: &str = concat!(
    "Install Ruff using ONE of these options:\n\n",
    "Project dependency (run in the project directory):\n",
    "  uv add --dev ruff\n\n",
    "Shared user tool (available across projects via PATH):\n",
    "  uv tool install ruff\n\n",
    "Current Python environment:\n",
    "  python -m pip install ruff\n",
    "  Uses the active virtual environment, if any; otherwise that Python's environment.\n\n",
    "fmt-sync searches the project's .venv first, then PATH.\n",
    "Installation guide: https://docs.astral.sh/ruff/installation/"
);

pub const BIOME: &str = concat!(
    "Install Biome using ONE of these options:\n\n",
    "Project dev dependency (recommended; run in the project directory):\n",
    "  npm install --save-dev --save-exact @biomejs/biome\n",
    "  Requires Node.js/npm; installs into the project's node_modules.\n\n",
    "Shared native executable (Node.js not required):\n",
    "  Download Biome for your OS/architecture and add its directory to PATH.\n",
    "  Use the executable name biome.exe on Windows or biome on Linux.\n\n",
    "fmt-sync searches the project's node_modules first, then PATH.\n",
    "Installation guide: https://biomejs.dev/installation/quick-start/"
);

pub const CARGO: &str = concat!(
    "Rust toolchain (shared per user):\n",
    "  Install Rust with rustup and add Cargo's bin directory to PATH.\n",
    "  Installation guide: https://rust-lang.org/tools/install/\n\n",
    "Toolchain components (run in the project directory):\n",
    "  rustup component add rustfmt clippy\n",
    "  Installs into the active toolchain, shared by projects using it.\n",
    "  These are toolchain components, not Cargo.toml dependencies.\n\n",
    "fmt-sync finds cargo on PATH; Cargo/rustup select the project's active toolchain."
);

pub const RUSTFMT: &str = concat!(
    "Rustfmt component (run in the project directory):\n",
    "  rustup component add rustfmt\n\n",
    "Installation scope:\n",
    "  Installs into the active Rust toolchain, shared by projects using it.\n",
    "  If the project pins a toolchain, the component must be installed for it.\n",
    "  This is a toolchain component, not a Cargo.toml dependency.\n\n",
    "fmt-sync runs cargo fmt with the project's active toolchain.\n",
    "Installation guide: https://github.com/rust-lang/rustfmt#quick-start"
);

pub const CLIPPY: &str = concat!(
    "Clippy component (run in the project directory):\n",
    "  rustup component add clippy\n\n",
    "Installation scope:\n",
    "  Installs into the active Rust toolchain, shared by projects using it.\n",
    "  If the project pins a toolchain, the component must be installed for it.\n",
    "  This is a toolchain component, not a Cargo.toml dependency.\n\n",
    "fmt-sync runs cargo clippy with the project's active toolchain.\n",
    "Installation guide: https://doc.rust-lang.org/clippy/installation.html"
);

pub const GO: &str = concat!(
    "Go toolchain (shared across projects):\n",
    "  Install Go using the official instructions for your OS.\n",
    "  Includes gofmt for formatting and go vet for lint; no separate install needed.\n\n",
    "PATH setup:\n",
    "  Add the Go installation's bin directory containing go and gofmt to PATH.\n",
    "  Windows default: C:\\Program Files\\Go\\bin\n",
    "  Linux typical: /usr/local/go/bin\n",
    "  Reopen the terminal after changing PATH.\n\n",
    "fmt-sync finds go and gofmt on PATH; they are not project dependencies.\n",
    "Installation guide: https://go.dev/doc/install"
);

pub const NODE: &str = concat!(
    "Shared Node.js runtime (required by this local Biome installation):\n",
    "  Install Node.js and add its executable directory to PATH.\n",
    "  Official download: https://nodejs.org/en/download\n\n",
    "Project Biome dependency (after installing Node.js/npm):\n",
    "  npm install --save-dev --save-exact @biomejs/biome\n",
    "  Run in the project directory; installs into node_modules.\n\n",
    "fmt-sync prefers local native Biome, which does not require Node.js.\n",
    "The local JavaScript entry point requires node on PATH.\n",
    "Biome installation guide: https://biomejs.dev/installation/quick-start/"
);
