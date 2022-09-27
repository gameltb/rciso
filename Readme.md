# RCISO

rciso is a port of ciso, which a simple commandline utility to compress PSP iso files.

# build

```shell
cargo build --features build-binary --release
```

# usage

```shell
$ target/release/rciso -h
rciso 0.1.0
Compressed ISO9660 converter rust version

USAGE:
    rciso --level <LEVEL> <INFILE> <OUTFILE>

ARGS:
    <INFILE>     Path of the input file
    <OUTFILE>    Path of the output file

OPTIONS:
    -h, --help             Print help information
    -l, --level <LEVEL>    1-9 compress ISO to CSO (1=fast/large - 9=small/slow)
                           0   decompress CSO to ISO
    -V, --version          Print version information
```