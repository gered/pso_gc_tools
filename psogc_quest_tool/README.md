# Quest Info and Conversion Tool

This tool can be used to display information about and perform simple data validations for any given PSO Gamecube quest
as well as convert between various different quest file formats.

## Usage

### Quest Info

Use the `info` command argument and pass the quest file(s).

```text
psogc_quest_tool info quest.bin quest.dat

psogc_quest_tool info quest.qst
```

When providing .bin and .dat files, this tool will automatically figure out which is the .bin and .dat file, so if you
mix up the order of these files it does not matter.

### Quest Conversion

Use the `convert` command argument and pass the input quest file(s) and output quest file(s).

```text 
psogc_quest_tool convert <input files> <output_format_type> <output_files>
```

Where:

* `<input files>` should be either two files, a .bin and .dat file, or a single .qst file.
* `<output_files>` same as the above, but the `<output_format_type>` dictates the files you should specify here.
* `<output_format_type>` should be one of the following:
  * `raw_bindat` - Produces a .bin and .dat file, both uncompressed.
  * `prs_bindat` - Produces a .bin and .dat file, both PRS compressed.
  * `online_qst` - Produces a .qst file using packets 0x44 and 0x13 for online play via a server.
  * `offline_qst` - Produces a .qst file using packets 0xA6 and 0xA7 for offline play from a memory card when downloaded from a server.
