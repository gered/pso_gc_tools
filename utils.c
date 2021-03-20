#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <malloc.h>

#include "utils.h"

int read_file(const char *filename, uint8_t** out_file_data, uint32_t *out_file_size) {
	if (!out_file_size || !out_file_data)
		return ERROR_INVALID_PARAMS;

	FILE *fp = fopen(filename, "rb");
	if (!fp)
		return ERROR_FILE_NOT_FOUND;

	fseek(fp, 0, SEEK_END);
	*out_file_size = ftell(fp);
	fseek(fp, 0, SEEK_SET);

	uint8_t *result = malloc(*out_file_size);

	uint32_t read, next;
	uint8_t buffer[1024];

	next = 0;

	do {
		read = fread(buffer, 1, 1024, fp);
		if (read) {
			memcpy(&result[next], buffer, read);
			next += read;
		}
	} while (read);

	*out_file_data = result;
	return SUCCESS;
}

int get_filesize(const char *filename, size_t *out_size) {
	if (!filename || !out_size)
		return ERROR_INVALID_PARAMS;

	FILE *fp = fopen(filename, "rb");
	if (!fp)
		return ERROR_FILE_NOT_FOUND;

	fseek(fp, 0, SEEK_END);
	*out_size = ftell(fp);
	fclose(fp);

	return SUCCESS;
}

const char* path_to_filename(const char *path) {
	const char *pos = strrchr(path, '/');
	if (pos) {
		return pos+1;
	} else {
		return path;
	}
}

char* append_string(const char *a, const char *b) {
	if (!a)
		return NULL;

	char *result = malloc(strlen(a) + strlen(b));
	strcpy(result, a);
	strcat(result, b);
	return result;
}
