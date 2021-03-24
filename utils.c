#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <malloc.h>

#include "utils.h"
#include "retvals.h"

// from error codes defined in retvals.h
static const char *error_messages[] = {
		"No error",                    // SUCCESS
		"Invalid parameter(s)",        // ERROR_INVALID_PARAMS
		"File not found",              // ERROR_FILE_NOT_FOUND
		"Cannot create file",          // ERROR_CREATING_FILE
		"Bad data",                    // ERROR_BAD_DATA
		"I/O error",                   // ERROR_IO
		NULL
};

int read_file(const char *filename, uint8_t** out_file_data, uint32_t *out_file_size) {
	if (!filename || !out_file_size || !out_file_data)
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

int write_file(const char *filename, const uint8_t *data, size_t size) {
	if (!filename || !data || size == 0)
		return ERROR_INVALID_PARAMS;

	FILE *fp = fopen(filename, "wb");
	if (!fp)
		return ERROR_CREATING_FILE;

	int bytes_written = fwrite(data, 1, size, fp);
	if (bytes_written != size) {
		fclose(fp);
		return ERROR_IO;
	}

	fclose(fp);
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

const char* get_error_message(int retvals_error_code) {
	retvals_error_code = abs(retvals_error_code);

	int max_error_index;
	for (max_error_index = 0; error_messages[max_error_index]; ++max_error_index) {}
	max_error_index = max_error_index;

	if (retvals_error_code >= max_error_index)
		return "Unknown error";
	else
		return error_messages[retvals_error_code];
}
