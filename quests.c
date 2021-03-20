#include <stdio.h>
#include <stdint.h>
#include <string.h>

#include "utils.h"
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

	memcpy(out_header->name, bin_header->name, sizeof(out_header->name));
	memcpy(out_header->filename, src_file, strlen(src_file));

	return SUCCESS;
}
