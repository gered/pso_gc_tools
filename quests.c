#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <malloc.h>

#include "retvals.h"
#include "quests.h"

int generate_qst_header(const char *src_file, size_t src_file_size, const QUEST_BIN_HEADER *bin_header, QST_HEADER *out_header) {
	if (!src_file || !bin_header || !out_header)
		return ERROR_INVALID_PARAMS;

	memset(out_header, 0, sizeof(QST_HEADER));

	out_header->pkt_id = PACKET_ID_QUEST_INFO_DOWNLOAD;
	out_header->pkt_size = sizeof(QST_HEADER);
	out_header->pkt_flags = 0;
	out_header->flags = 0;
	out_header->size = src_file_size;

	strncpy(out_header->name, bin_header->name, strlen(bin_header->name));
	strncpy(out_header->filename, src_file, strlen(src_file));

	return SUCCESS;
}

int generate_qst_data_chunk(const char *base_filename, uint8_t counter, const uint8_t *src, uint32_t size, QST_DATA_CHUNK *out_chunk) {
	if (!base_filename || !src || !out_chunk)
		return ERROR_INVALID_PARAMS;

	memset(out_chunk, 0, sizeof(QST_DATA_CHUNK));

	out_chunk->pkt_id = PACKET_ID_QUEST_CHUNK_DOWNLOAD;
	out_chunk->pkt_flags = counter;
	out_chunk->pkt_size = sizeof(QST_DATA_CHUNK);
	strncpy(out_chunk->filename, base_filename, sizeof(out_chunk->filename));
	memcpy(out_chunk->data, src, size);
	out_chunk->size = size;

	return SUCCESS;
}

int validate_quest_bin(const QUEST_BIN_HEADER *header, uint32_t length, bool print_errors) {
	int result = 0;

	// TODO: validations might need tweaking ...
	if (header->object_code_offset != 468) {
		if (print_errors)
			printf("Quest bin file issue: unexpected object_code_offset = %d\n", header->object_code_offset);
		result |= QUESTBIN_ERROR_OBJECT_CODE_OFFSET;
	}
	if (header->bin_size < length) {
		if (print_errors)
			printf("Quest bin file issue: bin_size %d is smaller than the actual decompressed bin size %d\n", header->bin_size, length);
		result |= QUESTBIN_ERROR_SMALLER_BIN_SIZE;
	} else if (header->bin_size > length) {
		if (print_errors)
			printf("Quest bin file issue: bin_size %d is larger than the actual decompressed bin size %d\n", header->bin_size, length);
		result |= QUESTBIN_ERROR_LARGER_BIN_SIZE;
	}
	if (strlen(header->name) == 0) {
		if (print_errors)
			printf("Quest bin file issue: blank quest name\n");
		result |= QUESTBIN_ERROR_NAME;
	}
	if (header->episode > 1) {
		if (print_errors)
			printf("Quest bin file issue: unexpected episode value %d, quest was probably created using a 16-bit quest_number\n", header->episode);
		result |= QUESTBIN_ERROR_EPISODE;
	}

	return result;
}

int validate_quest_dat(const uint8_t *data, uint32_t length, bool print_errors) {
	int result = 0;
	int table_index = 0;

	// TODO: validations might need tweaking ...
	uint32_t offset = 0;
	while (offset < length) {
		QUEST_DAT_TABLE_HEADER *table_header = (QUEST_DAT_TABLE_HEADER*)(data + offset);

		if (table_header->type > 5) {
			if (print_errors)
				printf("Quest dat file issue: invalid table type value %d found in table index %d\n", table_header->type, table_index);
			result |= QUESTDAT_ERROR_TYPE;
		}
		if (table_header->type == 0 &&
		    table_header->table_size == 0 &&
		    table_header->area == 0 &&
		    table_header->table_body_size == 0) {
			if ((offset + sizeof(QUEST_DAT_TABLE_HEADER)) == length) {
				// ignore this case ... this empty table is used to mark EOF apparently
			} else {
				if (print_errors)
					printf("Quest dat file issue: empty table encountered at table index %d with %d bytes left in file. treating this as early EOF\n", table_index, length - offset);
				result |= QUESTDAT_ERROR_PREMATURE_EOF;
				break;
			}

		} else if (table_header->table_size == (table_header->table_body_size - sizeof(QUEST_DAT_TABLE_HEADER))) {
			if (print_errors)
				printf("Quest dat file issue: mismatching table_size (%d) and table_body_size (%d) found in table index %d\n",
				       table_header->table_size,
				       table_header->table_body_size,
				       table_index);
			result |= QUESTDAT_ERROR_TABLE_BODY_SIZE;
		}

		offset += sizeof(QUEST_DAT_TABLE_HEADER);
		offset += table_header->table_body_size;
		++table_index;
	}

	return result;
}

