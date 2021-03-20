#include <stdio.h>
#include <stdint.h>
#include <malloc.h>
#include <string.h>

#include <sylverant/prs.h>

#include "utils.h"
#include "quests.h"
#include "textconv.h"

int write_qst_header(const char *filename, const QST_HEADER *header) {
	FILE *fp = fopen(filename, "wb");
	if (!fp)
		return ERROR_CREATING_FILE;

	fwrite(header, sizeof(QST_HEADER), 1, fp);
	fclose(fp);

	return SUCCESS;
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

	char *bin_hdr_file = append_string(bin_file, ".hdr");
	char *dat_hdr_file = append_string(dat_file, ".hdr");

	if (write_qst_header(bin_hdr_file, &qst_bin_header)) {
		return 1;
	}
	if (write_qst_header(dat_hdr_file, &qst_dat_header)) {
		return 1;
	}

	free(bin_data);
	free(dat_data);

	return 0;
}
