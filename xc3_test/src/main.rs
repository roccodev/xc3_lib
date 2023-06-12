use std::{
    io::{BufReader, Cursor, Seek, SeekFrom},
    path::Path,
};

use binrw::BinReaderExt;
use clap::Parser;
use rayon::prelude::*;
use xc3_lib::{
    dds::{create_dds, create_mibl},
    map::{MapModelData, PropModelData},
    mibl::Mibl,
    model::ModelData,
    msmd::Msmd,
    msrd::{EntryType, Msrd},
    mxmd::Mxmd,
    sar1::Sar1,
    spch::Spch,
    xbc1::Xbc1,
};

fn main() {
    // Create a CLI for conversion testing instead of unit tests.
    // The main advantage is being able to avoid distributing assets.
    // The user can specify the path instead of hardcoding it.
    // It's also easier to apply optimizations like multithreading.

    let cli = Cli::parse();
    let root = Path::new(&cli.root_folder);

    let start = std::time::Instant::now();

    // Check conversions for various file types.
    if cli.mibl || cli.all {
        println!("Checking MIBL files ...");
        check_all_mibl(root);
    }

    if cli.mxmd || cli.all {
        println!("Checking MXMD files ...");
        check_all_mxmd(root);
    }

    if cli.msrd || cli.all {
        println!("Checking MSRD files ...");
        check_all_msrd(root);
    }

    if cli.msmd || cli.all {
        println!("Checking MSMD files ...");
        check_all_msmd(root);
    }

    if cli.sar1 || cli.all {
        println!("Checking SAR1 files ...");
        check_all_sar1(root);
    }

    // TODO: check standalone shaders

    println!("Finished in {:?}", start.elapsed());
}

fn check_all_mxmd<P: AsRef<Path>>(root: P) {
    // TODO: The map folder .wimdo files are a different format?
    // TODO: b"APMD" magic in "chr/oj/oj03010100.wimdo"?
    globwalk::GlobWalkerBuilder::from_patterns(root, &["*.wimdo", "!map/**"])
        .build()
        .unwrap()
        .par_bridge()
        .for_each(|entry| {
            let path = entry.as_ref().unwrap().path();
            // TODO: How to validate this file?
            match Mxmd::from_file(path) {
                Ok(_) => (),
                Err(e) => println!("Error reading {path:?}: {e}"),
            }
        });
}

fn check_all_mibl<P: AsRef<Path>>(root: P) {
    // The h directory doesn't have mibl footers?
    let folder = root.as_ref().join("chr").join("tex").join("nx");
    globwalk::GlobWalkerBuilder::from_patterns(folder, &["*.wismt", "!h/**"])
        .build()
        .unwrap()
        .par_bridge()
        .for_each(|entry| {
            let path = entry.as_ref().unwrap().path();
            let (original_bytes, mibl) = read_wismt_single_tex(path);
            check_mibl(original_bytes, mibl, path);
        });

    let folder = root.as_ref().join("monolib").join("shader");
    globwalk::GlobWalkerBuilder::from_patterns(folder, &["*.{witex,witx}"])
        .build()
        .unwrap()
        .par_bridge()
        .for_each(|entry| {
            let path = entry.as_ref().unwrap().path();
            let original_bytes = std::fs::read(path).unwrap();
            let mibl = Mibl::from_file(path).unwrap();
            check_mibl(original_bytes, mibl, path);
        });
}

fn check_all_msrd<P: AsRef<Path>>(root: P) {
    let folder = root.as_ref().join("chr");

    // Skip the .wismt textures in the tex folder.
    globwalk::GlobWalkerBuilder::from_patterns(folder, &["*.wismt", "!tex/**"])
        .build()
        .unwrap()
        .par_bridge()
        .for_each(|entry| {
            let path = entry.as_ref().unwrap().path();
            match Msrd::from_file(path) {
                Ok(msrd) => {
                    check_msrd(msrd);
                }
                Err(e) => println!("Error reading {path:?}: {e}"),
            }
        });
}

fn check_msrd(msrd: Msrd) {
    let decompressed_streams: Vec<_> = msrd
        .streams
        .iter()
        .map(|stream| stream.xbc1.decompress().unwrap())
        .collect();

    // Check parsing for any embedded files.
    for (i, item) in msrd.stream_entries.into_iter().enumerate() {
        match item.item_type {
            EntryType::Shader => {
                assert_eq!(i, msrd.shader_entry_index as usize);

                let stream = &decompressed_streams[item.stream_index as usize];
                let data = &stream[item.offset as usize..item.offset as usize + item.size as usize];

                Spch::read(&mut Cursor::new(data)).unwrap();
            }
            EntryType::Model => {
                assert_eq!(i, msrd.model_entry_index as usize);

                let stream = &decompressed_streams[item.stream_index as usize];
                let data = &stream[item.offset as usize..item.offset as usize + item.size as usize];

                ModelData::read(&mut Cursor::new(data)).unwrap();
            }
            // TODO: check texture parsing?
            EntryType::CachedTexture => {
                assert_eq!(i, msrd.texture_entry_index as usize);
            }
            EntryType::Texture => {}
        }
    }
}

