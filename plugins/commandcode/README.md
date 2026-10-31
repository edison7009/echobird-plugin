# Command Code

Adds [Command Code Desktop](https://commandcode.ai/desktop) to EchoBird: store entry, installation, launch, model switching, account switching and Smart Access.

Requires an EchoBird host with application plugins, and Node.js 22 or newer on the `PATH` for this plugin's service.

- Accounts are Command Code API keys saved in `~/.echobird/commandcode-accounts.json`. They are not encrypted and are limited to the current user where the OS supports it, like Command Code's own `auth.json`.
- A model applied from EchoBird becomes the `echobird` provider in `~/.commandcode/providers.json`. Its key is kept in `~/.echobird/commandcode-model-key`, because Command Code refuses a key written in that file.
- Smart Access offers the chat-completions models of the Provider API. Go plans have no Provider API access, and Claude models need the Messages API, so neither is offered.

The icon belongs to Command Code and identifies the product.
