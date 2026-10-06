# Original and redistributable fixtures

Tests generate their own asymmetric PNG pixels and temporary files. Text is
original synthetic content, including combining marks and Arabic test text.

`DejaVuSans.ttf` is the unmodified DejaVu Sans font from Debian 12's
`fonts-dejavu-core` package (2.37). SHA-256:
`abdc775b21b1bc470d50c97e790d276f2054b7504e56e5bd3e64f48d68582322`.
The accompanying `DejaVuSans.LICENSE` is the package's full copyright notice:
Bitstream Vera license with DejaVu changes in the public domain. Its Debian
packaging section does not change the font's license. No system font
installation is required by the preparation tests; retain these notices with
the font.

`DejaVuSans-Bold.ttf` is the unmodified DejaVu Sans Bold face from the
official dejavu-fonts 2.37 release tarball
(`dejavu-fonts-ttf-2.37.tar.bz2`, github.com/dejavu-fonts/dejavu-fonts,
release `version_2_37`); the Debian pool no longer serves the matching
source tarball. SHA-256:
`e6476c1b80502924294eed40894c5b18e06c181444ca953e5334262df9c27724`.
The same Bitstream Vera license (with DejaVu changes in the public domain)
covers it; keep `DejaVuSans.LICENSE` alongside both faces. It exists so the
bundled default can use a real Bold face (M1-05g2) instead of synthesizing
one.
