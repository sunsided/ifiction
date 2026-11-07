# Fixtures from babel

These files come from the `test/generic` and `test/html` directories of
[iftechfoundation/babel-tool](https://github.com/iftechfoundation/babel-tool), the Treaty of
Babel reference tool, at commit `22467dde1240a4c7dfca1baf01b3a3fb9befa523` (babel 0.75,
Treaty revision 13). They are unchanged.

They are licensed under the
[Creative Commons Attribution 4.0 International License](https://creativecommons.org/licenses/by/4.0/),
as the babel-tool repository states in its `LICENSE` file.

`expected` holds babel's own output for each directory; `tests/babel.rs` checks that
`ifiction::detect` finds the same IFIDs.
