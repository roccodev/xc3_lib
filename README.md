# xc3_lib
An experimental Rust library for reading rendering related file formats for Xenoblade Chronicles 3.

The initial focus is creating robust and readable parsing code for formats related to in game rendering. Writing support and higher level libraries may be added at a later date once the formats have been more thoroughly researched. Formats not directly related to rendering may be considered at a later date.

## xc3_lib
The file format library and utilities.

## xc3_shader
A library and command line tool for working with the game's compiled shader programs. Run `cargo run --release -p xc3_shader -- --help` for usage instructions. 

## xc3_test
A command line tool for testing parsing and conversion code for all files in an extracted dump. Details for failed conversions will be printed to the console. File types can all be enabled at once or enabled individually.  

`cargo run -p xc3_test --release <path to extracted folder> --all`  
`cargo run -p xc3_test --release <path to extracted folder> --mxmd --mibl`

## xc3_tex
A command line tool for converting texture files to and from DDS or image formats like PNG, TIFF, or JPEG.

## xc3_viewer
A simple winit desktop application for rendering model files using xc3_wgpu.

## xc3_wgpu
A wgpu based renderer for model files with an emphasis on portability and readability over perfect in game accuracy.

## Usage
This library is still highly experimental. When adding this project to the Cargo.toml, specify a specific git revision or commit the Cargo.lock file to source control. This locks the version and avoids any breaking changes. The debug or JSON output has not stabilized and should not be assumed to be the same between commits.

`xc3_lib = { git = "https://github.com/ScanMountGoat/xc3_lib", rev = "commit_hash" }`  

## Building
After installing the [Rust toolchain](https://www.rust-lang.org/tools/install), run `cargo build --release` in the repository directory.
Running `cargo build` without the `--release` will result in faster compile times during development but dramatically worse runtime performance. The tools can be run using `cargo run --release -p <project> <args>`.

## Documentation
The projects are not currently published to crates.io, so run `cargo doc -p xc3_lib --open` to generate and view the rustdoc output in the browser. Replace xc3_lib with the name of other packages to view the corresponding documentation.

## Tests
Unit tests and doc tests can be run using `cargo test`. Most of the file processing and conversion code is tested by running the xc3_test executable against an extracted dump of the game.

## Credits
This project makes use of a number of Rust crates that are useful for reverse engineering. For a full list of dependencies, see the Cargo.toml files.
* [binrw](https://github.com/jam1garner/binrw) - declarative binary parsing
* [tegra_swizzle](https://github.com/ScanMountGoat/tegra_swizzle) - efficient and robust Tegra X1 swizzling/deswizzling
* [image_dds](https://github.com/ScanMountGoat/image_dds) - encode/decode BCN image data

This project is based on previous reverse engineering work, including work done for Xenoblade 2.
Special thanks go to members of the World Tree Research discord (formerly the World of Alrest discord) for their assistance.
* [Xenoblade Data Hub](https://xenobladedata.github.io/)
* [XenoLib](https://github.com/PredatorCZ/XenoLib)
* [XB2AssetTool](https://github.com/BlockBuilder57/XB2AssetTool)
* [Xenoblade-Switch-Model-Importer-Noesis](https://github.com/Turk645/Xenoblade-Switch-Model-Importer-Noesis)
* [XbTool](https://github.com/AlexCSDev/XbTool)
