# FLM Release Notes

## flm 0.6.0

Welcome to the new rust version of FLM, the Flexible Latex-like Markup language!
This rust-based version is a complete redesign of FLM, which existed earlier as
a [python project](https://github.com/phfaist/flm/tree/main-py).

Skip the boilerplate and type content. FLM lets you typeset documents in a
language that looks like a simplified version of latex directly into outputs
such as HTML and PDF. You can also directly generate self-contained latex code
that latex will parse correctly. [Impatient? Get started with a simple
example.](https://github.com/phfaist/flm#getting-started)

Installation: If you have a rust toolchain set up, you can install flm by typing
`cargo install flm`. Otherwise, binaries are available for download on github's
release page. (Note: macOS won't let you run these binaries because they are not
signed; run `xattr -d com.apple.quarantine flm` to bypass macOS' quarantine.)

## flm <0.6

FLM existed earlier as a python project. You can find it here:
<https://github.com/phfaist/flm/tree/main-py>.
