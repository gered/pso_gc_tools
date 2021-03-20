#include <stdio.h>
#include <string.h>
#include <malloc.h>

#include <iconv.h>

#include "textconv.h"
#include "retvals.h"

int sjis_to_utf8(char *s, size_t length) {
	if (!s)
		return ERROR_INVALID_PARAMS;

	iconv_t conv;
	size_t in_size, out_size;

	char *outbuf = malloc(length);

	in_size = length;
	out_size = length;
	char *in = s;
	char *out = outbuf;
	conv = iconv_open("SHIFT_JIS", "UTF-8");
	iconv(conv, &in, &in_size, &out, &out_size);
	iconv_close(conv);

	memset(s, 0, length);
	memcpy(s, outbuf, length);
	free(outbuf);

	return SUCCESS;
}
