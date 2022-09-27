use binrw::{binrw, BinWrite};

use std::{
    error::Error,
    io::{Read, Seek, SeekFrom, Write},
};

use binrw::BinRead;

#[binrw]
#[brw(little)]
#[brw(magic = b"CISO")]
#[derive(Clone, Debug, PartialEq)]
pub struct CisoHeader {
    header_size: u32,
    total_bytes: u64,
    block_size: u32,
    ver: u8,
    align: u8,
    rsv_06: [u8; 2],
    #[br(count((total_bytes / (block_size as u64)) + 1))]
    index_buff: Vec<CisoIndex>,
}

#[binrw]
#[derive(Clone, Debug, PartialEq)]
pub struct CisoIndex(u32);

impl CisoIndex {
    pub fn is_plain(&self) -> bool {
        self.0 & 0x80000000 != 0
    }

    pub fn get_read_pos(&self, align: u8) -> u32 {
        self.0 & 0x7fffffff << align
    }
}

pub fn decomp_ciso<R: Read + Seek, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn Error>> {
    let ciso_header = CisoHeader::read(reader)?;

    let ciso_total_block = ciso_header.total_bytes / (ciso_header.block_size as u64);

    let mut decompresser = flate2::Decompress::new(false);

    for block in 0..ciso_total_block {
        let index = &ciso_header.index_buff[block as usize];
        let read_size = if index.is_plain() {
            ciso_header.block_size
        } else {
            let index2 = &ciso_header.index_buff[(block + 1) as usize];
            index2.get_read_pos(ciso_header.align) - index.get_read_pos(ciso_header.align)
        };

        reader.seek(SeekFrom::Start(index.get_read_pos(ciso_header.align) as u64))?;

        let mut read_buff = vec![0u8; read_size as usize];
        reader.read_exact(&mut read_buff)?;

        if index.is_plain() {
            writer.write_all(&read_buff)?;
        } else {
            let mut decompress_buff = vec![0u8; ciso_header.block_size as usize];
            decompresser.decompress(
                &read_buff,
                &mut decompress_buff,
                flate2::FlushDecompress::Finish,
            )?;
            writer.write_all(&decompress_buff)?;
            decompresser.reset(false);
        }
    }

    Ok(())
}

pub fn comp_ciso<R: Read + Seek, W: Write + Seek>(
    reader: &mut R,
    writer: &mut W,
    level: u8,
) -> Result<(), Box<dyn Error>> {
    let file_size = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;

    let mut ciso_header = CisoHeader {
        header_size: 0,
        total_bytes: file_size,
        block_size: 0x800,
        ver: 1,
        align: 0,
        rsv_06: [0u8; 2],
        index_buff: Vec::new(),
    };

    let ciso_total_block = ciso_header.total_bytes / (ciso_header.block_size as u64);

    ciso_header.index_buff = vec![CisoIndex(0); ciso_total_block as usize + 1];

    ciso_header.write(writer)?;

    let mut write_pos = writer.seek(SeekFrom::Current(0))?;

    let align_b = 1 << (ciso_header.align);
    let align_m = align_b - 1;

    let mut compresser = flate2::Compress::new(flate2::Compression::new(level as u32), false);

    for block in 0..ciso_total_block {
        let mut align = write_pos & align_m;
        if align > 0 {
            align = align_b - align;
            writer.write_all(&vec![0u8; align as usize])?;
            write_pos += align;
        }

        ciso_header.index_buff[block as usize] = CisoIndex((write_pos >> ciso_header.align) as u32);

        let mut data_buff = vec![0u8; ciso_header.block_size as usize];
        reader.read_exact(&mut data_buff)?;

        let mut compress_buff = vec![0u8; ciso_header.block_size as usize * 2];
        compresser.compress(
            &data_buff,
            &mut compress_buff,
            flate2::FlushCompress::Finish,
        )?;
        let mut cmp_size = compresser.total_out();

        if cmp_size >= (ciso_header.block_size as u64) {
            cmp_size = ciso_header.block_size as u64;
            writer.write_all(&data_buff)?;
            ciso_header.index_buff[block as usize].0 |= 0x80000000;
        } else {
            writer.write_all(&compress_buff[..cmp_size as usize])?;
        }

        write_pos += cmp_size;

        compresser.reset();
    }

    ciso_header.index_buff[ciso_total_block as usize] =
        CisoIndex((write_pos >> ciso_header.align) as u32);

    writer.seek(SeekFrom::Start(0))?;

    ciso_header.write(writer)?;

    Ok(())
}
