# Original and redistributable fixtures

Tests generate their own asymmetric PNG pixels and temporary files. Text is
original synthetic content, including combining marks and Arabic test text.

`DejaVuSans.ttf` is the unmodified DejaVu Sans font from Debian 12's
`fonts-dejavu-core` package (2.37). SHA-256:
`abdc775b21b1bc470d50c97e790d276f2054b7504e56e5bd3e64f48d68582322`.
The accompanying `DejaVuSans.LICENSE` is the package's full copyright notice:
Bitstream Vera license with DejaVu changes in the public domain. Its Debian
packaging section does not change the font's license. No system font installation
is required by the preparation tests; retain these notices with the font.
