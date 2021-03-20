#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <malloc.h>

void* read_file(const char *filename, uint32_t *out_file_size) {
	if (!out_file_size)
		return NULL;

	FILE *fp = fopen(filename, "rb");
	if (!fp)
		return NULL;

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

	return result;
}

int get_filesize(const char *filename, size_t *out_size) {
	FILE *fp = fopen(filename, "rb");
	if (!fp)
		return 1;

	fseek(fp, 0, SEEK_END);
	*out_size = ftell(fp);
	fclose(fp);

	return 0;
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
