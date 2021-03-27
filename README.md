# PSO Ep I & II Gamecube Tools

A small collection of tools, intended to assist with my own efforts in investigating how download/offline quests need
to be prepared in order to work correctly and tools to automate that process.

## Tools

* [bindat_to_gcdl](bindat_to_gcdl.md): Turns a set of .bin/.dat files into a Gamecube-compatible offline/download quest .qst file.
* [decrypt_packets](decrypt_packets.md): Decrypts server/client packet capture.
* [gci_extract](gci_extract.md): Extracts quest .bin/.dat files **only** from specially prepared Gamecube memory card dumps in .gci format. This is a highly specific tool that is **not** usable on any arbitrary .gci file!
* [gen_qst_header](gen_qst_header.md): Generates nicer .qst header files than what [qst_tool](https://github.com/Sylverant/pso_tools/tree/master/qst_tool) does. Can be then fed into qst_tool.
* [quest_info](quest_info.md): Displays basic information about quest files (supports both .bin/.dat and .qst formats).
