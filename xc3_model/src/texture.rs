use std::{error::Error, path::Path};

use image_dds::Surface;
use thiserror::Error;
use xc3_lib::{
    mibl::{Mibl, SwizzleError},
    msrd::Msrd,
    mxmd::{Mxmd, PackedTexture},
    xbc1::Xbc1,
};

pub use xc3_lib::mibl::{ImageFormat, ViewDimension};

#[derive(Debug, Error)]
pub enum CreateImageTextureError {
    #[error("error deswizzling surface: {0}")]
    Swizzle(#[from] SwizzleError),

    #[error("error reading data: {0}")]
    Binrw(#[from] binrw::Error),

    #[error("error decompressing stream: {0}")]
    Stream(#[from] xc3_lib::error::DecompressStreamError),
}

/// A non swizzled version of an [Mibl] texture.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageTexture {
    /// An optional name assigned to some textures.
    /// This will typically be [None]
    /// and can not be used for reliably identifying textures.
    pub name: Option<String>,
    /// The width of the base mip level in pixels.
    pub width: u32,
    /// The height of the base mip level in pixels.
    pub height: u32,
    /// The depth of the base mip level in pixels.
    pub depth: u32,
    pub view_dimension: ViewDimension, // TODO: is this redundant?
    pub image_format: ImageFormat,
    /// The number of mip levels or 1 if there are no mipmaps.
    pub mipmap_count: u32,
    /// The combined image surface data in a standard row-major layout.
    /// Ordered as `Layer 0 Mip 0, Layer 0 Mip 1, ... Layer L-1 Mip M-1`
    /// for L layers and M mipmaps similar to DDS files.
    pub image_data: Vec<u8>,
}

impl ImageTexture {
    /// Deswizzle the data from `mibl`.
    /// The `name` is not required but creates more descriptive file names and debug information.
    pub fn from_mibl(mibl: &Mibl, name: Option<String>) -> Result<Self, SwizzleError> {
        Ok(Self {
            name,
            width: mibl.footer.width,
            height: mibl.footer.height,
            depth: mibl.footer.depth,
            view_dimension: mibl.footer.view_dimension,
            image_format: mibl.footer.image_format,
            mipmap_count: mibl.footer.mipmap_count,
            image_data: mibl.deswizzled_image_data()?,
        })
    }

    /// Deswizzle and combine the data from `base_mip_level` for mip 0 and `mibl_m` for the remaining mip levels.
    pub fn from_mibl_base_mip(
        base_mip_level: Vec<u8>,
        mibl_m: Mibl,
        name: Option<String>,
    ) -> Result<Self, SwizzleError> {
        // TODO: double depth?
        let width = mibl_m.footer.width * 2;
        let height = mibl_m.footer.height * 2;
        let depth = mibl_m.footer.depth;

        let image_data = mibl_m.deswizzle_image_data_base_mip(base_mip_level)?;
        Ok(ImageTexture {
            name,
            width,
            height,
            depth,
            view_dimension: mibl_m.footer.view_dimension,
            image_format: mibl_m.footer.image_format,
            mipmap_count: mibl_m.footer.mipmap_count + 1,
            image_data,
        })
    }

    pub fn from_packed_texture(texture: &PackedTexture) -> Result<Self, CreateImageTextureError> {
        let mibl = Mibl::from_bytes(&texture.mibl_data)?;
        Self::from_mibl(&mibl, Some(texture.name.clone())).map_err(Into::into)
    }

    pub fn to_image(&self) -> Result<image_dds::image::RgbaImage, Box<dyn Error>> {
        let dds = self.to_dds()?;
        image_dds::image_from_dds(&dds, 0).map_err(Into::into)
    }

    pub fn to_surface(&self) -> image_dds::Surface<&[u8]> {
        Surface {
            width: self.width,
            height: self.height,
            depth: self.depth,
            layers: if self.view_dimension == ViewDimension::Cube {
                6
            } else {
                1
            },
            mipmaps: self.mipmap_count,
            image_format: self.image_format.into(),
            data: &self.image_data,
        }
    }

    pub fn to_dds(&self) -> Result<image_dds::ddsfile::Dds, Box<dyn Error>> {
        self.to_surface().to_dds().map_err(Into::into)
    }

    // TODO: from_dds and from_surface?
    // TODO: to_mibl?
}

pub(crate) fn load_textures(
    mxmd: &Mxmd,
    msrd: Option<&Msrd>,
    m_tex_folder: &Path,
    h_tex_folder: &Path,
) -> Vec<ImageTexture> {
    // TODO: packed mxmd, external mxmd, low res msrd, msrd,
    // TODO: Is this the correct way to handle this?
    // TODO: Is it possible to have both packed and external mxmd textures?
    if let Some(textures) = &mxmd.textures {
        let mxmd_textures = match &textures.inner {
            xc3_lib::mxmd::TexturesInner::Unk0(t) => Some(&t.textures1.textures),
            xc3_lib::mxmd::TexturesInner::Unk1(t) => t.textures.as_ref().map(|t| &t.textures),
        };

        let packed_texture_data = msrd.unwrap().extract_low_texture_data().unwrap();
        // TODO: These textures aren't in the same order?
        let middle_textures = msrd.unwrap().extract_middle_textures().unwrap();

        // TODO: Same as msrd?
        let texture_ids = &msrd.as_ref().unwrap().texture_ids;

        // Assume the packed and non packed textures have the same ordering.
        // Xenoblade 3 has some textures in the chr/tex folder.
        // TODO: Are the mxmd and msrd packed texture lists always identical?
        mxmd_textures
            .map(|packed_textures| {
                packed_textures
                    .iter()
                    .enumerate()
                    .map(|(i, texture)| {
                        load_wismt_texture(m_tex_folder, h_tex_folder, &texture.name)
                            .ok()
                            .or_else(|| {
                                // TODO: Assign in a second pass to avoid O(N) find.
                                texture_ids
                                    .iter()
                                    .position(|id| *id as usize == i)
                                    .and_then(|index| {
                                        middle_textures.get(index).map(|mibl| {
                                            ImageTexture::from_mibl(
                                                mibl,
                                                Some(texture.name.clone()),
                                            )
                                            .unwrap()
                                        })
                                    })
                            })
                            .unwrap_or_else(|| {
                                // Some textures only appear in the packed textures and have no high res version.
                                load_packed_texture(&packed_texture_data, texture).unwrap()
                            })
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else if let Some(packed_textures) = &mxmd.packed_textures {
        packed_textures
            .textures
            .iter()
            .map(|t| ImageTexture::from_packed_texture(t).unwrap())
            .collect()
    } else {
        // TODO: How to handle this case?
        Vec::new()
    }
}

fn load_packed_texture(
    packed_texture_data: &[u8],
    item: &xc3_lib::mxmd::PackedExternalTexture,
) -> Result<ImageTexture, CreateImageTextureError> {
    let data = &packed_texture_data
        [item.mibl_offset as usize..item.mibl_offset as usize + item.mibl_length as usize];

    let mibl = Mibl::from_bytes(data)?;
    ImageTexture::from_mibl(&mibl, Some(item.name.clone())).map_err(Into::into)
}

fn load_wismt_texture(
    m_texture_folder: &Path,
    h_texture_folder: &Path,
    texture_name: &str,
) -> Result<ImageTexture, CreateImageTextureError> {
    // TODO: Create a helper function in xc3_lib for this?
    let xbc1 = Xbc1::from_file(m_texture_folder.join(texture_name).with_extension("wismt"))?;
    let mibl_m: Mibl = xbc1.extract()?;

    let base_mip_level =
        Xbc1::from_file(h_texture_folder.join(texture_name).with_extension("wismt"))?
            .decompress()?;

    ImageTexture::from_mibl_base_mip(base_mip_level, mibl_m, Some(texture_name.to_string()))
        .map_err(Into::into)
}
