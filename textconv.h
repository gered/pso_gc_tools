#ifndef TEXTCONV_H_INCLUDED
#define TEXTCONV_H_INCLUDED

#include <stdio.h>
#include <iconv.h>

#include "retvals.h"

int sjis_to_utf8(char *s, size_t length);

#endif
