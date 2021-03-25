#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <malloc.h>

#include <sylverant/encryption.h>
#include "fuzziqer_prs.h"

#include "retvals.h"
#include "utils.h"
#include "quests.h"

typedef struct _PACKED_ {
	uint8_t pkt_id;
	uint8_t pkt_flags;
	uint16_t pkt_size;
} PACKET_HEADER;

#define PACKET_TYPE_ERROR  0
#define PACKET_TYPE_HEADER 1
#define PACKET_TYPE_DATA   2
#define PACKET_TYPE_EOF    4   // not really a packet type, lol

#define QST_TYPE_NONE     0
#define QST_TYPE_ONLINE   1
#define QST_TYPE_DOWNLOAD 2

const char* get_area_string(int area, int episode) {
	if (episode == 0) {
		switch (area) {
			case 0: return "Pioneer 2";
			case 1: return "Forest 1";
			case 2: return "Forest 2";
			case 3: return "Caves 1";
			case 4: return "Caves 2";
			case 5: return "Caves 3";
			case 6: return "Mines 1";
			case 7: return "Mines 2";
			case 8: return "Ruins 1";
			case 9: return "Ruins 2";
			case 10: return "Ruins 3";
			case 11: return "Under the Dome";
			case 12: return "Underground Channel";
			case 13: return "Monitor Room";
			case 14: return "????";
			case 15: return "Visual Lobby";
			case 16: return "VR Spaceship Alpha";
			case 17: return "VR Temple Alpha";
			default: return "Invalid Area";
		}
	} else if (episode == 1) {
		switch (area) {
			case 0: return "Lab";
			case 1: return "VR Temple Alpha";
			case 2: return "VR Temple Beta";
			case 3: return "VR Spaceship Alpha";
			case 4: return "VR Spaceship Beta";
			case 5: return "Central Control Area";
			case 6: return "Jungle North";
			case 7: return "Jungle East";
			case 8: return "Mountain";
			case 9: return "Seaside";
			case 10: return "Seabed Upper";
			case 11: return "Seabed Lower";
			case 12: return "Cliffs of Gal Da Val";
			case 13: return "Test Subject Disposal Area";
			case 14: return "VR Temple Final";
			case 15: return "VR Spaceship Final";
			case 16: return "Seaside Night";
			case 17: return "Control Tower";
			default: return "Invalid Area";
		}
	} else {
		return "Invalid Episode";
	}
}

