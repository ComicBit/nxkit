# Security policy

## Supported versions

Only the latest release receives security fixes.

## Report a problem

Do not open a public issue for a security problem. Use the private vulnerability report on the repository page
(Security tab, "Report a vulnerability"). Give these facts:

1. The version and the command that you ran.
2. The input file type that causes the problem. Do not attach game files or keys. Describe how to make a small test file instead.
3. What you saw and what you expected.

We answer in 7 days. We publish a fix and a note in [CHANGELOG.md](CHANGELOG.md) after the fix is available.

## What is in scope

The tools read files that can be damaged or made by an attacker: NSP, NCA, RomFS, NSO, zstd, LZ4, ZIP, plist and Mach-O.
A crash, a hang, an unlimited memory use or a write outside the output folder, caused by such a file, is a security problem.

The trust model and the limits that the code applies are in [docs/security.md](docs/security.md).
