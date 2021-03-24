/*
 * PSO EP1&2 (Gamecube) .qst Header Generator Tool
 *
 * Given a set of input .bin/.dat quest files, this will automatically generate .hdr files for each appropriate for
 * a .qst file containing these .bin/.dat files.
 *
 * This tool was originally made to supplement the "qst_tool" found here https://github.com/Sylverant/pso_tools
 * which has somewhat primitive support for automatically generating .qst header information.
 *
 * Gered King, March 2021
 */

#include <stdio.h>
#include <stdint.h>
#include <malloc.h>
#include <string.h>

#include <sylverant/prs.h>

#include "utils.h"
#include "quests.h"
#include "textconv.h"

int main(int argc, char *argv[]) {
	int returncode, validation_result;
	uint8_t *bin_data = NULL;
	uint8_t *dat_data = NULL;
	char *bin_hdr_file = NULL;
	char *dat_hdr_file = NULL;

	if (argc != 3) {
		printf("Usage: gen_qst_header quest.bin quest.dat\n");
		return 1;
	}

	const char *bin_file = argv[1];
	const char *dat_file = argv[2];

	const char *bin_base_filename = path_to_filename(bin_file);
	if (strlen(bin_base_filename) > QUEST_FILENAME_MAX_LENGTH) {
		printf("Bin filename is too long to fit in a QST header. Maximum length is 16 including file extension.\n");
		goto error;
	}
	const char *dat_base_filename = path_to_filename(dat_file);
	if (strlen(dat_base_filename) > QUEST_FILENAME_MAX_LENGTH) {
		printf("Dat filename is too long to fit in a QST header. Maximum length is 16 including file extension.\n");
		goto error;
	}

	size_t bin_compressed_size, dat_compressed_size;

	returncode = get_filesize(bin_file, &bin_compressed_size);
	if (returncode) {
		printf("Error code %d (%s) getting size of bin file: %s\n", returncode, get_error_message(returncode), bin_file);
		goto error;
	}
	returncode = get_filesize(dat_file, &dat_compressed_size);
	if (returncode) {
		printf("Error code %d (%s) getting size of dat file: %s\n", returncode, get_error_message(returncode), dat_file);
		goto error;
	}


	size_t bin_decompressed_size = prs_decompress_file(bin_file, &bin_data);
	if (bin_decompressed_size < 0) {
		printf("Error opening and decompressing bin file: %s\n", bin_file);
		goto error;
	}

	size_t dat_decompressed_size = prs_decompress_file(dat_file, &dat_data);
	if (dat_decompressed_size < 0) {
		printf("Error opening and decompressing dat file: %s\n", dat_file);
		goto error;
	}


	QUEST_BIN_HEADER *bin_header = (QUEST_BIN_HEADER*)bin_data;
	validation_result = validate_quest_bin(bin_header, bin_decompressed_size, true);
	if (validation_result) {
		printf("Aborting due to invalid quest .bin data.\n");
		goto error;
	}

	//sjis_to_utf8(bin_header->name, sizeof(bin_header->name));
	//sjis_to_utf8(bin_header->short_description, sizeof(bin_header->short_description));
	//sjis_to_utf8(bin_header->long_description, sizeof(bin_header->long_description));

	printf("Quest: id=%d (%d), episode=%d, download=%d, unknown=0x%02x, name=\"%s\", compressed_bin_size=%ld, compressed_dat_size=%ld\n",
	       bin_header->quest_number_byte,
	       bin_header->quest_number_word,
	       bin_header->episode+1,
	       bin_header->download,
	       bin_header->unknown,
	       bin_header->name,
	       bin_compressed_size,
	       dat_compressed_size);


	QST_HEADER qst_bin_header, qst_dat_header;
	generate_qst_header(bin_base_filename, bin_compressed_size, bin_header, &qst_bin_header);
	generate_qst_header(dat_base_filename, dat_compressed_size, bin_header, &qst_dat_header);

	bin_hdr_file = append_string(bin_file, ".hdr");
	dat_hdr_file = append_string(dat_file, ".hdr");

	returncode = write_file(bin_hdr_file, &qst_bin_header, sizeof(QST_HEADER));
	if (returncode) {
		printf("Error code %d (%s) writing out bin header file: %s\n", returncode, get_error_message(returncode), bin_hdr_file);
		goto error;
	}

	returncode = write_file(dat_hdr_file, &qst_dat_header, sizeof(QST_HEADER));
	if (returncode) {
		printf("Error code %d (%s) writing out dat header file: %s\n", returncode, get_error_message(returncode), dat_hdr_file);
		goto error;
	}


	returncode = 0;
	goto quit;
error:
	returncode = 1;
quit:
	free(bin_hdr_file);
	free(dat_hdr_file);
	free(bin_data);
	free(dat_data);
	return returncode;
}