void display_info(uint8_t *bin_data, size_t bin_length, uint8_t *dat_data, size_t dat_length, int qst_type) {
	int validation_result;
	int32_t result;
	uint8_t *decompressed_bin_data = NULL;
	uint8_t *decompressed_dat_data = NULL;
	size_t decompressed_bin_length, decompressed_dat_length;

	printf("Decompressing .bin data ...\n");
	result = fuzziqer_prs_decompress_buf(bin_data, &decompressed_bin_data, bin_length);
	if (result < 0) {
		printf("Error code %d decompressing .bin data.\n", result);
		goto error;
	}
	decompressed_bin_length = result;

	printf("Decompressing .dat data ...\n");
	result = fuzziqer_prs_decompress_buf(dat_data, &decompressed_dat_data, dat_length);
	if (result < 0) {
		printf("Error code %d decompressing .dat data.\n", result);
		goto error;
	}
	decompressed_dat_length = result;


	printf("Validating .bin data ...\n");
	QUEST_BIN_HEADER *bin_header = (QUEST_BIN_HEADER*)decompressed_bin_data;
	validation_result = validate_quest_bin(bin_header, decompressed_bin_length, true);
	if (validation_result == QUESTBIN_ERROR_SMALLER_BIN_SIZE) {
		printf("WARNING: Decompressed .bin data is larger than expected. Proceeding using the smaller .bin header bin_size value ...\n");
		decompressed_bin_length = bin_header->bin_size;
	} else if (validation_result == QUESTBIN_ERROR_LARGER_BIN_SIZE) {
		if ((decompressed_bin_length + 1) == bin_header->bin_size) {
			printf("WARNING: Decompressed .bin data is 1 byte smaller than the .bin header bin_size specifies. Correcting by adding a null byte ...\n");
			++decompressed_bin_length;
			decompressed_bin_data = realloc(decompressed_bin_data, decompressed_bin_length);
			decompressed_bin_data[decompressed_bin_length - 1] = 0;
		}
	} else if (validation_result) {
		printf("Aborting due to invalid quest .bin data.\n");
		goto error;
	}


	printf("Validating .dat data ...\n");
	validation_result = validate_quest_dat(decompressed_dat_data, decompressed_dat_length, true);
	if (validation_result != QUESTDAT_ERROR_EOF_EMPTY_TABLE) {
		printf("Aborting due to invalid quest .dat data.\n");
		goto error;
	}

	printf("\n\n");

	printf("QUEST FILE FORMAT: ");
	switch (qst_type) {
		case QST_TYPE_NONE: printf("raw .bin/.dat\n"); break;
		case QST_TYPE_DOWNLOAD: printf("download/offline .qst (0x%02X)\n", PACKET_ID_QUEST_INFO_DOWNLOAD); break;
		case QST_TYPE_ONLINE: printf("online .qst (0x%02X)\n", PACKET_ID_QUEST_INFO_ONLINE); break;
		default: printf("unknown\n");
	}
	printf("\n");


	printf("QUEST .BIN FILE\n");
	printf("======================================================================\n");
	printf("name:                             %s\n", bin_header->name);
	printf("download flag:                    %d\n", bin_header->download);
	printf("quest_number:                     as byte: %d    as word: %d\n", bin_header->quest_number_byte, bin_header->quest_number_word);
	printf("episode:                          %d (%d)\n", bin_header->episode, bin_header->episode + 1);
	printf("xffffffff:                        0x%08x\n", bin_header->xffffffff);
	printf("unknown:                          0x%02x\n", bin_header->unknown);
	printf("\n");
	printf("short_description:\n%s\n\n", bin_header->short_description);
	printf("long_description:\n%s\n", bin_header->long_description);
	printf("object_code_offset:               %d\n", bin_header->object_code_offset);
	printf("function_offset_table_offset:     %d\n", bin_header->function_offset_table_offset);
	printf("object_code_size:                 %d\n", (bin_header->function_offset_table_offset - bin_header->object_code_offset));
	printf("function_offset_table_size:       %d\n", (bin_header->bin_size - bin_header->function_offset_table_offset));


	printf("\n\n");
	printf("QUEST .DAT FILE\n");
	printf("======================================================================\n");

	int table_index = 0;
	uint32_t offset = 0;
	while (offset < decompressed_dat_length) {
		QUEST_DAT_TABLE_HEADER *table_header = (QUEST_DAT_TABLE_HEADER*)(decompressed_dat_data + offset);

		printf("Table index %d - ", table_index);
		switch (table_header->type) {
			case 1:
				printf("Object\n");
				printf("table_body_size:                  %d\n", table_header->table_body_size);
				printf("area:                             %s (%d)\n", get_area_string(table_header->area, bin_header->episode), table_header->area);
				printf("object count:                     %d\n", table_header->table_body_size / 68);
				break;
			case 2:
				printf("NPC\n");
				printf("table_body_size:                  %d\n", table_header->table_body_size);
				printf("area:                             %s (%d)\n", get_area_string(table_header->area, bin_header->episode), table_header->area);
				printf("npc count:                        %d\n", table_header->table_body_size / 72);
				break;
			case 3:
				printf("Wave\n");
				printf("table_body_size:                  %d\n", table_header->table_body_size);
				printf("area:                             %s (%d)\n", get_area_string(table_header->area, bin_header->episode), table_header->area);
				break;
			case 4:
				printf("Challenge Mode Spawn Points\n");
				printf("table_body_size:                  %d\n", table_header->table_body_size);
				printf("area:                             %s (%d)\n", get_area_string(table_header->area, bin_header->episode), table_header->area);
				break;
			case 5:
				printf("Challenge Mode (?)\n");
				printf("table_body_size:                  %d\n", table_header->table_body_size);
				printf("area:                             %s (%d)\n", get_area_string(table_header->area, bin_header->episode), table_header->area);
				break;
			default:
				if (table_header->type == 0 && table_header->table_size == 0 && table_header->area == 0 && table_header->table_body_size == 0)
					printf("EOF marker\n");
				else {
					printf("Unknown\n");
					printf("type:                             %d\n", table_header->type);
					printf("table_body_size:                  %d\n", table_header->table_body_size);
					printf("area:                             %d\n", table_header->area);
				}
				break;
		}
		printf("\n");

		offset += sizeof(QUEST_DAT_TABLE_HEADER);
		offset += table_header->table_body_size;
		++table_index;
	}

error:
	free(decompressed_bin_data);
	free(decompressed_dat_data);
}

