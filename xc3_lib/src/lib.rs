// TODO: consistent naming for magics/extensions?
// mibl instead of lbim?
// TODO: Is the pointer placement algorithm similar enough to SSBH?
// TODO: naming for wismt vertex data?

use std::{
    error::Error,
    io::{BufWriter, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};

use binrw::{BinRead, BinReaderExt, BinResult, BinWrite, NullString, VecArgs};

pub mod dds;
pub mod mibl;
pub mod model;
pub mod msmd;
pub mod msrd;
pub mod mxmd;
pub mod sar1;
pub mod spch;
pub mod xbc1;

// TODO: Make a type for this and just use temp to derive it?
fn parse_array<T, R>(reader: &mut R, endian: binrw::Endian, _args: ()) -> BinResult<Vec<T>>
where
    for<'a> T: BinRead<Args<'a> = ()> + 'static,
    R: std::io::Read + std::io::Seek,
{
    let offset = u32::read_options(reader, endian, ())?;
    let count = u32::read_options(reader, endian, ())?;

    let saved_pos = reader.stream_position()?;

    reader.seek(SeekFrom::Start(offset as u64))?;

    let values = Vec::<T>::read_options(
        reader,
        endian,
        VecArgs {
            count: count as usize,
            inner: (),
        },
    )?;

    reader.seek(SeekFrom::Start(saved_pos))?;

    Ok(values)
}

fn parse_string_ptr32<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    args: (u64,),
) -> BinResult<String> {
    let offset = u32::read_options(reader, endian, ())?;
    let saved_pos = reader.stream_position()?;

    reader.seek(SeekFrom::Start(args.0 + offset as u64))?;
    let value = NullString::read_options(reader, endian, ())?;
    reader.seek(SeekFrom::Start(saved_pos))?;

    Ok(value.to_string())
}

// TODO: Dedicated error types?
macro_rules! file_read_write_impl {
    ($($type_name:path),*) => {
        $(
            impl $type_name {
                pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, Box<dyn Error>> {
                    reader.read_le().map_err(Into::into)
                }

                pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
                    let mut reader = Cursor::new(std::fs::read(path)?);
                    reader.read_le().map_err(Into::into)
                }

                pub fn write<W: Write + Seek>(&self, writer: &mut W) -> Result<(), Box<dyn Error>> {
                    self.write_le(writer).map_err(Into::into)
                }

                pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn Error>> {
                    let mut writer = BufWriter::new(std::fs::File::create(path)?);
                    self.write_le(&mut writer).map_err(Into::into)
                }
            }
        )*
    };
}

file_read_write_impl!(mibl::Mibl);

macro_rules! file_read_impl {
    ($($type_name:path),*) => {
        $(
            impl $type_name {
                pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, Box<dyn Error>> {
                    reader.read_le().map_err(Into::into)
                }

                pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
                    let mut reader = Cursor::new(std::fs::read(path)?);
                    reader.read_le().map_err(Into::into)
                }
            }
        )*
    };
}

file_read_impl!(
    msrd::Msrd,
    mxmd::Mxmd,
    sar1::Sar1,
    spch::Spch,
    xbc1::Xbc1,
    model::ModelData
);
