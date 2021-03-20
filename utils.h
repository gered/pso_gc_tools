#ifndef UTILS_H_INCLUDED
#define UTILS_H_INCLUDED

#include <stdint.h>

#define SUCCESS                        0
#define ERROR_INVALID_PARAMS           1
#define ERROR_FILE_NOT_FOUND           2
#define ERROR_CREATING_FILE            3

int read_file(const char *filename, uint8_t** out_file_data, uint32_t *out_file_size);
int get_filesize(const char *filename, size_t *out_size);
const char* path_to_filename(const char *path);
char* append_string(const char *a, const char *b);

#endif
