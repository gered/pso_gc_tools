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