fn check_all_msmd<P: AsRef<Path>>(root: P) {
    let folder = root.as_ref().join("map");

    globwalk::GlobWalkerBuilder::from_patterns(folder, &["*.wismhd"])
        .build()
        .unwrap()
        .par_bridge()
        .for_each(|entry| {
            let path = entry.as_ref().unwrap().path();
            match Msmd::from_file(path) {
                Ok(msmd) => {
                    check_msmd(msmd, path);
                }
                Err(e) => println!("Error reading {path:?}: {e}"),
            }
        });
}

fn check_msmd(msmd: Msmd, path: &Path) {
    // Parse all the data from the .wismda
    let mut reader = BufReader::new(std::fs::File::open(path.with_extension("wismda")).unwrap());

    // TODO: Move this functionality to xc3_lib?
    for model in msmd.map_models {
        reader
            .seek(SeekFrom::Start(model.entry.offset as u64))
            .unwrap();
        let bytes = Xbc1::read(&mut reader).unwrap().decompress().unwrap();
        let mut reader_inner = Cursor::new(bytes);
        let _: MapModelData = reader_inner.read_le().unwrap();
    }

    for model in msmd.prop_models {
        reader
            .seek(SeekFrom::Start(model.entry.offset as u64))
            .unwrap();
        let bytes = Xbc1::read(&mut reader).unwrap().decompress().unwrap();
        let mut reader_inner = Cursor::new(bytes);
        let _: PropModelData = reader_inner.read_le().unwrap();
    }

    for entry in msmd.map_model_data {
        reader.seek(SeekFrom::Start(entry.offset as u64)).unwrap();
        let bytes = Xbc1::read(&mut reader).unwrap().decompress().unwrap();
        let mut reader_inner = Cursor::new(bytes);
        let _: ModelData = reader_inner.read_le().unwrap();
    }

    for entry in msmd.prop_model_data {
        reader.seek(SeekFrom::Start(entry.offset as u64)).unwrap();
        let bytes = Xbc1::read(&mut reader).unwrap().decompress().unwrap();
        let mut reader_inner = Cursor::new(bytes);
        let _: ModelData = reader_inner.read_le().unwrap();
    }
}

fn check_mibl(original_bytes: Vec<u8>, mibl: Mibl, path: &Path) {
    let dds = create_dds(&mibl).unwrap();
    let new_mibl = create_mibl(&dds).unwrap();

    let mut writer = Cursor::new(Vec::new());
    new_mibl.write(&mut writer).unwrap();

    // DDS should support all MIBL image formats.
    // Check that read -> MIBL -> DDS -> MIBL -> write is 1:1.
    if original_bytes != writer.into_inner() {
        println!("Read/write not 1:1 for {path:?}");
    };
}

fn read_wismt_single_tex<P: AsRef<Path>>(path: P) -> (Vec<u8>, Mibl) {
    let xbc1 = Xbc1::from_file(path).unwrap();

    let decompressed = xbc1.decompress().unwrap();
    let mut reader = Cursor::new(decompressed.clone());
    (decompressed, Mibl::read(&mut reader).unwrap())
}

fn check_all_sar1<P: AsRef<Path>>(root: P) {
    let folder = root.as_ref().join("chr");
    globwalk::GlobWalkerBuilder::from_patterns(folder, &["*.chr"])
        .build()
        .unwrap()
        .par_bridge()
        .for_each(|entry| {
            // TODO: How to validate this file?
            let path = entry.as_ref().unwrap().path();
            match Sar1::from_file(path) {
                Ok(_) => (),
                Err(e) => println!("Error reading {path:?}: {e}"),
            }
        });
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// The root folder that contains folders like `chr/` and `monolib/`.
    root_folder: String,

    /// Process MIBL image files from .witex, .witx, .wismt
    #[arg(long)]
    mibl: bool,

    /// Process MXMD model files from .wimdo
    #[arg(long)]
    mxmd: bool,

    /// Process MSRD model files from .wismt
    #[arg(long)]
    msrd: bool,

    /// Process MSMD map files from .wismhd
    #[arg(long)]
    msmd: bool,

    /// Process SAR1 model files from .chr
    #[arg(long)]
    sar1: bool,

    /// Process all file types
    #[arg(long)]
    all: bool,
}