int read_next_qst_packet(FILE *fp, QST_HEADER *out_header_packet, QST_DATA_CHUNK *out_data_packet) {
	size_t bytes_read;
	PACKET_HEADER packet_header;

	bytes_read = fread(&packet_header, 1, sizeof(PACKET_HEADER), fp);
	if (bytes_read == 0 && feof(fp))
		return PACKET_TYPE_EOF;
	if (bytes_read != sizeof(PACKET_HEADER))
		return PACKET_TYPE_ERROR;

	if (packet_header.pkt_size == sizeof(QST_HEADER) &&
	    (packet_header.pkt_id == PACKET_ID_QUEST_INFO_ONLINE ||
	     packet_header.pkt_id == PACKET_ID_QUEST_INFO_DOWNLOAD)) {
		memcpy(out_header_packet, &packet_header, sizeof(PACKET_HEADER));
		size_t remaining_bytes = sizeof(QST_HEADER) - sizeof(PACKET_HEADER);
		bytes_read = fread((uint8_t*)out_header_packet + sizeof(PACKET_HEADER), 1, remaining_bytes, fp);
		if (bytes_read != remaining_bytes)
			return PACKET_TYPE_ERROR;
		else
			return PACKET_TYPE_HEADER;

	} else if (packet_header.pkt_size == sizeof(QST_DATA_CHUNK) &&
	           (packet_header.pkt_id == PACKET_ID_QUEST_CHUNK_ONLINE ||
	            packet_header.pkt_id == PACKET_ID_QUEST_CHUNK_DOWNLOAD)) {
		memcpy(out_data_packet, &packet_header, sizeof(PACKET_HEADER));
		size_t remaining_bytes = sizeof(QST_DATA_CHUNK) - sizeof(PACKET_HEADER);
		bytes_read = fread((uint8_t*)out_data_packet + sizeof(PACKET_HEADER), 1, remaining_bytes, fp);
		if (bytes_read != remaining_bytes)
			return PACKET_TYPE_ERROR;
		else
			return PACKET_TYPE_DATA;

	} else
		return PACKET_TYPE_ERROR;
}

int load_quest_from_qst(const char *filename, uint8_t **out_bin_data, size_t *out_bin_length, uint8_t **out_dat_data, size_t *out_dat_length, int *out_qst_type) {
	int returncode;
	FILE *fp = NULL;
	uint8_t *bin_data = NULL;
	uint8_t *dat_data = NULL;
	int qst_type;

	fp = fopen(filename, "rb");
	if (!fp) {
		returncode = ERROR_FILE_NOT_FOUND;
		goto error;
	}

	char bin_filename[QUEST_FILENAME_MAX_LENGTH] = "";
	char dat_filename[QUEST_FILENAME_MAX_LENGTH] = "";
	size_t bin_data_length, dat_data_length;
	size_t bin_data_pos, dat_data_pos;

	while (!feof(fp)) {
		QST_HEADER header;
		QST_DATA_CHUNK data;
		int type = read_next_qst_packet(fp, &header, &data);

		if (type == PACKET_TYPE_EOF && bin_data && dat_data)
			break;

		if (type == PACKET_TYPE_ERROR) {
			returncode = ERROR_BAD_DATA;
			goto error;

		} else if (type == PACKET_TYPE_HEADER) {
			//CRYPT_PrintData(&header, sizeof(QST_HEADER));
			if (string_ends_with(header.filename, ".bin")) {
				strncpy(bin_filename, header.filename, QUEST_FILENAME_MAX_LENGTH);
				bin_data_length = header.size;
				bin_data_pos = 0;
				bin_data = malloc(bin_data_length);
			} else if (string_ends_with(header.filename, ".dat")) {
				strncpy(dat_filename, header.filename, QUEST_FILENAME_MAX_LENGTH);
				dat_data_length = header.size;
				dat_data_pos = 0;
				dat_data = malloc(dat_data_length);
			} else {
				returncode = ERROR_BAD_DATA;
				goto error;
			}

			if (header.pkt_id == PACKET_ID_QUEST_INFO_ONLINE)
				qst_type = QST_TYPE_ONLINE;
			else
				qst_type = QST_TYPE_DOWNLOAD;

		} else if (type == PACKET_TYPE_DATA) {
			//CRYPT_PrintData(&data, sizeof(QST_DATA_CHUNK));
			if (strncmp(data.filename, bin_filename, QUEST_FILENAME_MAX_LENGTH) == 0) {
				memcpy(bin_data + bin_data_pos, data.data, data.size);
				bin_data_pos += data.size;

			} else if (strncmp(data.filename, dat_filename, QUEST_FILENAME_MAX_LENGTH) == 0) {
				memcpy(dat_data + dat_data_pos, data.data, data.size);
				dat_data_pos += data.size;

			} else {
				returncode = ERROR_BAD_DATA;
				goto error;
			}
		}
	}

	fclose(fp);

	*out_bin_length = bin_data_length;
	*out_dat_length = dat_data_length;
	*out_bin_data = bin_data;
	*out_dat_data = dat_data;
	*out_qst_type = qst_type;

	return SUCCESS;

error:
	fclose(fp);
	free(bin_data);
	free(dat_data);
	return returncode;
}

