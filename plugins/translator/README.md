# AI Translate

An installable EchoBird page plugin using the original translation interface and icon. Choose source and target languages, translate with the model selected at launch, and copy the result. Ctrl/Command+Enter translates. Automatic detection, swapping languages and clearing text remain available.

## Install

Requires the unreleased EchoBird development build with the `0.1-draft` page host. Install **AI 翻译 · Translate** from the Hub, or add this repository with `#plugins/translator`, or select this directory locally. Review `model.request` and `clipboard.write` before installation. After enabling the plugin, select its desktop entry and a model, then launch it. The Projects demonstration remains separate.

The page receives the model's display name and final text through `window.echobird.generateText`. Credentials and model endpoints stay in the host. Copy uses the write-only `writeClipboardText` host API; the page cannot read the clipboard. Opening the page makes no model or clipboard call. A translation request is limited by the host's 32 KiB prompt, 1 MiB response and 60-second deadline.

## Build

From the repository root, run `npm run pack:translator`, then `npm run check`. Versioned packages are immutable. Repository CI checks Windows, macOS and Linux; native application verification is separate. The draft runtime is not available in existing released clients.
