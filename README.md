# bookshelfman

## Development

A Nix flake provides Rust 1.99.1 and the native tools required by
`wreq`/BoringSSL (`cmake`, `libclang`, and GCC):

```sh
nix develop
cargo build
cargo test --all-targets
cargo test --doc
cargo run
```

Or run a command directly without entering a shell:

```sh
nix develop --command cargo build
```

The first invocation downloads the toolchain and builds BoringSSL, which can
take several minutes. The dev shell is provided for `x86_64-linux` and
`aarch64-linux`.

## License

bookshelfman is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version.

bookshelfman is distributed in the hope that it will be useful, but WITHOUT
ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with
bookshelfman. If not, see <https://www.gnu.org/licenses/>.

The full license text is available in [LICENSE](LICENSE).