int decrypt_qst_bindat(uint8_t *bin_data, size_t *bin_length, uint8_t *dat_data, size_t *dat_length) {
	DOWNLOAD_QUEST_CHUNKS_HEADER *bin_dl_header = (DOWNLOAD_QUEST_CHUNKS_HEADER*)bin_data;
	DOWNLOAD_QUEST_CHUNKS_HEADER *dat_dl_header = (DOWNLOAD_QUEST_CHUNKS_HEADER*)dat_data;

	CRYPT_SETUP bin_cs, dat_cs;
	CRYPT_CreateKeys(&bin_cs, &bin_dl_header->crypt_key, CRYPT_PC);
	CRYPT_CreateKeys(&dat_cs, &dat_dl_header->crypt_key, CRYPT_PC);

	uint8_t *actual_bin_data = bin_data + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	uint8_t *actual_dat_data = dat_data + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	size_t decrypted_bin_length = *bin_length - sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	size_t decrypted_dat_length = *dat_length - sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER);
	CRYPT_CryptData(&bin_cs, bin_data + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER), decrypted_bin_length, 0);
	CRYPT_CryptData(&dat_cs, dat_data + sizeof(DOWNLOAD_QUEST_CHUNKS_HEADER), decrypted_dat_length, 0);

	memmove(bin_data, actual_bin_data, decrypted_bin_length);
	memmove(dat_data, actual_dat_data, decrypted_dat_length);

	*bin_length = decrypted_bin_length;
	*dat_length = decrypted_dat_length;

	return SUCCESS;
}

int load_quest_from_bindat(const char *bin_filename, const char *dat_filename, uint8_t **out_bin_data, size_t *out_bin_length, uint8_t **out_dat_data, size_t *out_dat_length) {
	int returncode;
	uint8_t *bin_data = NULL;
	uint8_t *dat_data = NULL;
	uint32_t bin_data_length, dat_data_length;

	returncode = read_file(bin_filename, &bin_data, &bin_data_length);
	if (returncode)
		goto error;

	returncode = read_file(dat_filename, &dat_data, &dat_data_length);
	if (returncode)
		goto error;

	*out_bin_length = bin_data_length;
	*out_dat_length = dat_data_length;
	*out_bin_data = bin_data;
	*out_dat_data = dat_data;

	return SUCCESS;

error:
	free(bin_data);
	free(dat_data);
	return returncode;
}

int main(int argc, char *argv[]) {
	int returncode;

	if (argc != 2 && argc != 3) {
		printf("Usage: quest_info quest.bin quest.dat\n");
		printf("       quest_info quest.qst\n");
		return 1;
	}

	uint8_t *bin_data = NULL;
	uint8_t *dat_data = NULL;
	size_t bin_data_size, dat_data_size;
	int qst_type = QST_TYPE_NONE;

	if (argc == 2) {
		printf("Reading .qst file: %s\n", argv[1]);
		returncode = load_quest_from_qst(argv[1], &bin_data, &bin_data_size, &dat_data, &dat_data_size, &qst_type);
		if (returncode) {
			printf("Error code %d (%s) loading quest: %s\n", returncode, get_error_message(returncode), argv[1]);
			goto error;
		}
		if (qst_type == QST_TYPE_DOWNLOAD) {
			printf("Decrypting download .qst data ...\n");
			returncode = decrypt_qst_bindat(bin_data, &bin_data_size, dat_data, &dat_data_size);
			if (returncode) {
				printf("Error code %d (%s) while decrypting .qst contents", returncode, get_error_message(returncode));
				goto error;
			}
		}
	} else {
		printf("Reading .bin file %s and .dat file %s ... \n", argv[1], argv[2]);
		returncode = load_quest_from_bindat(argv[1], argv[2], &bin_data, &bin_data_size, &dat_data, &dat_data_size);
		if (returncode) {
			printf("Error code %d (%s) loading quest files %s and %s\n", returncode, get_error_message(returncode), argv[1], argv[2]);
			goto error;
		}
	}

	display_info(bin_data, bin_data_size, dat_data, dat_data_size, qst_type);

	returncode = 0;
	goto quit;
error:
	returncode = 1;
quit:
	free(bin_data);
	free(dat_data);
	return returncode;
}
