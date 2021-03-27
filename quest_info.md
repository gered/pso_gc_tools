# PSO EP1&2 (Gamecube) Quest Info Tool

This tool can load Gamecube quest data from any of the following types of files:

- Compressed .bin + .dat file combo
- Online-play, unencrypted (0x44 / 0x13) .qst file (interleaved or not)
- Download/Offline-play, encrypted (0xA6 / 0xA7) .qst file (interleaved or not)

And display basic information about the quest and perform some basic validations on the data.

This tool was primarily written for my own benefit, as I needed something to help me quickly run through a series of
.bin/.dat files freshly extracted from a set of GCI files to validate them to make sure I had not done something silly
during the process.

## Usage

Simply pass either a `.bin` and `.dat` file (in that order) as arguments, or pass a single `.qst` file.

```text
quest_info quest.bin quest.dat

quest_info quest.qst
```
