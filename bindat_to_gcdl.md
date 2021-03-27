# PSO Ep 1 & 2 (Gamecube) Quest .bin/.dat to Download .qst Tool

This tool takes a set of `.bin` and `.dat` files for a Gamecube quest and turns it into a `.qst` file that can be
served up by a PSO server to Gamecube clients as "download quests" which can then be played by Gamecube users directly
from a memory card.

This tool performs basically the same process that [Qedit's](https://qedit.info/) save file type 
"Download Quest file(GC)" does.

## Usage

Given two files, `quest.bin` and `quest.dat`, a download quest file, `download.qst`, could be created using:

```text
bindat_to_gcdl quest.bin quest.dat download.qst
```
