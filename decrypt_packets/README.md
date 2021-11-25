# Packet Capture Decryption and Display

This tool is intended for learning and development purposes. It was originally written by me to help myself be able
to gain a better understanding of the PSO network communication protocol by being able to analyze the packets from
existing sessions with various different private servers. This was with the end goal of being able to write a new
minimal server implementation specifically and only to serve up download quests.

Unfortunately, with PSO being a fairly niche and old game, there is not exactly a burgeoning community of developers
with tons of comprehensive documentation, so writing tools to help myself and supplement the dearth of documentation
on PSO's network protocol was an important first step.

### Further Reading on PSO's Network Protocol

* http://web.archive.org/web/20171201191537/http://sharnoth.com/psodevwiki/
* http://www.fuzziqersoftware.com/files/psoprotocol.rtf

## Usage

This tool reads `.pcap` or `.pcapng` files produced by a capture tool, such as [Wireshark](https://www.wireshark.org/).

```text
decrypt_packets /path/to/capture.pcapng
```

It is probably important that your capture does not include anything else other than PSO Gamecube packet data. This
tool has **not** been tested against more broad captures which include a whole bunch of other intermingled packets
for other non-PSO things.

### Capturing Packets from PSO

This is probably easiest if you already have Dolphin set up to run PSO with a working network configuration. Setting up
Dolphin in this way is not actually easy itself. But if you can overcome that obstacle, then you can begin capturing
packets from your local computer right away.

I do not personally have such a setup configured because I am lazy and cannot be bothered to figure out the janky "Tap"
set up that Dolphin requires. This is also because my home router runs [OpenWrt](https://openwrt.org/) which allows me
to install and use the [iptables-mod-tee](https://openwrt.org/packages/pkgdata/iptables-mod-tee) iptables extension
and then I can configure packet mirroring through my router's iptables configuration for any device on my network 
(such as my Gamecube running PSO) and capture Gamecube traffic from my PC.

Alternatively, if you are running the PSO server yourself, then you can capture PSO traffic from the same computer that
runs the PSO server, as that would be far easier than setting something up to capture from your Gamecube directly.

In any event, the point of this section is not to provide a full how-to to set this up but just to get you pointed in
the right direction if you were unsure. If you're knowledgeable enough to be considering doing packet capture analysis 
of any sort in the first place, then you should be able to set up one of these methods to enable you to capture that 
traffic without too much fuss. If you are not knowledgeable like this, please do not contact me for assistance. This
kind of stuff is far too difficult and frustrating to remote troubleshoot with someone not well versed in this area!
