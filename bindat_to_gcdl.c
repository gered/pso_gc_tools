/*
 * PSO EP1&2 (Gamecube) Quest .bin/.dat File to Download/Offline .qst File Converter
 *
 * This tool will take PRS-compressed quest .bin/.dat files and process them into a working .qst file that can be
 * served up by a PSO server as a "download quest" which will be playable offline from a Gamecube memory card.
 *
 * This tool performs basically the same process that Qedit's save file type "Download Quest file(GC)" does.
 *
 * Note that .qst files created in this way cannot be used as "online" quests.
 *
 * Gered King, March 2021
 */

#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <stdint.h>
#include <string.h>
#include <malloc.h>

#include <sylverant/encryption.h>
#include <sylverant/prs.h>
#include "fuzziqer_prs.h"

#include "defs.h"

#include "quests.h"
#include "utils.h"

int main(int argc, char *argv[]) {
	int returncode, validation_result;
	uint8_t *compressed_bin = NULL;
	uint8_t *compressed_dat = NULL;
	uint8_t *decompressed_bin = NULL;
	uint8_t *decompressed_dat = NULL;
	uint8_t *final_bin = NULL;
	uint8_t *final_dat = NULL;

	if (argc != 4) {
		printf("Usage: bindat_to_gcdl quest.bin quest.dat output.qst\n");
		return 1;
	}

	int result;
	const char *bin_filename = argv[1];
	const char *dat_filename = argv[2];
	const char *output_qst_filename = argv[3];


	/** validate lengths of the given quest .bin and .dat files, to make sure they fit into the packet structs **/

	const char *bin_base_filename = path_to_filename(bin_filename);
	if (strlen(bin_base_filename) > QUEST_FILENAME_MAX_LENGTH) {
		printf("Bin filename is too long to fit in a QST file header. Maximum length is 16 including file extension.\n");
		goto error;
	}

	const char *dat_base_filename = path_to_filename(dat_filename);
	if (strlen(dat_base_filename) > QUEST_FILENAME_MAX_LENGTH) {
		printf("Dat filename is too long to fit in a QST file header. Maximum length is 16 including file extension.\n");
		goto error;
	}


	/** read in given quest .bin and .dat files **/

	uint32_t compressed_bin_size, compressed_dat_size;

	printf("Reading quest .bin file %s ...\n", bin_filename);
	returncode = read_file(bin_filename, &compressed_bin, &compressed_bin_size);
	if (returncode) {
		printf("Error code %d (%s) reading bin file: %s\n", returncode, get_error_message(returncode), bin_filename);
		goto error;
	}

	printf("Reading quest .dat file %s ...\n", dat_filename);
	returncode = read_file(dat_filename, &compressed_dat, &compressed_dat_size);
	if (returncode) {
		printf("Error code %d (%s) reading dat file: %s\n", returncode, get_error_message(returncode), dat_filename);
		goto error;
	}


	/** prs decompress the .bin file, parse out it's header and validate it **/
	printf("Decompressing and validating .bin file ...\n");

	size_t decompressed_bin_size;
	result = fuzziqer_prs_decompress_buf(compressed_bin, &decompressed_bin, compressed_bin_size);
	if (result < 0) {
		printf("Error code %d decompressing .dat data.\n", result);
		goto error;
	}
	decompressed_bin_size = result;

	QUEST_BIN_HEADER *bin_header = (QUEST_BIN_HEADER*)decompressed_bin;
	validation_result = validate_quest_bin(bin_header, decompressed_bin_size, true);
	validation_result = handle_quest_bin_validation_issues(validation_result, bin_header, &decompressed_bin, &decompressed_bin_size);
	if (validation_result) {
		printf("Aborting due to invalid quest .bin data.\n");
		goto error;
	}


	/** prs decompress the .dat file and validate it **/
	printf("Decompressing and validating .dat file ...\n");

	size_t decompressed_dat_size;
	result = fuzziqer_prs_decompress_buf(compressed_dat, &decompressed_dat, compressed_dat_size);
	if (result < 0) {
		printf("Error code %d decompressing .dat data.\n", result);
		goto error;
	}
	decompressed_dat_size = result;

	validation_result = validate_quest_dat(decompressed_dat, decompressed_dat_size, true);
	validation_result = handle_quest_dat_validation_issues(validation_result, &decompressed_dat, &decompressed_dat_size);
	if (validation_result) {
		printf("Aborting due to invalid quest .dat data.\n");
		goto error;
	}


	print_quick_quest_info(bin_header, compressed_bin_size, compressed_dat_size);


	/** set the "download" flag in the .bin header and then re-compress the .bin data **/
	printf("Setting .bin header 'download' flag and re-compressing .bin file data ...\n");

	bin_header->download = 1;  // gamecube pso client will not find quests on a memory card if this is not set!

	uint8_t *recompressed_bin;
	result = fuzziqer_prs_compress(decompressed_bin, &recompressed_bin, decompressed_bin_size);
	if (result < 0) {
		printf("Error code %d re-compressing .bin file data.\n", result);
		goto error;
	}

	// overwrite old compressed bin data, since we don't need it anymore
	free(compressed_bin);
	compressed_bin = recompressed_bin;
	compressed_bin_size = (uint32_t)result;


	/** encrypt compressed .bin and .dat file data, using PC crypt method with randomly generated crypt key.
	    prefix unencrypted download quest chunks header to prs compressed + encrypted .bin and .dat file data. **/
	printf("Preparing final .qst file data ... \n");

	srand(time(NULL));

	uint32_t final_bin_size = compressed_bin_size + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	final_bin = malloc(final_bin_size);
	memset(final_bin, 0, final_bin_size);
	uint8_t *crypt_compressed_bin = final_bin + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	DOWNLOAD_QUEST_CHUNKS_HEADER *bin_dlchunks_header = (DOWNLOAD_QUEST_CHUNKS_HEADER*)final_bin;
	bin_dlchunks_header->decompressed_size = decompressed_bin_size + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	bin_dlchunks_header->crypt_key = rand();
	memcpy(crypt_compressed_bin, compressed_bin, compressed_bin_size);

	uint32_t final_dat_size = compressed_dat_size + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	final_dat = malloc(final_dat_size);
	memset(final_dat, 0, final_dat_size);
	uint8_t *crypt_compressed_dat = final_dat + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	DOWNLOAD_QUEST_CHUNKS_HEADER *dat_dlchunks_header = (DOWNLOAD_QUEST_CHUNKS_HEADER*)final_dat;
	dat_dlchunks_header->decompressed_size = decompressed_dat_size + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	dat_dlchunks_header->crypt_key = rand();
	memcpy(crypt_compressed_dat, compressed_dat, compressed_dat_size);

	CRYPT_SETUP bin_cs, dat_cs;

	// yes, we need to use PC encryption even for gamecube download quests
	CRYPT_CreateKeys(&bin_cs, &bin_dlchunks_header->crypt_key, CRYPT_PC);
	CRYPT_CreateKeys(&dat_cs, &dat_dlchunks_header->crypt_key, CRYPT_PC);

	// NOTE: encrypts the compressed bin/dat data in-place
	CRYPT_CryptData(&bin_cs, crypt_compressed_bin, final_bin_size - sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER), 1);
	CRYPT_CryptData(&dat_cs, crypt_compressed_dat, final_dat_size - sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER), 1);


	/** generate .qst file header for both the encrypted+compressed .bin and .dat file data, using the .bin header data **/

	QST_HEADER qst_bin_header, qst_dat_header;

	generate_qst_header(bin_base_filename, final_bin_size, bin_header, &qst_bin_header);
	generate_qst_header(dat_base_filename, final_dat_size, bin_header, &qst_dat_header);


	/** write out the .qst file. chunk data is written out as interleaved 0xA7 packets containing 1024 bytes each */
	printf("Writing out %s ...\n", output_qst_filename);

	FILE *fp = fopen(output_qst_filename, "wb");
	if (!fp) {
		printf("Error creating output .qst file: %s\n", output_qst_filename);
		goto error;
	}

	fwrite(&qst_bin_header, sizeof(qst_bin_header), 1, fp);
	fwrite(&qst_dat_header, sizeof(qst_dat_header), 1, fp);

	uint32_t bin_pos = 0, bin_done = 0;
	uint32_t dat_pos = 0, dat_done = 0;
	uint8_t bin_counter = 0, dat_counter = 0;
	QST_DATA_CHUNK chunk;

	// note: .qst files actually do NOT need to be interleaved like this to work with the gamecube pso client. the
	// khyller server did not do this. it is possible that some .qst file tools (qedit?) expect it though? so, meh,
	// we'll just do it here because it's easy enough. also worth mentioning that khyller also put the .dat file data
	// first. so the order seems unimportant too ... ?

	while (!bin_done || !dat_done) {
		if (!bin_done) {
			uint32_t size = (final_bin_size - bin_pos >= 1024) ? 1024 : (final_bin_size - bin_pos);

			generate_qst_data_chunk(bin_base_filename, bin_counter, final_bin + bin_pos, size, &chunk);
			fwrite(&chunk, sizeof(QST_DATA_CHUNK), 1, fp);

			bin_pos += size;
			++bin_counter;
			if (bin_pos >= final_bin_size)
				bin_done = 1;
		}

		if (!dat_done) {
			uint32_t size = (final_dat_size - dat_pos >= 1024) ? 1024 : (final_dat_size - dat_pos);

			generate_qst_data_chunk(dat_base_filename, dat_counter, final_dat + dat_pos, size, &chunk);
			fwrite(&chunk, sizeof(QST_DATA_CHUNK), 1, fp);

			dat_pos += size;
			++dat_counter;
			if (dat_pos >= final_dat_size)
				dat_done = 1;
		}
	}

	fclose(fp);

	returncode = 0;
	goto quit;
error:
	returncode = 1;
quit:
	free(decompressed_bin);
	free(final_bin);
	free(final_dat);
	free(compressed_bin);
	free(compressed_dat);
	return returncode;
}
