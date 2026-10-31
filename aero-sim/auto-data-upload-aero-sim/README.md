# Aero Archive

A Rust terminal application for the dynamics team's simulation review workflow: select files, add tags and notes, review the batch, and publish a version to GitHub. It opens with an animated radar sweep and supports keyboard navigation, mouse selection, and a native file picker.

## Install

Run the script for your operating system from this directory:

```sh
# macOS
./macos.sh

# Linux
./linux.sh
```

```powershell
# Windows PowerShell
.\windows.ps1
```

The scripts build a release executable using `Cargo.lock`, then install only for the current user. Building needs a Rust toolchain (1.85 or later) and the platform's linker/build tools. On macOS, install the Xcode Command Line Tools. On Windows, use the Rust MSVC toolchain with Visual Studio C++ Build Tools. On Linux, install a C compiler and `pkg-config`; native file dialogs use XDG Desktop Portal with a desktop-specific backend (such as `xdg-desktop-portal-gtk`). A terminal emulator is needed for desktop launch on Linux.

If you already have a binary built for the destination OS and CPU, no Rust toolchain is needed on that machine:

```sh
./macos.sh /path/to/aero-archive
./linux.sh /path/to/aero-archive
```

```powershell
.\windows.ps1 -BinaryPath C:\Downloads\aero-archive.exe
```

| OS | Installed application | Launch entry |
| --- | --- | --- |
| macOS | `~/Applications/Aero Archive.app` | Finder Applications / Spotlight; opens Terminal |
| Windows | `%LOCALAPPDATA%\Programs\Aero Archive\aero-archive.exe` | Start menu → Aero Archive |
| Linux | `~/.local/lib/aero-archive/aero-archive` | Applications menu → Aero Archive; also `~/.local/bin/aero-archive` |

Scripts do not request administrator rights, disable Gatekeeper, change PowerShell execution policy, or install Git automatically. If PowerShell is restricted by your organization, use its approved script-signing process. Public macOS distribution should use a signed and notarized application bundle. This repository supplies a source-build installer, not a signed release distribution. There are no published binary downloads configured yet.

## First flight

