#include <stdio.h>
#include <stdint.h>
#include <malloc.h>
#include <string.h>

#include <iconv.h>
#include <sylverant/prs.h>

#include "utils.h"

typedef struct __attribute__((packed)) {
	uint32_t object_code_offset;
	uint32_t function_offset_table_offset;
	uint32_t bin_size;
	uint32_t xffffffff;
	uint16_t language;
	uint16_t quest_number;
	char name[32];
	char short_description[128];
	char long_description[288];
} QUEST_BIN_HEADER;

typedef struct __attribute__((packed)) {
	uint8_t pkt_id;
	uint8_t pkt_flags;
	uint16_t pkt_size;
	char name[32];
	uint16_t unused;
	uint16_t flags;
	char filename[16];
	uint32_t size;
} QST_HEADER;

int sjis_to_utf8(char *s, size_t length) {
	iconv_t conv;
	size_t in_size, out_size;

	char *outbuf = malloc(length);

	in_size = length;
	out_size = length;
	char *in = s;
	char *out = outbuf;
	conv = iconv_open("SHIFT_JIS", "UTF-8");
	iconv(conv, &in, &in_size, &out, &out_size);
	iconv_close(conv);

	memset(s, 0, length);
	memcpy(s, outbuf, length);
	free(outbuf);

	return 0;
}

void generate_qst_header(const char *src_file, size_t src_file_size, QUEST_BIN_HEADER *bin_header, QST_HEADER *out_header) {
	memset(out_header, 0, sizeof(QST_HEADER));

	// 0xA6 = download to memcard, 0x44 = download for online play
	// (quest file data chunks must then be encoded accordingly. 0xA6 = use 0xA7, and 0x44 = use 0x13)
	out_header->pkt_id = 0xa6;
	out_header->pkt_size = sizeof(QST_HEADER);

	// khyller sets .dat header value to 0xC9, .bin header value to 0x88
	// newserv sets both to 0x00
	// sylverant appears to set it differently per quest, logic/reasoning is unknown to me
	// ... so, this value is probably unimportant
	out_header->pkt_flags = 0;

	// khyller sets .dat header value to 0x02, .bin header value to 0x00
	// newserv sets both to 0x02
	// sylverant sets both to 0x00
	// ... and so, this value is also probably unimportant
	out_header->flags = 0;

	out_header->size = src_file_size;

	memcpy(out_header->name, bin_header->name, sizeof(out_header->name));
	memcpy(out_header->filename, src_file, strlen(src_file));
}

int write_qst_header(const char *src_file, QST_HEADER *header) {
	char *header_file = append_string(src_file, ".hdr");

	FILE *fp = fopen(header_file, "wb");
	if (!fp) {
		printf("Error creating header file: %s\n", header_file);
		free(header_file);
		return 1;
	}

	fwrite(header, sizeof(QST_HEADER), 1, fp);
	fclose(fp);

	free(header_file);
}

int main(int argc, char *argv[]) {
	if (argc != 3) {
		printf("Usage: gen_qst_header quest.bin quest.dat\n");
		return 1;
	}

	const char *bin_file = argv[1];
	const char *dat_file = argv[2];

	const char *bin_base_filename = path_to_filename(bin_file);
	if (strlen(bin_base_filename) > 16) {
		printf("Bin filename is too long to fit in a QST header. Maximum length is 16 including file extension.\n");
		return 1;
	}

	const char *dat_base_filename = path_to_filename(dat_file);
	if (strlen(dat_base_filename) > 16) {
		printf("Dat filename is too long to fit in a QST header. Maximum length is 16 including file extension.\n");
		return 1;
	}

	size_t bin_compressed_size, dat_compressed_size;

	if (get_filesize(bin_file, &bin_compressed_size)) {
		printf("Error getting size of bin file: %s\n", bin_file);
		return 1;
	}
	if (get_filesize(dat_file, &dat_compressed_size)) {
		printf("Error getting size of dat file: %s\n", dat_file);
		return 1;
	}

	uint8_t *bin_data;
	size_t bin_decompressed_size = prs_decompress_file(bin_file, &bin_data);
	if (bin_decompressed_size < 0) {
		printf("Error opening and decompressing bin file: %s\n", bin_file);
		return 1;
	}

	uint8_t *dat_data;
	size_t dat_decompressed_size = prs_decompress_file(dat_file, &dat_data);
	if (dat_decompressed_size < 0) {
		printf("Error opening and decompressing dat file: %s\n", dat_file);
		return 1;
	}


	QUEST_BIN_HEADER *bin_header = (QUEST_BIN_HEADER*)bin_data;

	sjis_to_utf8(bin_header->name, sizeof(bin_header->name));
	sjis_to_utf8(bin_header->short_description, sizeof(bin_header->short_description));
	sjis_to_utf8(bin_header->long_description, sizeof(bin_header->long_description));

	if (bin_header->object_code_offset != 468) {
		printf("Quest bin file invalid (unexpected object_code_offset = %d).\n", bin_header->object_code_offset);
		return 1;
	}
	if (bin_header->bin_size != bin_decompressed_size) {
		printf("Quest bin file invalid (decompressed size does not match bin_size value: %d).\n", bin_header->bin_size);
		return 1;
	}
	if (strlen(bin_header->name) == 0) {
		printf("Quest bin file invalid or missing quest name.\n");
		return 1;
	}
	if (bin_header->quest_number == 0) {
		printf("Quest bin file invalid (quest_number is zero?).\n");
		return 1;
	}

	printf("Quest: id=%d, language=0x%04x, name=%s\n", bin_header->quest_number, bin_header->language, bin_header->name);

	QST_HEADER qst_bin_header, qst_dat_header;
	generate_qst_header(bin_base_filename, bin_compressed_size, bin_header, &qst_bin_header);
	generate_qst_header(dat_base_filename, dat_compressed_size, bin_header, &qst_dat_header);

	if (write_qst_header(bin_file, &qst_bin_header)) {
		return 1;
	}
	if (write_qst_header(dat_file, &qst_dat_header)) {
		return 1;
	}

	free(bin_data);
	free(dat_data);

	return 0;
}
