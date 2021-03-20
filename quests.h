#ifndef QUESTS_H_INCLUDED
#define QUESTS_H_INCLUDED

#include <stdio.h>
#include <stdint.h>

#define PACKET_ID_QUEST_INFO_ONLINE    0x44
#define PACKET_ID_QUEST_INFO_DOWNLOAD  0xa6
#define PACKET_ID_QUEST_CHUNK_ONLINE   0x13
#define PACKET_ID_QUEST_CHUNK_DOWNLOAD 0xa7

// quest .bin file header (after file contents have been prs-decompressed)
typedef struct __attribute__((packed)) {
	uint32_t object_code_offset;
	uint32_t function_offset_table_offset;
	uint32_t bin_size;
	uint32_t xffffffff;                     // always 0xffffffff ?
	uint16_t language;
	uint16_t quest_number;

	// some sources say these strings are all UTF-16LE, but i'm not sure that is really the case for gamecube data?
	// for gamecube-format quest .bin files, it instead looks like SHIFT-JIS probably ... ?

	char name[32];
	char short_description[128];
	char long_description[288];
} QUEST_BIN_HEADER;

// .qst file header, for either the embedded bin or dat quest data
typedef struct __attribute__((packed)) {
	// 0xA6 = download to memcard, 0x44 = download for online play
	// (quest file data chunks must then be encoded accordingly. 0xA6 = use 0xA7, and 0x44 = use 0x13)
	uint8_t pkt_id;

	// khyller sets .dat header value to 0xC9, .bin header value to 0x88
	// newserv sets both to 0x00
	// sylverant appears to set it differently per quest, the logic/reasoning behind it is unknown to me
	// ... so, this value is probably unimportant?
	uint8_t pkt_flags;

	uint16_t pkt_size;
	char name[32];
	uint16_t unused;

	// khyller sets .dat header value to 0x02, .bin header value to 0x00
	// newserv sets both to 0x02
	// sylverant sets both to 0x00
	// ... and so, this value is also probably unimportant?
	uint16_t flags;

	char filename[16];
	uint32_t size;
} QST_HEADER;

int generate_qst_header(const char *src_file, size_t src_file_size, QUEST_BIN_HEADER *bin_header, QST_HEADER *out_header);

#endif