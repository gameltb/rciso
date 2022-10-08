use binrw::{binrw, BinWrite};

use std::{
    error::Error,
    io::{Read, Seek, SeekFrom, Write},
};

use binrw::BinRead;

pub struct Ciso<R: Read + Seek> {
    source: R,
    ciso_header: CisoHeader,
    current_pos: usize,
}

impl<R: Read + Seek> Ciso<R> {
    pub fn new(mut source: R) -> Result<Self, Box<dyn Error>> {
        let ciso_header = CisoHeader::read(&mut source)?;
        Ok(Self {
            source,
            ciso_header,
            current_pos: 0,
        })
    }

    pub fn read_block(&mut self, buf: &mut [u8], block: usize) -> std::io::Result<usize> {
        if (block + 1) >= self.ciso_header.index_buff.len() {
            return Ok(0);
        }

        let index = &self.ciso_header.index_buff[block];
        let read_size = if index.is_plain() {
            self.ciso_header.block_size
        } else {
            let index2 = &self.ciso_header.index_buff[block + 1];
            index2.get_read_pos(self.ciso_header.align) - index.get_read_pos(self.ciso_header.align)
        };

        self.source.seek(SeekFrom::Start(
            index.get_read_pos(self.ciso_header.align) as u64
        ))?;

        if index.is_plain() {
            self.source.read_exact(&mut buf[..read_size as usize])?;
            Ok(read_size as usize)
        } else {
            let mut read_buff = vec![0u8; read_size as usize];
            self.source.read_exact(&mut read_buff)?;
            let mut decompresser = flate2::Decompress::new(false);
            decompresser.decompress(&read_buff, buf, flate2::FlushDecompress::Finish)?;
            Ok(decompresser.total_out() as usize)
        }
    }
}

impl<R: Read + Seek> Read for Ciso<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let origin_pos = self.current_pos;
        let mut current_block = self.current_pos / (self.ciso_header.block_size as usize);
        let this_block_pos = self.current_pos % (self.ciso_header.block_size as usize);

        let mut buff_pos = 0;
        let buf_len = buf.len();

        if this_block_pos != 0 {
            let mut tmp_buff = vec![0u8; self.ciso_header.block_size as usize];
            let result = self.read_block(&mut tmp_buff, current_block)?;
            if result == 0 {
                return Ok(0);
            }
            if (buf_len - buff_pos) >= (result - this_block_pos) {
                buf[buff_pos..result].copy_from_slice(&tmp_buff[this_block_pos..result]);
                buff_pos += result - this_block_pos;
                self.current_pos += result - this_block_pos;
                current_block += 1;
            } else {
                buf[buff_pos..(buf_len - buff_pos)].copy_from_slice(
                    &tmp_buff[this_block_pos..(this_block_pos + buf_len - buff_pos)],
                );
                buff_pos += buf_len - buff_pos;
                self.current_pos += buf_len - buff_pos;
            }
        }
        while buff_pos < buf_len {
            let result = self.read_block(&mut buf[buff_pos..buf_len], current_block)?;
            if result == 0 {
                break;
            }
            buff_pos += result;
            self.current_pos += result;
            current_block += 1;
        }
        Ok(self.current_pos - origin_pos)
    }
}

impl<R: Read + Seek> Seek for Ciso<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Start(pos) => self.current_pos = pos as usize,
            SeekFrom::End(offset) => {
                let new_pos = self.ciso_header.total_bytes as i64 + offset;
                if new_pos >= 0 {
                    self.current_pos = new_pos as usize;
                } else {
                    self.current_pos = 0;
                }
            }
            SeekFrom::Current(offset) => {
                let new_pos = self.current_pos as i64 + offset;
                if new_pos >= 0 {
                    self.current_pos = new_pos as usize;
                } else {
                    self.current_pos = 0;
                }
            }
        }
        Ok(self.current_pos as u64)
    }
}

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
