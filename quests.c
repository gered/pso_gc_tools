#include <stdio.h>
#include <stdint.h>
#include <string.h>

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
	if (header->quest_number_word == 0) {
		if (print_errors)
			printf("Quest bin file issue: quest_number is zero\n");
		result |= QUESTBIN_ERROR_NAME;
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
			// all zeros seems to be used to indicate end of file ???
			if ((offset + sizeof(QUEST_DAT_TABLE_HEADER)) == length) {
				if (print_errors)
					printf("Quest dat file warning: empty table encountered at end of file (probably normal?)\n");
				result |= QUESTDAT_ERROR_EOF_EMPTY_TABLE;
			} else {
				if (print_errors)
					printf("Quest dat file warning: empty table encountered at table index %d\n", table_index);
				result |= QUESTDAT_ERROR_EMPTY_TABLE;
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
