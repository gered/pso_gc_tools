#ifndef PRS_H_INCLUDED
#define PRS_H_INCLUDED

#include <stdint.h>

int fuzziqer_prs_compress(const uint8_t *src, uint8_t **dst, size_t src_len);
int fuzziqer_prs_decompress_buf(const uint8_t *src, uint8_t **dst, size_t src_len);
int fuzziqer_prs_decompress_size(const uint8_t *src, size_t src_len);

#endif
