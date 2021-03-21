#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <stdint.h>
#include <string.h>
#include <malloc.h>

#include <sylverant/encryption.h>
#include <sylverant/prs.h>

#include "quests.h"
#include "utils.h"
#include "retvals.h"

typedef struct __attribute__((packed)) {
	uint32_t decompressed_size;
	uint32_t crypt_key;
} DOWNLOAD_QUEST_CHUNKS_HEADER;

int main(int argc, char *argv[]) {
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
	if (strlen(bin_base_filename) > 16) {
		printf("Bin filename is too long to fit in a QST file header. Maximum length is 16 including file extension.\n");
		return 1;
	}

	const char *dat_base_filename = path_to_filename(dat_filename);
	if (strlen(dat_base_filename) > 16) {
		printf("Dat filename is too long to fit in a QST file header. Maximum length is 16 including file extension.\n");
		return 1;
	}


	/** read in given quest .bin and .dat files **/

	uint8_t *compressed_bin, *compressed_dat;
	uint32_t compressed_bin_size, compressed_dat_size;

	if (read_file(bin_filename, &compressed_bin, &compressed_bin_size)) {
		printf("Error reading bin file: %s\n", bin_filename);
		return 1;
	}
	if (read_file(dat_filename, &compressed_dat, &compressed_dat_size)) {
		printf("Error reading dat file: %s\n", dat_filename);
		return 1;
	}


	/** prs decompress the .bin file. store the prs decompressed data sizes for both the .bin and .dat files **/

	uint8_t *decompressed_bin;
	uint32_t decompressed_bin_size, decompressed_dat_size;

	result = prs_decompress_buf(compressed_bin, &decompressed_bin, compressed_bin_size);
	if (result < 0) {
		printf("prs_decompress_buf() error %d with bin file data: %s\n", result, bin_filename);
		return 1;
	}
	decompressed_bin_size = (uint32_t)result;

	result = prs_decompress_size(compressed_dat, compressed_dat_size);
	if (result < 0) {
		printf("prs_decompress_size() error %d with dat file data: %s\n", result, dat_filename);
		return 1;
	}
	decompressed_dat_size = (uint32_t)result;


	/** parse quest .bin header from decompressed .bin file data. also set the "download" flag in the .bin header **/

	QUEST_BIN_HEADER *bin_header = (QUEST_BIN_HEADER*)decompressed_bin;
	bin_header->download = 1;
	printf("Quest: id=%d, download=%d, language=0x%02x, name=%s\n", bin_header->quest_number, bin_header->download, bin_header->language, bin_header->name);

	// TODO: validations might need tweaking ...
	if (bin_header->object_code_offset != 468) {
		printf("Quest bin file invalid (unexpected object_code_offset = %d).\n", bin_header->object_code_offset);
		return 1;
	}
	if (bin_header->bin_size != decompressed_bin_size) {
		printf("Quest bin file invalid (decompressed size does not match header bin_size value: %d).\n", bin_header->bin_size);
		return 1;
	}
	if (strlen(bin_header->name) == 0) {
		printf("Quest bin file invalid or missing quest name.\n");
		return 1;
	}
	if (bin_header->quest_number == 0) {
		printf("Quest bin file invalid (quest_number is zero).\n");
		return 1;
	}

	/** re-compress bin data, so it includes our modified header "download" flag **/

	uint8_t *recompressed_bin;

	result = prs_compress(decompressed_bin, &recompressed_bin, decompressed_bin_size);
	if (result < 0) {
		printf("prs_compress() error %d with modified bin file data: %s\n", result, bin_filename);
		return 1;
	}

	// overwrite old compressed bin data, since we don't need it anymore
	free(compressed_bin);
	compressed_bin = recompressed_bin;
	compressed_bin_size = (uint32_t)result;


	/** encrypt compressed .bin and .dat file data, using PC crypt method with randomly generated crypt key.
	    prefix unencrypted download quest chunks header to prs compressed + encrypted .bin and .dat file data. **/

	srand(time(NULL));

	uint32_t final_bin_size = compressed_bin_size + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	uint8_t *final_bin = malloc(final_bin_size);
	memset(final_bin, 0, final_bin_size);
	uint8_t *crypt_compressed_bin = final_bin + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	DOWNLOAD_QUEST_CHUNKS_HEADER *bin_dlchunks_header = (DOWNLOAD_QUEST_CHUNKS_HEADER*)final_bin;
	bin_dlchunks_header->decompressed_size = decompressed_bin_size + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	bin_dlchunks_header->crypt_key = rand();
	memcpy(crypt_compressed_bin, compressed_bin, compressed_bin_size);

	uint32_t final_dat_size = compressed_dat_size + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	uint8_t *final_dat = malloc(final_dat_size);
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

	FILE *fp = fopen(output_qst_filename, "wb");
	if (!fp) {
		printf("Error creating output .qst file: %s\n", output_qst_filename);
		return 1;
	}

	fwrite(&qst_bin_header, sizeof(qst_bin_header), 1, fp);
	fwrite(&qst_dat_header, sizeof(qst_dat_header), 1, fp);

	uint32_t bin_pos = 0, bin_done = 0;
	uint32_t dat_pos = 0, dat_done = 0;
	uint8_t bin_counter = 0, dat_counter = 0;
	QST_DATA_CHUNK chunk;

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

	free(compressed_bin);
	free(compressed_dat);

	return 0;
}
