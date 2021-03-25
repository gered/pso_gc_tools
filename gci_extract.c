/*
 * Unencrypted PRS-compressed GCI Download Quest Extractor Tool
 *
 * This tool is specifically made to extract Gamecube PSO quest .bin/.dat files from GCI download quests memory card
 * files generated using the "Decryption Key Saver" Action Replay code created by Ralf at the gc-forever forums.
 * However, this tool currently assumes the quest data has been pre-decrypted using the embedded decryption key.
 *
 * https://www.gc-forever.com/forums/viewtopic.php?f=38&t=2050&start=75
 *
 * To clarify: this tool can extract quest .bin/.dat file data from the quests available for download from the linked
 * thread ONLY if they are indicated to be "unencrypted PRS compressed quests." This tool will NOT currently work with
 * the quest downloads indicated to be "encrypted quests w/ embedded decryption key."
 *
 * A future update to this tool will likely include decryption capability. Maybe? :-)
 *
 * Gered King, March 2021
 */

#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <malloc.h>

#include <sylverant/encryption.h>
#include <sylverant/prs.h>
#include "fuzziqer_prs.h"

#include "defs.h"

#include "quests.h"
#include "utils.h"

#define ENDIAN_SWAP_32(x) ( (((x) >> 24) & 0x000000FF) | \
                            (((x) >>  8) & 0x0000FF00) | \
                            (((x) <<  8) & 0x00FF0000) | \
                            (((x) << 24) & 0xFF000000) )


// copied from https://github.com/suloku/gcmm/blob/master/source/gci.h
typedef struct _PACKED_ {
	uint8_t gamecode[4];
	uint8_t company[2];
	uint8_t reserved01;    /*** Always 0xff ***/
	uint8_t banner_fmt;
	uint8_t filename[32];
	uint32_t time;
	uint32_t icon_addr;  /*** Offset to banner/icon data ***/
	uint16_t icon_fmt;
	uint16_t icon_speed;
	uint8_t unknown1;    /*** Permission key ***/
	uint8_t unknown2;    /*** Copy Counter ***/
	uint16_t index;        /*** Start block of savegame in memory card (Ignore - and throw away) ***/
	uint16_t filesize8;    /*** File size / 8192 ***/
	uint16_t reserved02;    /*** Always 0xffff ***/
	uint32_t comment_addr;
} GCI;

typedef struct _PACKED_ {
	GCI gci_header;
	uint8_t card_file_header[0x2040];  // big area containing the icon and such other things. ignored

	// this is stored in big-endian format in the original card data. we will convert right after loading...
	// this size value indicates the size of the quest data. it DOES NOT include the size value itself, 'nor
	// the subsequent "unknown" bytes (which we are not interested in and will be skipping during load)
	uint32_t size;

	uint32_t unknown1;
	uint8_t unknown2[16];
} GCI_DECRYPTED_DLQUEST_HEADER;

int get_quest_data(const char *filename, uint8_t **dest, uint32_t *dest_size, GCI_DECRYPTED_DLQUEST_HEADER *header) {
	if (!filename || !dest || !dest_size)
		return ERROR_INVALID_PARAMS;

	FILE *fp = fopen(filename, "rb");
	if (!fp)
		return ERROR_FILE_NOT_FOUND;

	int bytes_read;

	bytes_read = fread(header, 1, sizeof(GCI_DECRYPTED_DLQUEST_HEADER), fp);
	if (bytes_read != sizeof(GCI_DECRYPTED_DLQUEST_HEADER)) {
		fclose(fp);
		return ERROR_BAD_DATA;
	}

	// think this is all the game codes we could encounter ... ?
	if (memcmp("GPOJ", header->gci_header.gamecode, 4) &&
	    memcmp("GPOE", header->gci_header.gamecode, 4) &&
	    memcmp("GPOP", header->gci_header.gamecode, 4)) {
		fclose(fp);
		return ERROR_BAD_DATA;
	}

	if (memcmp("8P", header->gci_header.company, 2)) {
		fclose(fp);
		return ERROR_BAD_DATA;
	}

	if (!header->size) {
		fclose(fp);
		return ERROR_BAD_DATA;
	}

	header->size = ENDIAN_SWAP_32(header->size);
	uint32_t quest_data_size = header->size - sizeof(header->unknown1);
	uint8_t *data = malloc(quest_data_size);
	bytes_read = fread(data, 1, quest_data_size, fp);
	if (bytes_read != quest_data_size) {
		fclose(fp);
		free(data);
		return ERROR_BAD_DATA;
	}

	fclose(fp);
	*dest = data;
	*dest_size = quest_data_size;

	return SUCCESS;
}

