use std::{
    fs::{File, OpenOptions},
    io::BufReader,
};

use clap::Parser;
use rciso::*;

/// Compressed ISO9660 converter rust version
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path of the input file
    #[clap(value_parser)]
    infile: String,

    /// Path of the output file
    #[clap(value_parser)]
    outfile: String,

    /// 1-9 compress ISO to CSO (1=fast/large - 9=small/slow)
    /// 0   decompress CSO to ISO
    #[clap(short, long, value_parser, verbatim_doc_comment)]
    level: u8,
}

fn main() {
    let args = Args::parse();

    let file = OpenOptions::new().read(true).open(args.infile).unwrap();
    let mut file = BufReader::new(file);

    let mut outfile = File::create(args.outfile).unwrap();

    if args.level <= 0 {
        decomp_ciso(&mut file, &mut outfile).unwrap();
    } else if args.level <= 9 {
        comp_ciso(&mut file, &mut outfile, args.level).unwrap();
    } else {
        println!("unspport compress level.")
    }
}
