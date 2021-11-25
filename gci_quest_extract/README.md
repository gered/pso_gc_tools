# Unencrypted PRS-compressed `.gci` Quest Extractor

This is a specialized tool written **specifically** to extract quest `.bin` and `.dat` files from `.gci` dumps of
Gamecube memory card quest files that were saved using a special Action Replay code which enabled an embedded
decryption key to be included in the save file, following this the data was manually decrypted with that key.

Put another way, this tool will **only** work on `.gci` files found on [this gc-forever.com forum thread](https://www.gc-forever.com/forums/viewtopic.php?f=38&t=2050&start=75).
And even then, **only** if they are labelled as "unencrypted PRS compressed quests". It will **not** work on the quests 
found there which are labelled as "encrypted quests w/ embedded decryption key."

**You CANNOT use this tool to extract quests from any arbitrary `.gci` file you have on your Gamecube memory cards!**

(Maybe one day someone will reverse-engineer the method in which PSO derives the encryption from the player's serial
number and access key. But until then, it is not possible for this tool to work with any arbitrary `.gci` file.)

## Usage

Quest files on a Gamecube memory card are split into two files per quest. One file contains the quest `.bin` data, and
the other contains the quest `.dat` data. Therefore, two `.gci` files are needed for each single quest.

```text
gci_quest_extract <quest_1.gci> <quest_2.gci> <output.bin> <output.dat>
```

For example, in the aforementioned gc-forever forum thread, you can download a zip of quests in `.gci` files. Consult
the included text file for information about which files are for which quest. Then you can run this tool using
something like this:

```text
gci_quest_extract /path/to/8P-GPOE-PSO______022.gci /path/to/8P-GPOE-PSO______023.gci quest.bin quest.dat
```

This will extract the quest found out into the files `quest.bin` and `quest.dat`.

This tool will automatically try to figure out which of the `.gci` files provided is the quest `.bin` file and which
is the quest `.dat` file, so if you mix up the order, it should not matter. However, it is entirely up to you to make
sure you provide matching files for the same quest!
