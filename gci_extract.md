# PSO Ep 1 & 2 (Gamecube) Unencrypted PRS-compressed .gci Quest Extractor

This is a specialized tool written **specifically** to extract quest `.bin` and `.dat` files from `.gci` dumps of
Gamecube memory card quest files that were saved using a special Action Replay code which included an embedded
decryption key in the save file and then were manually decrypted with that decryption key.

Put another way, this tool will **only** work on `.gci` files found on [this gc-forever.com forum thread](https://www.gc-forever.com/forums/viewtopic.php?f=38&t=2050&start=75).
And only if they are indicated to be "unencrypted PRS compressed quests" and **not** "encrypted quests w/ embedded 
decryption key".

**You cannot use this tool to extract quests from any arbitrary `.gci` file you have on your Gamecube memory cards!**

(Maybe one day someone will reverse-engineer the method in which the Gamecube client derives the encryption key
from the player's serial number and access key. But until then, it is not possible for this tool to work with any
arbitrary `.gci` file.)

## Usage

Quest files on a Gamecube memory card are split into two files per quest. One file contains the quest `.bin` data, and
the other contains the quest `.dat` data. Therefore, two `.gci` files need to be provided to this tool. 

Special care should be taken to ensure that the two `.gci` files you provide are a matching pair **and** that they are 
provided in the right order! **The file containing the `.bin` data should be specified first.** 

```text
gci_extract 8P-GPOE-PSO______NNN.gci 8P-GPOE-PSO______NNN+1.gci
```

This will read out the data, parse and validate the quest information and save the raw (compressed) `.bin` and `.dat` 
file using a filename automatically derived from the quest's ID number.

Or, you can provide your own `.bin` and `.dat` filenames if you wish:

```text
gci_extract 8P-GPOE-PSO______NNN.gci 8P-GPOE-PSO______NNN+1.gci myquest.bin myquest.dat
```
