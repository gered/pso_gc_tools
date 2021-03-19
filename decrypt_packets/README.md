# PSO Ep 1 & 2 (Gamecube) Client/Server Packets Decrypter Tool

This is a tool that will take raw binary PSO client and server packet data dumps generated from a packet capture tool 
(such as Wireshark) and display the decrypted packet data.

I put this tool together for myself to help further my understanding of PSO's network communication. More specifically,
to help me troubleshoot why my attempts at setting up Sylverant's open source [login_server](https://github.com/Sylverant/login_server)
to serve up quests for download was resulting in unusable quest files on Gamecube memory cards. Understanding the
quest download communication better by analyzing the packets being sent from a working implementation and comparing it
to what my local login_server instance was sending proved invaluable to me.

## Network Protocol

After the initial `0x17` packet sent from the server to the client (which contains the client and server encryption
keys), all subsequent communication between the server and client is encrypted. When you have the full set of packets, 
beginning with the `0x17` packet, it is pretty trivial to decrypt the entire set of data.

```text
'Welcome' packet. id=17, flags=0, size=276
0000 | 17 00 14 01 44 72 65 61 6D 43 61 73 74 20 50 6F | ....DreamCast Po
0010 | 72 74 20 4D 61 70 2E 20 43 6F 70 79 72 69 67 68 | rt Map. Copyrigh
0020 | 74 20 53 45 47 41 20 45 6E 74 65 72 70 72 69 73 | t SEGA Enterpris
0030 | 65 73 2E 20 31 39 39 39 00 00 00 00 00 00 00 00 | es. 1999........
0040 | 00 00 00 00 6B 81 4B 4F 01 A2 65 78 54 68 69 73 | ....k.KO..exThis
0050 | 20 73 65 72 76 65 72 20 69 73 20 69 6E 20 6E 6F |  server is in no
0060 | 20 77 61 79 20 61 66 66 69 6C 69 61 74 65 64 2C |  way affiliated,
0070 | 20 73 70 6F 6E 73 6F 72 65 64 2C 20 6F 72 20 73 |  sponsored, or s
0080 | 75 70 70 6F 72 74 65 64 20 62 79 20 53 45 47 41 | upported by SEGA
0090 | 20 45 6E 74 65 72 70 72 69 73 65 73 20 6F 72 20 |  Enterprises or
00A0 | 53 4F 4E 49 43 54 45 41 4D 2E 20 54 68 65 20 70 | SONICTEAM. The p
00B0 | 72 65 63 65 64 69 6E 67 20 6D 65 73 73 61 67 65 | receding message
00C0 | 20 65 78 69 73 74 73 20 6F 6E 6C 79 20 69 6E 20 |  exists only in
00D0 | 6F 72 64 65 72 20 74 6F 20 72 65 6D 61 69 6E 20 | order to remain
00E0 | 63 6F 6D 70 61 74 69 62 6C 65 20 77 69 74 68 20 | compatible with
00F0 | 70 72 6F 67 72 61 6D 73 20 74 68 61 74 20 65 78 | programs that ex
0100 | 70 65 63 74 20 69 74 2E 00 00 00 00 00 00 00 00 | pect it.........
0110 | 00 00 00 00                                     | ....

server_key = 0x4f4b816b
client_key = 0x7865a201
```

Note, sometimes the `0x17` packet will contain significantly less text than what is shown above. The above output is 
from a Fuzziqer [newserv](https://github.com/fuzziqersoftware/newserv) I was testing with.

Also of note is that Sylverant's login_server currently seems to always use identical server and client keys (I believe
this is a bug in libsylverant's usage of its random number generator library). This does not cause problems, but it is 
weird to see when you first notice it.

Some relevant reading regarding PSO's network protocol:

* [Network Protocol](http://web.archive.org/web/20171201191557/http://sharnoth.com/psodevwiki/net/protocol)
* [Network Protocol Messages](http://web.archive.org/web/20171201191532/http://sharnoth.com/psodevwiki/net/messages)
* ["Detailed" Message Flow](http://web.archive.org/web/20171201191527/http://sharnoth.com/psodevwiki/net/message_flow)

Currently, [libsylverant](https://github.com/Sylverant/libsylverant) has the cleanest and easiest to use PSO encryption
API, and that is what is used by this tool.

**Note that the PSO encryption method (and thus, the `CRYPT_` API provided by libsylverant) is stateful**. That is, you 
cannot just use it to arbitrarily decrypt any single random packet and expect it to result in readable data. To 
correctly decrypt any individual packet from either client or server, you need to work through the full sequence of 
packets (for either client or server) beginning with the very first client or server packet (**after** the `0x17` 
packet) up to the packet(s) you really wanted, decrypting all of it along the way. 

## Usage

### Capturing Packets from PSO

This is probably easiest if you already have Dolphin set up to run PSO with a working network configuration. In such
a configuration, you can capture from your local computer right away.

I do not have this set up and I cannot be bothered to figure out the janky "Tap" set up that Dolphin requires. Mostly 
because I am lazy. And because I have a router running [OpenWrt](https://openwrt.org/) which allows me to easily set up
packet mirroring with a special iptables kernel module loaded so that I can capture packets directly from my Gamecube.

I'm not going to go into details here on setting up either method. If you're knowledgeable enough to be considering 
doing packet capture analysis of any sort in the first place, then you should be able to set up either method yourself.

### Dumping PSO Server/Client Communication Data Dumps with Wireshark 

It is easy to generate packet data dumps containing _just_ the PSO packet data we are interested in with Wireshark. 

After taking a capture of a PSO server/client session, find the TCP packet sent from the server to the client that
contains the `0x17` packet. This should be easy enough to find as it will be one of the first TCP packets sent from the
server to the client and contains the clear-text string `DreamCast Port Map. Copyright SEGA Enterprises. 1999` (for 
non-BB clients anyway, this tool is aimed at GC anyway so that is all I will be covering ...).

Once you've found this packet, right-click it from the top packet list and select "Follow" then "TCP Stream". This will
bring up a window that shows the raw data, colour-coded to show data originating from the client and server. Use the
drop-downs and buttons at the bottom of this window to save "Raw"-format data for the client and server in 
**individual** files.

### Decrypting

Assuming you saved the data to two files, `server.bin` (containing server-to-client packets) and `client.bin` 
(containing client-to-server packets), you can run the tool like so:

```text
decrypt_packets /path/to/server.bin /path/to/client.bin
```
