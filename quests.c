#include <stdio.h>
#include <stdint.h>
#include <string.h>

#include "retvals.h"
#include "quests.h"

int generate_qst_header(const char *src_file, size_t src_file_size, QUEST_BIN_HEADER *bin_header, QST_HEADER *out_header) {
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

int validate_quest_bin(QUEST_BIN_HEADER *header, uint32_t length) {
	// TODO: validations might need tweaking ...
	if (header->object_code_offset != 468) {
		printf("Quest bin file invalid (unexpected object_code_offset = %d).\n", header->object_code_offset);
		return 1;
	}
	if (header->bin_size != length) {
		printf("Quest bin file invalid (decompressed size does not match header bin_size value: %d).\n", header->bin_size);
		return 2;
	}
	if (strlen(header->name) == 0) {
		printf("Quest bin file invalid or missing quest name.\n");
		return 3;
	}
	if (header->quest_number == 0) {
		printf("Quest bin file invalid (quest_number is zero).\n");
		return 4;
	}

	return 0;
}

int validate_quest_dat(uint8_t *data, uint32_t length) {
	// TODO: validations might need tweaking ...
	if (!data || length == 0) {
	//	printf("Invalid")
	}

	uint32_t offset = 0;
	while (offset < length) {
		QUEST_DAT_TABLE_HEADER *table_header = (QUEST_DAT_TABLE_HEADER*)(data + offset);

		if (table_header->type > 5) {
			printf("Invalid table type value found (type = %d)\n", table_header->type);
			return 1;
		}
		if (table_header->type == 0 &&
		    table_header->table_size == 0 &&
		    table_header->area == 0 &&
		    table_header->table_body_size == 0) {
			// all zeros seems to be used to indicate end of file ???
			// just ignore this and move on ...

		} else if (table_header->table_size == (table_header->table_body_size - 16)) {
			printf("Invalid table_body_size found (table_size = %d, table_body_size = %d)\n", table_header->table_size, table_header->table_body_size);
			return 2;
		}

		offset += sizeof(QUEST_DAT_TABLE_HEADER);
		offset += table_header->table_body_size;
	}

	return 0;
}
