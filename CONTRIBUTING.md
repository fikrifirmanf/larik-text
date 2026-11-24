# Contributing to Larik Text

Thanks for helping. Larik Text is a small native editor, so the bar for a change is simple: it should make opening and editing one file faster or clearer, and it should not make the app heavier.

## Before you start

- Check the [issues](https://github.com/fikrifirmanf/larik-text/issues) for an existing report or idea.
- For anything larger than a small fix, open an issue first so we can agree on the direction before you spend time on it.
- Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## Set up

You need a current stable Rust toolchain.

```sh
git clone https://github.com/fikrifirmanf/larik-text.git
cd larik-text
cargo run --locked -- path/to/a/file.py
```

Platform requirements (Xcode with the Metal toolchain on macOS, the Windows SDK on Windows, and a few development libraries on Linux) are listed in the [README](README.md#build-from-source).

## Make a change

1. Fork the repository and create a branch from `main`.
2. Keep the change focused. One fix or feature per pull request.
3. Add or update a test when behavior changes. The suite runs with `cargo test --locked`.
4. Before you push, run:

   ```sh
   cargo fmt --check
   cargo clippy --locked --all-targets -- -D warnings
   cargo test --locked
   ```

5. Open a pull request that says what changed and why. For visible changes, include a screenshot of the window, in light and dark if both are affected.

## What a good change looks like

- **Native and light.** No web views, no background services, no new heavy dependencies without a reason.
- **Files stay safe.** Saves must keep a file’s encoding and line endings, and never overwrite changes made on disk without asking.
- **Quiet design.** Follow the existing look: small type, calm colors, one accent. See the [design proposal](docs/design-proposal.md).
- **Plain words.** Interface text and docs should be short and specific.

## Reporting bugs

Open an issue with your operating system, the Larik Text version (**Help → About**), what you did, what you expected, and what happened. If a file triggers it, describe the file type and encoding rather than attaching private content.

## Security

Please do not open a public issue for a security problem. Use the repository’s **Security → Report a vulnerability** page so it can be fixed before it is disclosed.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
