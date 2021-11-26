# PSO Episode I & II Gamecube Tools

A small collection of tools, mainly intended to assist with my own efforts in making it easier to prepare and distribute
downloadable (or "offline play") quests.

Please note that I am **only** interested in the Gamecube version of PSO. I do not own 'nor play any of the other
versions (Dreamcast, Xbox or Blue Burst). Because of this, the tools found in this repository are laser-focused on the
Gamecube version of PSO only, and that will not change.

## Tools

* [decrypt_packets](decrypt_packets/README.md): Tool for decrypting and displaying raw packets captured as a `.pcapng` file from a PSO Gamecube client/server session.
* [gci_quest_extract](gci_quest_extract/README.md): A very specific tool for extracting PSO Gamecube quests **only** out of pre-decrypted `.gci` files.
* [psogc_quest_tool](psogc_quest_tool/README.md): Conversion and info tool for PSO Gamecube quest `.bin`/`.dat` and/or `.qst` files.
* [psoutils](psoutils/README.md): Library that all of these tools use that contains useful PSO Gamecube things (quest file formats, encryption, compression, text, etc).

(This is more or less my first project of non-trivial size in Rust. I am still learning the language and ecosystem,
and this repository probably includes quite a number of mistakes or poor quality code because I simply don't know any
better at this time. Feel free to point out any mistakes or suggest improvements!)
