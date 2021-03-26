#ifndef QUESTS_H_INCLUDED
#define QUESTS_H_INCLUDED

#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>

#include "defs.h"

#define QUESTBIN_ERROR_OBJECT_CODE_OFFSET  1
#define QUESTBIN_ERROR_LARGER_BIN_SIZE     2
#define QUESTBIN_ERROR_SMALLER_BIN_SIZE    4
#define QUESTBIN_ERROR_NAME                8
#define QUESTBIN_ERROR_EPISODE             16

#define QUESTDAT_ERROR_TYPE                1
#define QUESTDAT_ERROR_TABLE_BODY_SIZE     2
#define QUESTDAT_ERROR_PREMATURE_EOF       4

#define PACKET_ID_QUEST_INFO_ONLINE    0x44
#define PACKET_ID_QUEST_INFO_DOWNLOAD  0xa6
#define PACKET_ID_QUEST_CHUNK_ONLINE   0x13
#define PACKET_ID_QUEST_CHUNK_DOWNLOAD 0xa7

#define QUEST_FILENAME_MAX_LENGTH      16

// decompressed quest .bin file header
typedef struct _PACKED_ {
	uint32_t object_code_offset;
	uint32_t function_offset_table_offset;
	uint32_t bin_size;
	uint32_t xffffffff;                // always 0xffffffff ?
	uint8_t download;                  // must be '1' to be usable as an offline quest (played from memory card)

	// have seen some projects define this field as language. "newserv" just calls it unknown? i've seen multiple
	// values present for english language quests ...
	uint8_t unknown;

	// "newserv" has these like this here, as quest_number and episode separately. most other projects that parse
	// .bin files treat quest_number as a 16-bit number. in general, i think the "episode" field as a separate byte
	// is *probably* better when dealing with non-custom quests. however, some custom quests (which are mostly of
	// dubious quality anyway) clearly were created using a tool which had quest_number as a 16-bit value ...
	// ... so .... i dunno! i guess i'll just leave it like this ...
	union {
		struct {
			uint8_t quest_number_byte;
			uint8_t episode;
		};
		struct {
			uint16_t quest_number_word;
		};
	};

	// some sources say these strings are all UTF-16LE, but i'm not sure that is really the case for gamecube data?
	// for gamecube-format quest .bin files, it instead looks like SHIFT-JIS probably ... ?

	char name[32];
	char short_description[128];
	char long_description[288];
} QUEST_BIN_HEADER;

// decompressed quest .dat file table header
typedef struct _PACKED_ {
	uint32_t type;
	uint32_t table_size;
	uint32_t area;
	uint32_t table_body_size;
} QUEST_DAT_TABLE_HEADER;

// .qst file header, for either the embedded bin or dat quest data (there should be two of these per .qst file).
typedef struct _PACKED_ {
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

	char filename[QUEST_FILENAME_MAX_LENGTH];
	uint32_t size;
} QST_HEADER;

// .qst raw .bin/.dat file data packet. the original .bin/.dat file data is broken down into as many of these structs
// as is necessary to fit into the resulting .qst file
typedef struct _PACKED_ {
	uint8_t pkt_id;
	uint8_t pkt_flags;
	uint16_t pkt_size;
	char filename[QUEST_FILENAME_MAX_LENGTH];
	uint8_t data[1024];
	uint32_t size;
} QST_DATA_CHUNK;

// for download/offline .qst files only. the raw .bin/.dat file data needs to be prefixed with one of these structs
// before being turned into QST_DATA_CHUNKs. only one of these is needed per each .bin/.dat file.
typedef struct _PACKED_ {
	uint32_t decompressed_size;
	uint32_t crypt_key;
} DOWNLOAD_QUEST_CHUNKS_HEADER;

int generate_qst_header(const char *src_file, size_t src_file_size, const QUEST_BIN_HEADER *bin_header, QST_HEADER *out_header);
int generate_qst_data_chunk(const char *base_filename, uint8_t counter, const uint8_t *src, uint32_t size, QST_DATA_CHUNK *out_chunk);
int validate_quest_bin(const QUEST_BIN_HEADER *header, uint32_t length, bool print_errors);
int validate_quest_dat(const uint8_t *data, uint32_t length, bool print_errors);
int handle_quest_bin_validation_issues(int bin_validation_result, QUEST_BIN_HEADER *bin_header, uint8_t **decompressed_bin_data, size_t *decompressed_bin_length);
int handle_quest_dat_validation_issues(int dat_validation_result, uint8_t **decompressed_dat_data, size_t *decompressed_dat_length);
void print_quick_quest_info(QUEST_BIN_HEADER *bin_header, size_t compressed_bin_size, size_t compressed_dat_size);

#endif
