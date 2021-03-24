# PSO Ep 1 & 2 (Gamecube) .qst Header Generator Tool

This is a simple tool that can generate quest `.qst` file headers for a given set of `.bin` and `.dat` files. This tool
was written to complement Sylverant's [qst_tool](https://github.com/Sylverant/pso_tools/tree/master/qst_tool) which
has primitive support for automatically generating a `.qst` file header if one is not provided.

**This tool is NOT required if you are using the "bindat_to_gcdl" tool also included in this repository. That tool
automatically generates the necessary header information in an identical manner to how this tool does.**

## Usage

Given two quest `.bin` and `.dat` files ...

```
gen_qst_header quest.bin quest.dat
```

Will result in the `.bin` file's header information being saved to a file called `quest.bin.hdr` and the `.dat` file's
header information being saved to a file called `quest.dat.hdr`.

This can then be used with "qst_tool" to generate a `.qst` file if you wish:

```
qst_tool -m gc quest.bin quest.dat quest.bin.hdr quest.dat.hdr
```