// HACK: this function applies some arguably shitty hack-fixes under certain circumstances.
int handle_quest_bin_validation_issues(int bin_validation_result, QUEST_BIN_HEADER *bin_header, uint8_t **decompressed_bin_data, size_t *decompressed_bin_length) {
	// this hacky fix _probably_ isn't so bad. in these cases, the extra data sitting in the decompressed memory seems
	// to just be repeated subsets of the previous "good" data. almost as if the PRS decompression was stuck in a loop
	// that it eventually worked itself out of. just a wild guess though ...
	if (bin_validation_result & QUESTBIN_ERROR_SMALLER_BIN_SIZE) {
		bin_validation_result &= ~QUESTBIN_ERROR_SMALLER_BIN_SIZE;
		printf("WARNING: Decompressed .bin data is larger than expected. Proceeding using the smaller .bin header bin_size value ...\n");
		*decompressed_bin_length = bin_header->bin_size;
	}

	// this hacky fix is _probably_ not too bad either, but might have more potential for breaking things than the
	// above hack fix. maybe. i also think this is a result of some PRS decompression bug (or maybe a PRS compression
	// bug? since i believe the decompression implementation is based on game code disassembly, but most (all?) of the
	// PRS-compression implementations are based on the fuzziqer implementation which he coded himself instead of it
	// being based on game code disassembly?) ... who knows!
	if (bin_validation_result & QUESTBIN_ERROR_LARGER_BIN_SIZE) {
		bin_validation_result &= ~QUESTBIN_ERROR_LARGER_BIN_SIZE;
		if ((*decompressed_bin_length + 1) == bin_header->bin_size) {
			printf("WARNING: Decompressed .bin data is 1 byte smaller than the .bin header bin_size specifies. Correcting by adding a null byte ...\n");
			size_t length = *decompressed_bin_length + 1;
			uint8_t *new_bin_data;
			new_bin_data = realloc(*decompressed_bin_data, length);
			new_bin_data[length - 1] = 0;
			*decompressed_bin_data = new_bin_data;
			*decompressed_bin_length = length;
		}
	}
	if (bin_validation_result & QUESTBIN_ERROR_EPISODE) {
		bin_validation_result &= ~QUESTBIN_ERROR_EPISODE;
		printf("WARNING: .bin header episode value should be ignored due to apparent 16-bit quest_number value\n");
	}

	return bin_validation_result;
}

int handle_quest_dat_validation_issues(int dat_validation_result, uint8_t **decompressed_dat_data, size_t *decompressed_dat_length) {
	// this one is a bit more annoying. the quest .dat format does not have any explicit value anywhere that tells you
	// how large the entire data should be. so we have to guess. from what i can piece together, .dat files normally
	// have a table with all zeros located at the end of the file (therefore, the last 16 bytes of an uncompressed .dat
	// file should all be zero). in the cases where i have seen what looks like an early zero table in a .dat file, if
	// i let the process of walking through the file continue, the subsequent tables all look like garbage with random
	// values. so i am guessing that this is also a result of PRS compression/decompression issues ...
	if (dat_validation_result & QUESTDAT_ERROR_PREMATURE_EOF) {
		dat_validation_result &= ~QUESTDAT_ERROR_PREMATURE_EOF;
		printf("WARNING: .dat file appeared to end early (found zero-length table before end of file was reached). Decompressed .dat data might be too large? Ignoring.\n");
	}

	return dat_validation_result;
}

void print_quick_quest_info(QUEST_BIN_HEADER *bin_header, size_t compressed_bin_size, size_t compressed_dat_size) {
	printf("Quest: id=%d (%d, 0x%04x), episode=%d (0x%02x), download=%d, unknown=0x%02x, name=\"%s\"\n",
	       bin_header->quest_number_byte,
	       bin_header->quest_number_word,
	       bin_header->quest_number_word,
	       bin_header->episode + 1,
	       bin_header->episode,
	       bin_header->download,
	       bin_header->unknown,
	       bin_header->name);
	printf("       compressed_bin_size=%ld, compressed_dat_size=%ld\n", compressed_bin_size, compressed_dat_size);
}