int main(int argc, char *argv[]) {
	int returncode, validation_result;
	int32_t result;
	uint8_t *bin_data = NULL;
	uint8_t *dat_data = NULL;
	uint8_t *decompressed_bin_data = NULL;
	uint8_t *decompressed_dat_data = NULL;
	uint32_t bin_data_size, dat_data_size;
	size_t decompressed_bin_size, decompressed_dat_size;
	char out_filename[FILENAME_MAX];

	if (argc != 3 && argc != 5) {
		printf("Usage: gci quest-bin.gci quest-dat.gci [output.bin] [output.dat]\n");
		return 1;
	}

	const char *bin_gci_filename = argv[1];
	const char *dat_gci_filename = argv[2];
	const char *out_bin_filename = (argc == 5 ? argv[3] : NULL);
	const char *out_dat_filename = (argc == 5 ? argv[4] : NULL);

	/** extract quest .bin and .dat files from pre-decrypted GCI files **/

	printf("Reading quest .bin data from %s ...\n", bin_gci_filename);
	GCI_DECRYPTED_DLQUEST_HEADER bin_gci_header;
	result = get_quest_data(bin_gci_filename, &bin_data, &bin_data_size, &bin_gci_header);
	if (result) {
		printf("Error code %d reading quest .bin data: %s\n", result, get_error_message(result));
		goto error;
	}

	printf("Reading quest .dat data from %s ...\n", dat_gci_filename);
	GCI_DECRYPTED_DLQUEST_HEADER dat_gci_header;
	result = get_quest_data(dat_gci_filename, &dat_data, &dat_data_size, &dat_gci_header);
	if (result) {
		printf("Error code %d reading quest .dat data: %s\n", result, get_error_message(result));
		goto error;
	}


	/** decompress loaded quest .bin data and validate it **/
	printf("Validating quest .bin data ...\n");

	//result = prs_decompress_buf(bin_data, &decompressed_bin_data, bin_data_size);
	result = fuzziqer_prs_decompress_buf(bin_data, &decompressed_bin_data, bin_data_size);
	if (result < 0) {
		printf("Error code %d decompressing .bin data.\n", result);
		goto error;
	}
	decompressed_bin_size = result;

	QUEST_BIN_HEADER *bin_header = (QUEST_BIN_HEADER*)decompressed_bin_data;
	validation_result = validate_quest_bin(bin_header, decompressed_bin_size, true);
	validation_result = handle_quest_bin_validation_issues(validation_result, bin_header, &decompressed_bin_data, &decompressed_bin_size);
	if (validation_result) {
		printf("Aborting due to invalid quest .bin data.\n");
		goto error;
	}


	/** decompress loaded quest .dat data and validate it. this decompressed data is not used otherwise **/
	printf("Validating quest .dat data ...\n");

	result = prs_decompress_buf(dat_data, &decompressed_dat_data, dat_data_size);
	if (result < 0) {
		printf("Error code %d decompressing .dat data.\n", result);
		goto error;
	}
	decompressed_dat_size = result;

	validation_result = validate_quest_dat(decompressed_dat_data, decompressed_dat_size, true);
	if (validation_result) {
		printf("Aborting due to invalid quest .dat data.\n");
		goto error;
	}


	printf("Quest: id=%d (%d), episode=%d, download=%d, unknown=0x%02x, name=\"%s\", compressed_bin_size=%d, compressed_dat_size=%d\n",
	       bin_header->quest_number_byte,
	       bin_header->quest_number_word,
	       bin_header->episode+1,
	       bin_header->download,
	       bin_header->unknown,
	       bin_header->name,
	       bin_data_size,
	       dat_data_size);


	/** clear "download" flag from .bin data and re-compress **/
	printf("Clearing .bin header 'download' flag and re-compressing ...\n");

	// we are clearing this here because this is normally how you would want this .bin file to be. this way it is
	// suitable as-is for use in online-play with a server. the .bin file needs to be specially prepared for use
	// as a downloadable quest anyway (see bindat_to_gcdl), and that process can (should) turn this flag back on.
	bin_header->download = 0;

	uint8_t *recompressed_bin = NULL;
	// note: see header comment in fuzziqer_prs.c for explanation on why this is used instead of prs_compress()
	result = fuzziqer_prs_compress(decompressed_bin_data, &recompressed_bin, decompressed_bin_size);
	if (result < 0) {
		printf("Error code %d re-compressing .bin file data.\n", result);
		goto error;
	}

	// overwrite old compressed bin data, since we don't need it anymore
	free(bin_data);
	bin_data = recompressed_bin;
	bin_data_size = (uint32_t)result;


	/** write out .bin data file **/

	if (out_bin_filename)
		strncpy(out_filename, out_bin_filename, FILENAME_MAX-1);
	else
		snprintf(out_filename, FILENAME_MAX-1, "q%05d.bin", bin_header->quest_number_word);

	printf("Writing compressed quest .bin data to %s ...\n", out_filename);
	result = write_file(out_filename, bin_data, bin_data_size);
	if (result) {
		printf("Error code %d writing out file: %s\n", result, get_error_message(result));
		goto error;
	}


	/** write out .dat data file **/

	if (out_dat_filename)
		strncpy(out_filename, out_dat_filename, FILENAME_MAX-1);
	else
		snprintf(out_filename, FILENAME_MAX-1, "q%05d.dat", bin_header->quest_number_word);

	printf("Writing compressed quest .dat data to %s ...\n", out_filename);
	result = write_file(out_filename, dat_data, dat_data_size);
	if (result) {
		printf("Error code %d writing out file: %s\n", result, get_error_message(result));
		goto error;
	}


	printf("Success!\n");

	returncode = 0;
	goto quit;
error:
	returncode = 1;
quit:
	free(bin_data);
	free(dat_data);
	free(decompressed_dat_data);
	free(decompressed_bin_data);
	return returncode;
}