1. Open Aero Archive. On first boot, its setup check explains missing Git, author identity, or repository settings.
2. Install Git using the OS-specific instructions in the app. Reopen the application after installation, then press **F2** to recheck.
3. Create or select a GitHub repository where you have write access. Authenticate in a separate terminal using [GitHub CLI](https://cli.github.com): `gh auth login`, then `gh auth setup-git`. An SSH key and an SSH repository URL also work. Git's existing credential helper is supported; tokens are never stored by Aero Archive.
4. Press **F3** and enter a URL such as `https://github.com/OU-CHAOS/simulation-data.git`, a project slug, and your commit author name and email. Enter saves. These author details do not change your global Git configuration.
5. Press **F4**, or click the drop box, to select one or more files. Alternatively, paste or drop paths into the terminal. Press Enter if your terminal sends a drop as ordinary keystrokes.
6. Tab to tags and review notes. Add information such as `baseline, mach-2, revision-b` and the solver, flight conditions, or review purpose.
7. Press **F5** to review the repository, destination, size, tags and notes. Enter confirms the upload. Progress and actionable errors appear in the flight log.
8. Press **F7** for successful upload receipts. If a commit succeeds but its push fails, **F6** resumes that batch without importing another copy.

Terminals do not share a universal operating-system file-drop protocol. The app handles bracketed paste, quoted paths, shell-escaped spaces, Windows paths, and local `file://` URIs when the terminal supplies them. The native picker is the reliable fallback. For plain-text drops, focus the path field first; otherwise a terminal's keystrokes can enter the currently focused field. A graphical desktop and working portal are needed for the Linux native picker; typed paths work over SSH or on headless hosts.

| Control | Action |
| --- | --- |
| F1 / F2 / F3 | Help / Git setup check / Settings |
| F4 / click drop box | Native multi-file picker |
| F5 / Enter on review | Review / confirm upload |
| F6 / F7 | Resume pending batch / upload history |
| Tab / Shift+Tab / click field | Move focus |
| Up / Down, then Delete | Select and remove a queued file |
| Backspace / Ctrl+U | Edit / clear a text field |
| PageUp / PageDown | Scroll page and flight-log details |
| Esc / Ctrl+Q | Return home / exit when upload is idle |

Use a terminal at least 66 columns by 24 rows; 100 × 36 is more comfortable. Any key skips the splash. For development:

```sh
cargo run -- --no-splash
cargo run -- --doctor
cargo run -- /path/to/flight.ork /path/to/results.csv
```

## Repository organization and versioning

Each confirmed batch receives a UTC timestamp version (including fractional seconds), its own directory, and a Git commit. Earlier versions remain in place. Versioning uses folders and Git history; it does not create Git tag refs.

```text
sim-data/
  rocket-flight/
    versions/
      20261031T193500.123456789Z/
        manifest.json
        openrocket/
          001-flight.ork
        cfd/
          002-pressure.vtu
        results/
          003-coefficients.csv
        review/
          004-summary.pdf
```

Each manifest records a schema version, batch version, UTC creation time, project, author, notes, sorted tags, and per-file original name, stored relative path, byte count, file category and SHA-256 checksum. Index prefixes prevent collisions between identical or case-only filenames. A batch-level `.gitattributes` disables text normalization, filters, ident substitution and encoding conversion so the committed files preserve their checksums across platforms. Original files are copied, never moved. Absolute source paths and credentials are not included in the manifest.

Automatic tags classify extensions; they do not interpret solver output or infer physical parameters:

| Category | Examples |
| --- | --- |
| `openrocket` | `.ork`, `.rkt` |
| `propulsion` | `.rse`, `.eng` |
| `cfd` | `.foam`, `.cas`, `.msh`, `.cgns`, `.vtk`, `.vtu`, `.vtp`, `.cfx` |
| `geometry` | `.stl`, `.step`, `.stp`, `.iges`, `.igs`, `.obj` |
| `results` | `.csv`, `.tsv`, `.dat`, `.h5`, `.hdf5`, `.mat`, `.sim` |
| `review` | `.png`, `.jpg`, `.jpeg`, `.svg`, `.pdf` |
| `other` | Other regular files |

Directories and symbolic links are rejected. To preserve a multi-file CFD case, archive the case first and upload the archive. Each file is limited to 95 MiB to avoid ordinary GitHub Git object size failures. Git LFS setup, solver integration, live folder watching, automatic repository creation, pull requests and structured extraction of OpenRocket metadata are outside this first implementation.

## Git behavior and recovery

The app creates a dedicated clone in its user data directory. It never commits in your working simulation directory or changes the repository containing this source. Existing repositories follow the default branch at initial clone; empty repositories use Git's configured initial branch. The target branch must accept direct pushes. Repositories requiring pull requests, signed commits or organization-specific authentication need an appropriate repository policy or a later PR publishing mode.

Before import, the app checks for unfinished changes, fetches and fast-forwards. It stages only the generated batch, commits with the configured author, fetches again, rebases its unpublished batch onto concurrent changes, then pushes without force. Generated batch files are explicitly added even if the destination repository's ignore patterns match them. Git hooks and commit signing are disabled for these app-generated commits; this is not a substitute for a repository's required validation pipeline. Batch attributes disable Git filters, including automatic LFS conversion; large-file LFS publishing requires a separate workflow.

A journal beside the checkout records a pending batch before the move/commit/push steps. **F6** retries using the batch's original author and project metadata. New uploads are blocked until the earlier batch finishes. A lock prevents two instances from writing the same managed checkout simultaneously. If a concurrent update arrives after the final fetch, the push can still be rejected; retry fetches and reconciles again. Conflicts stop with a checkout location for manual inspection.

`aero-archive --doctor` prints the exact settings and managed data paths. Typical locations:

| OS | Settings | Managed data |
| --- | --- | --- |
| macOS | `~/Library/Application Support/org.OUChaos.AeroArchive/settings.json` | `~/Library/Application Support/org.OUChaos.AeroArchive/` |
| Windows | `%APPDATA%\OUChaos\AeroArchive\config\settings.json` | `%LOCALAPPDATA%\OUChaos\AeroArchive\data\` |
| Linux | `${XDG_CONFIG_HOME:-~/.config}/aeroarchive/settings.json` | `${XDG_DATA_HOME:-~/.local/share}/aeroarchive/` |

Use the doctor output as authoritative if environment overrides change these locations. The data directory contains `repositories/<remote-url-hash>`, a local `history.log`, and any pending upload journal. If the application crashes, close all instances before removing its stale `.lock` file. Interrupted `.clone` or `.staging` directories are reported explicitly and should be inspected before removal. Do not delete the managed data directory while it contains unpushed work.

Uploads run in a worker thread so the UI can keep repainting. Quit is blocked during an upload. Git runs noninteractively, so complete authentication and SSH host verification outside the TUI first. Git network operations use the host's normal Git transport behavior; a stalled HTTPS operation may need external process termination and stale-lock recovery. SSH connection establishment has a 15-second timeout.

Rerun the installer to upgrade; settings, clones and history are retained. To uninstall, remove the installed application and launcher shown above. On Linux also remove `~/.local/bin/aero-archive` and the `aero-archive.svg` icon under the XDG user icon directory. Removing settings/data is a separate choice after verifying all pending work is published.

## Development and validation

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
bash -n macos.sh linux.sh
```

Application code forbids unsafe Rust with `#![forbid(unsafe_code)]` and a Cargo lint. This applies to this crate; platform libraries and dependencies can contain their own unsafe code. The application does not invoke a shell for user-supplied paths or repository URLs. Only GitHub HTTPS and SSH repository URLs are accepted, with embedded credentials rejected.

`src/ui.rs` owns the terminal lifecycle and screens; `config.rs` handles validated settings and platform directories; `ingest.rs` classifies, copies and hashes files; `git.rs` handles setup diagnostics and the recoverable publishing transaction. The native dialog stays on the main thread, as required on macOS. Terminal state is restored on normal exit, errors and panics.

Tests exercise a temporary local Git remote, initial upload, rejected pushes, retry without duplicate versions, concurrent reviewer commits, source preservation, checksums, filename collisions, large-file rejection, dropped-path formats, and rendering all screens at normal and small terminal sizes. They do not publish anything to GitHub. macOS builds and local workflow tests can run here; Windows/Linux launchers and native dialogs still require testing on their respective desktop hosts before distribution.

Implementation references: [Ratatui 0.29](https://docs.rs/ratatui/0.29.0/ratatui/), [Rusty File Dialogs](https://docs.rs/rfd/0.15.4/rfd/), [Git push](https://git-scm.com/docs/git-push), and the [freedesktop Exec specification](https://specifications.freedesktop.org/desktop-entry-spec/latest/exec-variables.html).
