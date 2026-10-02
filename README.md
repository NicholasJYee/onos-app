<div align="center">
    <img src="frontend/public/logo.png" width="120" alt="ONOS" />
    <h1>ONOS</h1>
    <p><b>A privacy-first AI clinical scribe that runs entirely on your machine.</b></p>
    <p>
        <img src="https://img.shields.io/badge/License-MIT-blue" alt="License: MIT" />
        <img src="https://img.shields.io/badge/Supported_OS-macOS,_Windows,_iOS-white" alt="Supported OS" />
        <img src="https://img.shields.io/badge/Built_with-Tauri_2-24C8DB" alt="Tauri 2" />
    </p>
</div>

---

ONOS records a clinical encounter, transcribes it, and drafts a structured note, with no audio, transcript, or note ever leaving the device. There is no account, no telemetry requirement, and no cloud dependency. Everything runs locally: speech recognition through Whisper, summarization through a local language model.

It was built for ambient documentation of in-person consults, where sending patient conversations to a third-party service is not an option.

## Contents

- [How it works](#how-it-works)
- [Note templates](#note-templates)
- [Installation](#installation)
- [Building from source](#building-from-source)
- [Where your data lives](#where-your-data-lives)
- [Architecture](#architecture)
- [Credits](#credits)

## How it works

**Transcription** runs locally via [whisper.cpp](https://github.com/ggerganov/whisper.cpp). The default model is **Whisper large-v3-turbo** (~1.5 GB), downloaded once on first launch. Whisper is multilingual, covering **97 languages**, so consultations can be conducted in whatever language the patient speaks. Smaller models (`small`, `medium`, `large-v3-q5_0`) and the faster NVIDIA Parakeet engine are selectable in settings, and the app switches between engines freely.

**Summarization** runs locally too, through a bundled `llama.cpp` sidecar. The default is **Gemma 3 4B** (~2.5 GB), chosen for note quality; the lighter **Gemma 3 1B** remains selectable in settings for low-memory machines. If you'd rather use a hosted model, Ollama, Claude, OpenAI, Groq, OpenRouter, and any OpenAI-compatible endpoint are all supported, but nothing leaves the machine unless you explicitly choose one.

**Language** defaults to English and remembers whatever you last selected. Whisper supports manual language selection across the full ISO-639-1 set; French is wired through to the note templates.

**GPU acceleration** is automatic: Metal on macOS, CUDA or Vulkan on Windows and Linux, with CPU fallback.

## Note templates

Templates live in [`frontend/src-tauri/templates/`](frontend/src-tauri/templates/) as plain JSON. Each defines a set of sections with an instruction and an output format, so adding your own is a matter of copying a file.

| Template | Purpose |
|---|---|
| `geri_consults.json` | Geriatrics consult note: frailty scale, collateral contacts, functional history |
| `consults.json` | General consult note |
| `follow_ups.json` | Follow-up note |
| `*_french.json` | French-language variants of each |

## Installation

Download the latest `.dmg` from [Releases](https://github.com/NicholasJYee/onos-app/releases) and drag ONOS to Applications. Apple Silicon only. macOS builds are signed and notarized, so they open normally.

On first launch the app downloads its models, about 4 GB in total, and caches them locally.

Windows and iOS are not packaged here and [build from source](#building-from-source). Windows builds are currently unsigned and will show a SmartScreen warning. Choose **More info → Run anyway**.

## Building from source

Requires [Rust](https://rustup.rs/), Node 20, [pnpm](https://pnpm.io/), and CMake.

```bash
git clone https://github.com/NicholasJYee/onos-app.git
cd onos-app/frontend
pnpm install
pnpm build:mac      # or: pnpm build:win
```

Artifacts land in `target/<triple>/release/bundle/`.

Both scripts pass an explicit `--target`, so macOS and Windows output never share a directory.

### iOS

Requires macOS with Xcode, a paid Apple Developer account, and a device registered to your team.

```bash
cd frontend
pnpm build:ios              # installable on a registered device
pnpm build:ios:testflight   # App Store build, for TestFlight
```

Set your team once in `frontend/src-tauri/tauri.conf.json` under `bundle.iOS.developmentTeam`. Code signing is enforced on iOS and the build fails without it. Installing directly on a device also requires that device's UDID to be registered with the team, which is why TestFlight is the easier route past one or two phones.

Both land in `frontend/src-tauri/gen/apple/build/arm64/`, and they are not interchangeable. `ONOS.ipa` installs on a registered device but is rejected by App Store Connect. `ONOS-testflight-<build>.ipa` uploads to TestFlight but refuses to sideload.

Do not build the Xcode project on its own. Its build phase calls back into the Tauri CLI, which is not running in that case, so it fails with a connection error. Always build through the pnpm scripts.

<details>
<summary><b>TestFlight credentials</b></summary>

`pnpm build:ios:testflight` signs with an App Store Connect API key. Copy `scripts/ios-signing.env.example` to `scripts/ios-signing.env` (gitignored) and fill in the key id and issuer id from App Store Connect under Users and Access > Integrations. The key needs the **App Manager** role; a Developer-role key cannot create distribution profiles.

Put the `.p8` itself in `~/.appstoreconnect/private_keys/`, outside the repository. The script finds it there automatically, stamps a fresh build number so App Store Connect accepts repeat uploads, and prints the upload command when it finishes.

</details>

<details>
<summary><b>Signing and notarization (macOS)</b></summary>

The signing identity lives in `tauri.conf.json`. Notarization credentials are read from the environment and are never committed:

```bash
export APPLE_API_KEY="<key id>"
export APPLE_API_ISSUER="<issuer id>"
export APPLE_API_KEY_PATH="$HOME/.appstoreconnect/AuthKey_<key id>.p8"
```

With those set, `pnpm build:mac` signs, uploads to Apple, staples the ticket, and packages the `.dmg` in one step. Without them the build still succeeds but skips notarization, producing a `.dmg` that Gatekeeper blocks on other machines.

</details>

<details>
<summary><b>Optional backend</b></summary>

The desktop app is fully standalone. A FastAPI service in [`backend/`](backend/) adds shared meeting storage and server-side summarization for multi-machine setups. See [`backend/README.md`](backend/README.md).

</details>

## Where your data lives

Everything stays on disk, in the clear, under your control.

**macOS**

```
~/Library/Application Support/com.onos.ai/
├── models/                   # Whisper + Gemma weights
├── meeting_minutes.sqlite    # transcripts, notes, settings
└── preferences.json

~/Movies/onos-recordings/     # audio files
```

The recordings folder is configurable. Change it under **Settings → Recording** to store audio on an external drive, an encrypted volume, or anywhere else that suits your setup. Existing recordings stay where they are; the new location applies to subsequent recordings.

**Windows**

```
%APPDATA%\com.onos.ai\
%USERPROFILE%\Music\onos-recordings\
```

On both, deleting the app leaves these in place. Remove them by hand to erase everything.

**iOS**

Everything stays inside the app's private container:

```
Documents/onos-recordings/<meeting>/   audio.m4a, transcripts.json, metadata.json
Library/Application Support/           transcripts, notes, settings, models
```

Recordings are reachable from the **Files** app under **On My iPhone > ONOS**, so they can be played, shared, or copied off the device without a computer.

Unlike macOS and Windows, deleting the app on iOS erases all of it, recordings and notes included. iOS gives apps no storage outside their own container.

## Architecture

```
┌──────────────────────── Desktop app (Tauri 2) ────────────────────────┐
│                                                                       │
│   Next.js UI  ←──IPC──→  Rust core  ──→  Whisper  (transcription)     │
│   (React/TS)             (audio,         llama.cpp (summarization)    │
│                           SQLite)                                      │
└───────────────────────────────────────────────────────────────────────┘
```

Audio capture runs two paths off one pipeline: a mixed stream written to disk, and a VAD-filtered stream sent to Whisper, so only speech is transcribed, cutting inference load substantially.

Deeper documentation: [`docs/architecture.md`](docs/architecture.md), [`docs/BUILDING.md`](docs/BUILDING.md), [`docs/GPU_ACCELERATION.md`](docs/GPU_ACCELERATION.md), and [`CLAUDE.md`](CLAUDE.md) for a codebase tour.

## Contributing

Issues and pull requests are welcome. `CONTRIBUTING.md` has the details.

## Credits

ONOS is built on top of **[Meetily](https://github.com/Zackriya-Solutions/meetily)** by [Zackriya Solutions](https://github.com/Zackriya-Solutions), an open-source, privacy-first meeting assistant. Their work provided the audio pipeline, the local transcription and summarization architecture, and the Tauri application foundation that this project is adapted from. ONOS narrows that general-purpose meeting tool into a clinical documentation workflow.
