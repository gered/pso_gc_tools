#ifndef UTILS_H_INCLUDED
#define UTILS_H_INCLUDED

#include <stdint.h>

#include "retvals.h"

int read_file(const char *filename, uint8_t** out_file_data, uint32_t *out_file_size);
int write_file(const char *filename, const uint8_t *data, size_t size);
int get_filesize(const char *filename, size_t *out_size);
const char* path_to_filename(const char *path);
char* append_string(const char *a, const char *b);

const char* get_error_message(int retvals_error_code);

#endif
